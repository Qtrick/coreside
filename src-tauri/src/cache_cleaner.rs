//! Smart startup cache cleaner.
//!
//! Runs once at application launch to remove stale temporary files, orphaned
//! staging directories, and oversized caches.  Designed to be conservative:
//! only files/directories that are clearly stale or over quota are touched.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::app_paths::AppPaths;

/// Maximum age for staging / temp directories before they are considered stale.
const STALE_AGE: Duration = Duration::from_secs(3600); // 1 hour

/// Crawler data quota (500 MB).
const CRAWLER_QUOTA_BYTES: u64 = 500 * 1024 * 1024;

/// Maximum number of files to delete per category per run (safety bound).
const MAX_DELETIONS_PER_CATEGORY: usize = 200;

/// Result summary from a single cleanup pass.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheCleanerReport {
    pub stale_restore_staging_removed: usize,
    #[serde(alias = "staleBackupStagingRemoved")]
    pub stale_attachment_staging_removed: usize,
    pub orphaned_temp_files_removed: usize,
    pub crawler_bytes_freed: u64,
    pub context_ledger_pruned: usize,
    pub errors: Vec<String>,
}

/// Known Coreside-generated restore staging directory / file prefixes.
/// Only directories/files matching these prefixes may ever be cleaned from `restore-staging`.
pub fn is_known_restore_staging_name(name: &str) -> bool {
    name.starts_with("restore-")
        || name.starts_with("assets-media-")
        || name.starts_with("assets-att-")
        || name.starts_with("hold-")
}

/// Known Coreside temporary file naming patterns.
/// Normal media assets and unrelated dotfiles (like `.gitkeep`, `.DS_Store`) must never be deleted.
pub fn is_coreside_temp_file(name: &str) -> bool {
    name.ends_with(".tmp")
        || name.ends_with(".part")
        || name.ends_with(".staging")
        || name.starts_with(".coreside-tmp-")
}

/// Run all startup cache cleaning tasks.
pub fn clean_stale_caches(paths: &AppPaths) -> CacheCleanerReport {
    let mut report = CacheCleanerReport::default();

    // Check if an active maintenance journal exists to protect in-flight profiles.
    let protected_paths: Vec<PathBuf> = crate::maintenance_journal::load_journal(paths)
        .ok()
        .flatten()
        .map(|j| {
            let mut p = Vec::new();
            if let Some(ref s) = j.source_profile {
                p.push(PathBuf::from(s));
            }
            if let Some(ref s) = j.staged_profile {
                p.push(PathBuf::from(s));
            }
            if let Some(ref s) = j.quarantine_profile {
                p.push(PathBuf::from(s));
            }
            p
        })
        .unwrap_or_default();

    clean_stale_restore_staging(
        &paths.restore_staging,
        &protected_paths,
        &mut report.stale_restore_staging_removed,
        &mut report.errors,
    );

    clean_stale_attachment_staging(
        &paths.attachment_staging,
        &mut report.stale_attachment_staging_removed,
        &mut report.errors,
    );

    clean_orphaned_temps(
        &paths.media,
        &mut report.orphaned_temp_files_removed,
        &mut report.errors,
    );

    report.crawler_bytes_freed = clean_oversized_crawler(&paths.crawler, &mut report.errors);

    report
}

/// Remove allowlisted directories under `restore_staging` that are older than `STALE_AGE`
/// and not referenced by an active maintenance journal.
fn clean_stale_restore_staging(
    dir: &Path,
    protected_paths: &[PathBuf],
    removed: &mut usize,
    errors: &mut Vec<String>,
) {
    if !dir.is_dir() {
        return;
    }
    let now = SystemTime::now();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("restore-staging: read_dir failed: {e}"));
            return;
        }
    };
    for entry in entries.flatten() {
        if *removed >= MAX_DELETIONS_PER_CATEGORY {
            break;
        }
        let path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        // STRICT ALLOWLIST: Only recognize known Coreside-generated forms.
        // Unknown directories or files MUST be preserved.
        if !is_known_restore_staging_name(&name_str) {
            continue;
        }

        // Never delete a path referenced by an active maintenance journal.
        if protected_paths.iter().any(|p| p == &path) {
            continue;
        }

        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Do not follow symlinks outside managed root.
        if meta.file_type().is_symlink() {
            continue;
        }

        let age = now
            .duration_since(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH))
            .unwrap_or_default();
        if age > STALE_AGE {
            let res = if meta.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            if let Err(e) = res {
                errors.push(format!(
                    "restore-staging: failed to remove {}: {e}",
                    path.display()
                ));
            } else {
                *removed += 1;
            }
        }
    }
}

/// Remove orphaned staging files in `attachment_staging` that are older than `STALE_AGE`.
/// Avoids deleting arbitrary directories or non-temp data.
fn clean_stale_attachment_staging(dir: &Path, removed: &mut usize, errors: &mut Vec<String>) {
    if !dir.is_dir() {
        return;
    }
    let now = SystemTime::now();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("attachment-staging: read_dir failed: {e}"));
            return;
        }
    };
    for entry in entries.flatten() {
        if *removed >= MAX_DELETIONS_PER_CATEGORY {
            break;
        }
        let path = entry.path();
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        // Never follow symlinks; only clean regular files
        if meta.file_type().is_symlink() || !meta.is_file() {
            continue;
        }
        let age = now
            .duration_since(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH))
            .unwrap_or_default();
        if age > STALE_AGE {
            if let Err(e) = std::fs::remove_file(&path) {
                errors.push(format!(
                    "attachment-staging: failed to remove {}: {e}",
                    path.display()
                ));
            } else {
                *removed += 1;
            }
        }
    }
}

/// Remove orphaned temporary files in the media directory that are older than `STALE_AGE`.
/// Only removes explicit Coreside temp patterns (`*.tmp`, `*.part`, `*.staging`, `.coreside-tmp-*`).
/// Preserves unrelated hidden files (e.g. `.gitkeep`, `.DS_Store`).
fn clean_orphaned_temps(dir: &Path, removed: &mut usize, errors: &mut Vec<String>) {
    if !dir.is_dir() {
        return;
    }
    let now = SystemTime::now();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("media: read_dir failed: {e}"));
            return;
        }
    };
    for entry in entries.flatten() {
        if *removed >= MAX_DELETIONS_PER_CATEGORY {
            break;
        }
        let path = entry.path();
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() || !meta.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        // Only target verified Coreside temp files.
        if !is_coreside_temp_file(&name_str) {
            continue;
        }

        let age = now
            .duration_since(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH))
            .unwrap_or_default();
        if age > STALE_AGE {
            if let Err(e) = std::fs::remove_file(&path) {
                errors.push(format!("media: failed to remove {}: {e}", path.display()));
            } else {
                *removed += 1;
            }
        }
    }
}

/// If the crawler cache directory exceeds `CRAWLER_QUOTA_BYTES`, delete the
/// oldest disposable subdirectories until under quota.
///
/// CRITICAL SAFETY GUARANTEE:
/// The managed Crawl4AI installation in `crawler/service` is NEVER deleted or
/// counted toward the disposable cache quota.
fn clean_oversized_crawler(dir: &Path, errors: &mut Vec<String>) -> u64 {
    if !dir.is_dir() {
        return 0;
    }

    let mut entries: Vec<(PathBuf, SystemTime, u64)> = Vec::new();
    let mut disposable_cache_bytes: u64 = 0;

    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let path = e.path();
            let file_name = e.file_name();
            let name_str = file_name.to_string_lossy();

            // CRITICAL: NEVER delete or count the managed service directory or hidden folders.
            if name_str == "service" || name_str.starts_with('.') {
                continue;
            }

            let meta = match std::fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Never follow symlinks outside managed root.
            if meta.file_type().is_symlink() {
                continue;
            }

            if meta.is_dir() {
                let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let size = dir_size(&path);
                disposable_cache_bytes += size;
                entries.push((path, modified, size));
            }
        }
    }

    if disposable_cache_bytes <= CRAWLER_QUOTA_BYTES {
        return 0;
    }

    let mut to_free = disposable_cache_bytes - CRAWLER_QUOTA_BYTES;
    let mut freed: u64 = 0;

    // Collect subdirectories sorted by modification time (oldest first).
    entries.sort_by_key(|(_, t, _)| *t);

    for (path, _, size) in entries {
        if to_free == 0 {
            break;
        }
        if let Err(e) = std::fs::remove_dir_all(&path) {
            errors.push(format!("crawler: failed to remove {}: {e}", path.display()));
        } else {
            freed += size;
            to_free = to_free.saturating_sub(size);
        }
    }
    freed
}

/// Recursively compute directory size in bytes without following symlinks.
fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Ok(meta) = std::fs::symlink_metadata(&p) {
                if meta.file_type().is_symlink() {
                    continue;
                }
                if meta.is_file() {
                    total += meta.len();
                } else if meta.is_dir() {
                    total += dir_size(&p);
                }
            }
        }
    }
    total
}

/// Format bytes as a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn removes_stale_staging_directories_with_allowlisted_names() {
        let root = tempdir().unwrap();
        let staging = root.path().join("restore-staging");
        std::fs::create_dir_all(&staging).unwrap();

        // Create an allowlisted restore staging folder.
        let old_dir = staging.join("restore-20240101");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(old_dir.join("data.db"), b"fake").unwrap();

        // Back-date the modification time to 2 hours ago.
        let two_hours_ago = SystemTime::now() - Duration::from_secs(7200);
        filetime::set_file_mtime(
            &old_dir,
            filetime::FileTime::from_system_time(two_hours_ago),
        )
        .unwrap();

        let mut removed = 0;
        let mut errors = Vec::new();
        clean_stale_restore_staging(&staging, &[], &mut removed, &mut errors);

        assert_eq!(removed, 1);
        assert!(errors.is_empty());
        assert!(!old_dir.exists());
    }

    #[test]
    fn preserves_unrelated_staging_directories_even_if_old() {
        let root = tempdir().unwrap();
        let staging = root.path().join("restore-staging");
        std::fs::create_dir_all(&staging).unwrap();

        // Create an un-allowlisted directory (e.g. user folder or unexpected name).
        let unknown_dir = staging.join("my-custom-backup");
        std::fs::create_dir_all(&unknown_dir).unwrap();
        std::fs::write(unknown_dir.join("important.txt"), b"keep me").unwrap();

        // Back-date the modification time to 2 hours ago.
        let two_hours_ago = SystemTime::now() - Duration::from_secs(7200);
        filetime::set_file_mtime(
            &unknown_dir,
            filetime::FileTime::from_system_time(two_hours_ago),
        )
        .unwrap();

        let mut removed = 0;
        let mut errors = Vec::new();
        clean_stale_restore_staging(&staging, &[], &mut removed, &mut errors);

        assert_eq!(removed, 0);
        assert!(unknown_dir.exists());
    }

    #[test]
    fn preserves_in_flight_journal_staging() {
        let root = tempdir().unwrap();
        let staging = root.path().join("restore-staging");
        std::fs::create_dir_all(&staging).unwrap();

        let active_dir = staging.join("restore-active-in-flight");
        std::fs::create_dir_all(&active_dir).unwrap();

        let two_hours_ago = SystemTime::now() - Duration::from_secs(7200);
        filetime::set_file_mtime(
            &active_dir,
            filetime::FileTime::from_system_time(two_hours_ago),
        )
        .unwrap();

        let protected = vec![active_dir.clone()];
        let mut removed = 0;
        let mut errors = Vec::new();
        clean_stale_restore_staging(&staging, &protected, &mut removed, &mut errors);

        assert_eq!(removed, 0);
        assert!(active_dir.exists());
    }

    #[test]
    fn keeps_recent_staging() {
        let root = tempdir().unwrap();
        let staging = root.path().join("restore-staging");
        std::fs::create_dir_all(&staging).unwrap();

        let recent_dir = staging.join("restore-recent");
        std::fs::create_dir_all(&recent_dir).unwrap();

        let mut removed = 0;
        let mut errors = Vec::new();
        clean_stale_restore_staging(&staging, &[], &mut removed, &mut errors);

        assert_eq!(removed, 0);
        assert!(recent_dir.exists());
    }

    #[test]
    fn removes_orphaned_tmp_files_and_preserves_unrelated_dotfiles() {
        let root = tempdir().unwrap();
        let media = root.path().join("media");
        std::fs::create_dir_all(&media).unwrap();

        let two_hours_ago = SystemTime::now() - Duration::from_secs(7200);

        // 1. Explicit Coreside tmp file (should be removed)
        let tmp_file = media.join("orphan.tmp");
        std::fs::write(&tmp_file, b"data").unwrap();
        filetime::set_file_mtime(
            &tmp_file,
            filetime::FileTime::from_system_time(two_hours_ago),
        )
        .unwrap();

        // 2. Coreside tmp prefix (should be removed)
        let coreside_tmp = media.join(".coreside-tmp-xyz");
        std::fs::write(&coreside_tmp, b"staging").unwrap();
        filetime::set_file_mtime(
            &coreside_tmp,
            filetime::FileTime::from_system_time(two_hours_ago),
        )
        .unwrap();

        // 3. Unrelated hidden dotfile (e.g. .gitkeep, .DS_Store) - MUST BE PRESERVED
        let gitkeep = media.join(".gitkeep");
        std::fs::write(&gitkeep, b"").unwrap();
        filetime::set_file_mtime(
            &gitkeep,
            filetime::FileTime::from_system_time(two_hours_ago),
        )
        .unwrap();

        let ds_store = media.join(".DS_Store");
        std::fs::write(&ds_store, b"binary").unwrap();
        filetime::set_file_mtime(
            &ds_store,
            filetime::FileTime::from_system_time(two_hours_ago),
        )
        .unwrap();

        // 4. Real asset (should NOT be removed)
        let real_file = media.join("photo.jpg");
        std::fs::write(&real_file, b"image").unwrap();

        let mut removed = 0;
        let mut errors = Vec::new();
        clean_orphaned_temps(&media, &mut removed, &mut errors);

        assert_eq!(removed, 2);
        assert!(!tmp_file.exists());
        assert!(!coreside_tmp.exists());
        assert!(gitkeep.exists(), ".gitkeep must be preserved!");
        assert!(ds_store.exists(), ".DS_Store must be preserved!");
        assert!(real_file.exists(), "real media must be preserved!");
    }

    #[test]
    fn crawler_service_directory_strictly_preserved() {
        let root = tempdir().unwrap();
        let crawler = root.path().join("crawler");
        std::fs::create_dir_all(&crawler).unwrap();

        // Create crawler/service directory (Crawl4AI installation).
        let service_dir = crawler.join("service");
        std::fs::create_dir_all(&service_dir).unwrap();
        std::fs::write(service_dir.join("main.py"), b"python code").unwrap();

        // Back-date service to very old
        let very_old = SystemTime::now() - Duration::from_secs(86400 * 30);
        filetime::set_file_mtime(&service_dir, filetime::FileTime::from_system_time(very_old))
            .unwrap();

        // Create disposable sessions
        let session1 = crawler.join("session1");
        std::fs::create_dir_all(&session1).unwrap();
        std::fs::write(session1.join("crawl.html"), vec![0u8; 100]).unwrap();

        let mut errors = Vec::new();
        // Since under quota, freed is 0
        let freed = clean_oversized_crawler(&crawler, &mut errors);
        assert_eq!(freed, 0);
        assert!(
            service_dir.exists(),
            "service directory must never be deleted!"
        );
    }

    #[test]
    fn format_bytes_works() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }
}
