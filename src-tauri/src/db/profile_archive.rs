//! Versioned `.coreside-backup` profile archives (SQLite snapshot + manifest).

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use super::{Database, DbError, DbResult};
use rusqlite::OptionalExtension;

pub const BACKUP_FORMAT: &str = "coreside-backup";
pub const BACKUP_SCHEMA_VERSION: u32 = 2;
/// Legacy DB-only archives.
pub const BACKUP_SCHEMA_VERSION_V1: u32 = 1;
/// Hard ceiling for a database snapshot inside a backup (ZIP bomb / OOM guard).
const MAX_ARCHIVE_DB_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_ARCHIVE_ASSET_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ARCHIVE_ASSET_COUNT: u64 = 10_000;
const MAX_ARCHIVE_TOTAL_ASSET_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format: String,
    pub schema_version: u32,
    pub application_version: String,
    pub created_at: String,
    pub database: BackupDatabaseEntry,
    pub media: BackupAssetSection,
    pub attachments: BackupAssetSection,
    pub caches_included: bool,
    pub integrity: BackupIntegrity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDatabaseEntry {
    pub path: String,
    pub sha256: String,
    pub byte_size: u64,
    pub latest_migration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAssetEntry {
    pub name: String,
    pub sha256: String,
    pub byte_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAssetSection {
    pub included: bool,
    pub count: u64,
    pub total_bytes: u64,
    /// `complete` | `directory_enumerated` | `truncated` | `unknown` (legacy).
    #[serde(default = "default_asset_completeness")]
    pub completeness: String,
    #[serde(default)]
    pub entries: Vec<BackupAssetEntry>,
}

fn default_asset_completeness() -> String {
    "unknown".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupIntegrity {
    pub database_quick_check: String,
    pub foreign_key_check: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub format: String,
    pub schema_version: u32,
    pub application_version: String,
    pub created_at: String,
    pub latest_migration: Option<String>,
    pub database_bytes: u64,
    pub media_count: u64,
    pub attachment_count: u64,
    pub integrity_ok: bool,
    pub warnings: Vec<String>,
}

fn sha256_file(path: &Path) -> DbResult<String> {
    let mut file = File::open(path)
        .map_err(|e| DbError::Invalid(format!("Failed to hash backup file: {e}")))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| DbError::Invalid(format!("Failed to read for hash: {e}")))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn is_safe_archive_entry_name(name: &str) -> bool {
    if name.is_empty() || name.contains('\0') || name.contains("..") {
        return false;
    }
    if name.starts_with('/') || name.starts_with('\\') {
        return false;
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return false;
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            _ => return false,
        }
    }
    true
}

fn create_owner_only_file(path: &Path) -> DbResult<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| DbError::Invalid(format!("create owner-only file: {e}")))
    }
    #[cfg(not(unix))]
    {
        File::create(path).map_err(|e| DbError::Invalid(format!("create file: {e}")))
    }
}

fn copy_limited_hashed(
    reader: &mut impl Read,
    dest: &Path,
    max_bytes: u64,
) -> DbResult<(u64, String)> {
    let parent = dest
        .parent()
        .ok_or_else(|| DbError::Invalid("extract destination has no parent".into()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| DbError::Invalid(format!("create extract parent: {e}")))?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| DbError::Invalid(format!("create extract tempfile: {e}")))?;
    set_owner_only_permissions(tmp.path());
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| DbError::Invalid(format!("read archive entry: {e}")))?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        if total > max_bytes {
            return Err(DbError::Invalid("archive entry exceeds size limit".into()));
        }
        hasher.update(&buf[..n]);
        tmp.write_all(&buf[..n])
            .map_err(|e| DbError::Invalid(format!("write extract dest: {e}")))?;
    }
    tmp.flush()
        .map_err(|e| DbError::Invalid(format!("flush extract dest: {e}")))?;
    set_owner_only_permissions(tmp.path());
    tmp.persist(dest)
        .map_err(|e| DbError::Invalid(format!("persist extract dest: {e}")))?;
    set_owner_only_permissions(dest);
    Ok((total, hex::encode(hasher.finalize())))
}

fn read_limited_string(reader: &mut impl Read, max_bytes: u64) -> DbResult<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| DbError::Invalid(format!("read archive entry: {e}")))?;
        if n == 0 {
            break;
        }
        if (buf.len() as u64).saturating_add(n as u64) > max_bytes {
            return Err(DbError::Invalid("manifest exceeds size limit".into()));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    String::from_utf8(buf).map_err(|e| DbError::Invalid(format!("manifest utf-8: {e}")))
}

fn preview_temp_db_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "coreside-restore-preview-{}.db",
        uuid::Uuid::new_v4()
    ))
}

fn classify_archive_entry(name: &str) -> Option<&'static str> {
    if name == "manifest.json" {
        return Some("manifest");
    }
    if name == "database/coreside.db" {
        return Some("database");
    }
    if let Some(rest) = name.strip_prefix("media/") {
        if !rest.is_empty() && !rest.contains('/') && !rest.contains('\\') {
            return Some("media");
        }
    }
    if let Some(rest) = name.strip_prefix("attachments/") {
        if !rest.is_empty() && !rest.contains('/') && !rest.contains('\\') {
            return Some("attachments");
        }
    }
    None
}

fn collect_named_files(
    root: &Path,
    max_count: u64,
    max_total: u64,
) -> DbResult<(Vec<(String, PathBuf, u64, String)>, u64, String)> {
    let mut files = Vec::new();
    let mut total = 0u64;
    if !root.exists() {
        return Ok((files, total, "complete".into()));
    }
    let entries =
        std::fs::read_dir(root).map_err(|e| DbError::Invalid(format!("read asset root: {e}")))?;
    let mut truncated = false;
    for entry in entries.flatten() {
        let meta = match std::fs::symlink_metadata(entry.path()) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() || !meta.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.is_empty()
            || name.contains("..")
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
        {
            return Err(DbError::Invalid(format!(
                "unsafe asset name under {}",
                root.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("assets")
            )));
        }
        let size = meta.len();
        if size > MAX_ARCHIVE_ASSET_BYTES {
            return Err(DbError::Invalid(format!(
                "asset exceeds per-file size limit ({name})"
            )));
        }
        if files.len() as u64 >= max_count || total.saturating_add(size) > max_total {
            truncated = true;
            break;
        }
        let hash = sha256_file(&entry.path())?;
        files.push((name.to_string(), entry.path(), size, hash));
        total = total.saturating_add(size);
    }
    if truncated {
        return Err(DbError::Invalid(
            "asset archive budget exceeded; refusing truncated backup".into(),
        ));
    }
    Ok((files, total, "directory_enumerated".into()))
}

fn collect_named_files_from_list(
    root: &Path,
    names: &[String],
    max_count: u64,
    max_total: u64,
) -> DbResult<(Vec<(String, PathBuf, u64, String)>, u64, String)> {
    let mut files = Vec::new();
    let mut total = 0u64;
    for name in names {
        if name.is_empty()
            || name.contains("..")
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
        {
            return Err(DbError::Invalid(format!(
                "unsafe referenced asset name: {name}"
            )));
        }
        if files.len() as u64 >= max_count {
            return Err(DbError::Invalid(
                "asset archive count limit exceeded; refusing truncated backup".into(),
            ));
        }
        let path = root.join(name);
        let meta = std::fs::symlink_metadata(&path).map_err(|_| {
            DbError::Invalid(format!("referenced media asset missing on disk: {name}"))
        })?;
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err(DbError::Invalid(format!(
                "referenced media asset is not a regular file: {name}"
            )));
        }
        let size = meta.len();
        if size > MAX_ARCHIVE_ASSET_BYTES {
            return Err(DbError::Invalid(format!(
                "referenced asset exceeds per-file size limit ({name})"
            )));
        }
        if total.saturating_add(size) > max_total {
            return Err(DbError::Invalid(
                "asset archive byte budget exceeded; refusing truncated backup".into(),
            ));
        }
        let hash = sha256_file(&path)?;
        files.push((name.clone(), path, size, hash));
        total = total.saturating_add(size);
    }
    Ok((files, total, "complete".into()))
}

fn table_exists(db: &Database, name: &str) -> DbResult<bool> {
    db.conn()
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            [name],
            |_| Ok(true),
        )
        .optional()
        .map(|v| v.unwrap_or(false))
        .map_err(|e| DbError::Invalid(format!("{name} presence check: {e}")))
}

fn list_referenced_media_filenames(db: &Database) -> DbResult<Vec<String>> {
    if !table_exists(db, "media_assets")? {
        return Ok(Vec::new());
    }
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT local_filename AS name FROM media_assets
             UNION
             SELECT thumbnail_filename AS name FROM media_assets
             WHERE thumbnail_filename IS NOT NULL AND TRIM(thumbnail_filename) != ''",
        )
        .map_err(|e| DbError::Invalid(format!("media_assets query prepare: {e}")))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| DbError::Invalid(format!("media_assets query: {e}")))?;
    let mut names = Vec::new();
    for row in rows {
        let name = row.map_err(|e| DbError::Invalid(format!("media_assets row: {e}")))?;
        names.push(name);
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// Active chat attachment storage keys that are backup-eligible (attached/durable).
/// Staged drafts remain backup_eligible=0 and must not enter profile archives.
fn list_referenced_attachment_storage_keys(db: &Database) -> DbResult<Vec<String>> {
    if !table_exists(db, "chat_attachments")? {
        return Ok(Vec::new());
    }
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT storage_key FROM chat_attachments
             WHERE deleted_at IS NULL
               AND backup_eligible = 1
               AND state NOT IN ('deleted', 'expired', 'failed', 'cancelled', 'staged')",
        )
        .map_err(|e| DbError::Invalid(format!("chat_attachments query prepare: {e}")))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| DbError::Invalid(format!("chat_attachments query: {e}")))?;
    let mut names = Vec::new();
    for row in rows {
        let name = row.map_err(|e| DbError::Invalid(format!("chat_attachments row: {e}")))?;
        names.push(name);
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// Resolve attachment files from the durable root and its `staging/` subdirectory.
fn collect_named_files_from_attachment_roots(
    attachments_root: &Path,
    names: &[String],
    max_count: u64,
    max_total: u64,
) -> DbResult<(Vec<(String, PathBuf, u64, String)>, u64, String)> {
    let staging_root = attachments_root.join("staging");
    let mut files = Vec::new();
    let mut total = 0u64;
    for name in names {
        if name.is_empty()
            || name.contains("..")
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
        {
            return Err(DbError::Invalid(format!(
                "unsafe referenced attachment name: {name}"
            )));
        }
        if files.len() as u64 >= max_count {
            return Err(DbError::Invalid(
                "asset archive count limit exceeded; refusing truncated backup".into(),
            ));
        }
        let path = {
            let mut found = None;
            for root in [attachments_root, &staging_root] {
                let candidate = root.join(name);
                match std::fs::symlink_metadata(&candidate) {
                    Ok(meta) if !meta.file_type().is_symlink() && meta.is_file() => {
                        found = Some((candidate, meta.len()));
                        break;
                    }
                    _ => continue,
                }
            }
            found.ok_or_else(|| {
                DbError::Invalid(format!(
                    "referenced chat attachment missing on disk: {name}"
                ))
            })?
        };
        let (path, size) = path;
        if size > MAX_ARCHIVE_ASSET_BYTES {
            return Err(DbError::Invalid(format!(
                "referenced attachment exceeds per-file size limit ({name})"
            )));
        }
        if total.saturating_add(size) > max_total {
            return Err(DbError::Invalid(
                "asset archive byte budget exceeded; refusing truncated backup".into(),
            ));
        }
        let hash = sha256_file(&path)?;
        files.push((name.clone(), path, size, hash));
        total = total.saturating_add(size);
    }
    Ok((files, total, "complete".into()))
}

fn set_owner_only_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if path.is_dir() { 0o700 } else { 0o600 };
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
}

impl Database {
    /// Create a versioned `.coreside-backup` archive (DB snapshot only — schema v1).
    pub fn create_profile_archive(
        &self,
        dest: &Path,
        application_version: &str,
    ) -> DbResult<BackupManifest> {
        self.create_profile_archive_with_assets(dest, application_version, None, None)
    }

    /// Create a full-profile archive when media/attachment roots are provided (schema v2).
    pub fn create_profile_archive_with_assets(
        &self,
        dest: &Path,
        application_version: &str,
        media_root: Option<&Path>,
        attachments_root: Option<&Path>,
    ) -> DbResult<BackupManifest> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DbError::Invalid(format!("Failed to create backup dir: {e}")))?;
            set_owner_only_permissions(parent);
        }
        let staging_dir = dest
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".tmp-backup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&staging_dir)
            .map_err(|e| DbError::Invalid(format!("Failed to create staging: {e}")))?;
        set_owner_only_permissions(&staging_dir);
        let db_snap = staging_dir.join("coreside.db");
        let include_assets = media_root.is_some() || attachments_root.is_some();
        let result = (|| {
            self.snapshot_to_path(&db_snap)?;
            let snap = Database::open_path(&db_snap)?;
            let quick = snap.quick_check()?;
            let fk = snap.foreign_key_check_summary()?;
            let latest = snap.latest_migration_name()?;
            let referenced_media = list_referenced_media_filenames(&snap)?;
            let referenced_attachments = list_referenced_attachment_storage_keys(&snap)?;
            let media_table = table_exists(&snap, "media_assets")?;
            let attachments_table = table_exists(&snap, "chat_attachments")?;
            drop(snap);
            let byte_size = std::fs::metadata(&db_snap)
                .map(|m| m.len())
                .map_err(|e| DbError::Invalid(format!("snapshot stat failed: {e}")))?;
            let hash = sha256_file(&db_snap)?;

            let mut media_files = Vec::new();
            let mut media_bytes = 0u64;
            let mut media_completeness = "complete".to_string();
            let mut attachment_files = Vec::new();
            let mut attachment_bytes = 0u64;
            let mut attachment_completeness = "complete".to_string();
            if let Some(root) = media_root {
                let (files, total, completeness) = if media_table {
                    // DB-referenced media only — empty refs means nothing to archive.
                    if referenced_media.is_empty() {
                        (Vec::new(), 0, "complete".into())
                    } else {
                        collect_named_files_from_list(
                            root,
                            &referenced_media,
                            MAX_ARCHIVE_ASSET_COUNT,
                            MAX_ARCHIVE_TOTAL_ASSET_BYTES,
                        )?
                    }
                } else {
                    // Legacy profiles without media_assets: directory enumeration.
                    collect_named_files(
                        root,
                        MAX_ARCHIVE_ASSET_COUNT,
                        MAX_ARCHIVE_TOTAL_ASSET_BYTES,
                    )?
                };
                media_files = files;
                media_bytes = total;
                media_completeness = completeness;
            }
            if let Some(root) = attachments_root {
                let remaining_count =
                    MAX_ARCHIVE_ASSET_COUNT.saturating_sub(media_files.len() as u64);
                let remaining_bytes = MAX_ARCHIVE_TOTAL_ASSET_BYTES.saturating_sub(media_bytes);
                let (files, total, completeness) = if attachments_table {
                    if referenced_attachments.is_empty() {
                        (Vec::new(), 0, "complete".into())
                    } else {
                        collect_named_files_from_attachment_roots(
                            root,
                            &referenced_attachments,
                            remaining_count,
                            remaining_bytes,
                        )?
                    }
                } else {
                    // Legacy profiles without chat_attachments: durable root only.
                    collect_named_files(root, remaining_count, remaining_bytes)?
                };
                attachment_files = files;
                attachment_bytes = total;
                attachment_completeness = completeness;
            }

            let schema_version = if include_assets {
                BACKUP_SCHEMA_VERSION
            } else {
                BACKUP_SCHEMA_VERSION_V1
            };

            let media_entries: Vec<BackupAssetEntry> = media_files
                .iter()
                .map(|(name, _, size, hash)| BackupAssetEntry {
                    name: name.clone(),
                    sha256: hash.clone(),
                    byte_size: *size,
                })
                .collect();
            let attachment_entries: Vec<BackupAssetEntry> = attachment_files
                .iter()
                .map(|(name, _, size, hash)| BackupAssetEntry {
                    name: name.clone(),
                    sha256: hash.clone(),
                    byte_size: *size,
                })
                .collect();

            let mut manifest = BackupManifest {
                format: BACKUP_FORMAT.into(),
                schema_version,
                application_version: application_version.into(),
                created_at: chrono::Utc::now().to_rfc3339(),
                database: BackupDatabaseEntry {
                    path: "database/coreside.db".into(),
                    sha256: hash,
                    byte_size,
                    latest_migration: latest,
                },
                media: BackupAssetSection {
                    included: include_assets,
                    count: media_files.len() as u64,
                    total_bytes: media_bytes,
                    completeness: if include_assets {
                        media_completeness
                    } else {
                        "unknown".into()
                    },
                    entries: media_entries,
                },
                attachments: BackupAssetSection {
                    included: include_assets,
                    count: attachment_files.len() as u64,
                    total_bytes: attachment_bytes,
                    completeness: if include_assets {
                        attachment_completeness
                    } else {
                        "unknown".into()
                    },
                    entries: attachment_entries,
                },
                caches_included: false,
                integrity: BackupIntegrity {
                    database_quick_check: quick,
                    foreign_key_check: fk,
                },
            };
            if manifest.integrity.database_quick_check != "ok" {
                return Err(DbError::Invalid("Snapshot failed quick_check".into()));
            }
            if manifest.integrity.foreign_key_check != "ok" {
                return Err(DbError::Invalid("Snapshot failed foreign_key_check".into()));
            }

            let tmp_zip = staging_dir.join("archive.zip");
            {
                // Create with 0o600 so the archive is never briefly world-readable.
                let file = create_owner_only_file(&tmp_zip)?;
                let mut zip = ZipWriter::new(file);
                let opts = SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);

                // Write assets first, finalize manifest last so counts stay accurate.
                zip.start_file("database/coreside.db", opts)
                    .map_err(|e| DbError::Invalid(format!("zip db entry: {e}")))?;
                let mut db_file = File::open(&db_snap)
                    .map_err(|e| DbError::Invalid(format!("open snapshot: {e}")))?;
                std::io::copy(&mut db_file, &mut zip)
                    .map_err(|e| DbError::Invalid(format!("copy db into zip: {e}")))?;

                for (name, path, ..) in &media_files {
                    let entry_name = format!("media/{name}");
                    if classify_archive_entry(&entry_name) != Some("media") {
                        return Err(DbError::Invalid("refusing unsafe media entry".into()));
                    }
                    zip.start_file(&entry_name, opts)
                        .map_err(|e| DbError::Invalid(format!("zip media entry: {e}")))?;
                    let mut f = File::open(path)
                        .map_err(|e| DbError::Invalid(format!("open media asset: {e}")))?;
                    std::io::copy(&mut f, &mut zip)
                        .map_err(|e| DbError::Invalid(format!("copy media asset: {e}")))?;
                }
                for (name, path, ..) in &attachment_files {
                    let entry_name = format!("attachments/{name}");
                    if classify_archive_entry(&entry_name) != Some("attachments") {
                        return Err(DbError::Invalid("refusing unsafe attachment entry".into()));
                    }
                    zip.start_file(&entry_name, opts)
                        .map_err(|e| DbError::Invalid(format!("zip attachment entry: {e}")))?;
                    let mut f = File::open(path)
                        .map_err(|e| DbError::Invalid(format!("open attachment: {e}")))?;
                    std::io::copy(&mut f, &mut zip)
                        .map_err(|e| DbError::Invalid(format!("copy attachment: {e}")))?;
                }

                zip.start_file("manifest.json", opts)
                    .map_err(|e| DbError::Invalid(format!("zip manifest entry: {e}")))?;
                let body = serde_json::to_vec_pretty(&manifest)
                    .map_err(|e| DbError::Invalid(format!("manifest serialize: {e}")))?;
                zip.write_all(&body)
                    .map_err(|e| DbError::Invalid(format!("write manifest: {e}")))?;
                zip.finish()
                    .map_err(|e| DbError::Invalid(format!("zip finish: {e}")))?;
            }

            let _preview = preview_profile_archive(&tmp_zip)?;
            std::fs::rename(&tmp_zip, dest)
                .map_err(|e| DbError::Invalid(format!("finalize backup rename: {e}")))?;
            set_owner_only_permissions(dest);
            // Re-read counts into returned manifest for callers.
            if include_assets {
                manifest.schema_version = BACKUP_SCHEMA_VERSION;
            }
            Ok(manifest)
        })();

        let _ = std::fs::remove_dir_all(&staging_dir);
        result
    }
}

/// Read and validate a backup archive without mutating the active profile.
pub fn preview_profile_archive(path: &Path) -> DbResult<RestorePreview> {
    let file =
        File::open(path).map_err(|e| DbError::Invalid(format!("open backup failed: {e}")))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| DbError::Invalid(format!("invalid zip: {e}")))?;

    let mut manifest: Option<BackupManifest> = None;
    let mut db_extracted: Option<(PathBuf, u64, String)> = None;
    let tmp_db = preview_temp_db_path();

    let result = (|| {
        let mut media_count = 0u64;
        let mut attachment_count = 0u64;
        let mut total_asset_bytes = 0u64;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| DbError::Invalid(format!("zip entry: {e}")))?;
            let name = entry.name().to_string();
            if !is_safe_archive_entry_name(&name) || entry.enclosed_name().is_none() {
                return Err(DbError::Invalid("archive path traversal rejected".into()));
            }
            if name.ends_with('/') {
                continue;
            }
            let kind = classify_archive_entry(&name).ok_or_else(|| {
                DbError::Invalid(format!("unexpected archive entry rejected: {name}"))
            })?;
            if kind == "manifest" {
                let buf = read_limited_string(&mut entry, MAX_MANIFEST_BYTES)?;
                manifest = Some(
                    serde_json::from_str(&buf)
                        .map_err(|e| DbError::Invalid(format!("manifest parse: {e}")))?,
                );
            } else if kind == "database" {
                let (byte_size, hash) =
                    copy_limited_hashed(&mut entry, &tmp_db, MAX_ARCHIVE_DB_BYTES)?;
                db_extracted = Some((tmp_db.clone(), byte_size, hash));
            } else if kind == "media" || kind == "attachments" {
                if media_count.saturating_add(attachment_count) >= MAX_ARCHIVE_ASSET_COUNT {
                    return Err(DbError::Invalid(
                        "archive asset count limit exceeded".into(),
                    ));
                }
                let remaining = MAX_ARCHIVE_TOTAL_ASSET_BYTES.saturating_sub(total_asset_bytes);
                let per_file_cap = remaining.min(MAX_ARCHIVE_ASSET_BYTES);
                if per_file_cap == 0 {
                    return Err(DbError::Invalid(
                        "archive total asset byte budget exceeded".into(),
                    ));
                }
                let mut sink = std::io::sink();
                let mut total = 0u64;
                let mut buf = [0u8; 8192];
                loop {
                    let n = entry
                        .read(&mut buf)
                        .map_err(|e| DbError::Invalid(format!("read archive entry: {e}")))?;
                    if n == 0 {
                        break;
                    }
                    total = total.saturating_add(n as u64);
                    if total > per_file_cap {
                        return Err(DbError::Invalid("archive asset exceeds size limit".into()));
                    }
                    sink.write_all(&buf[..n])
                        .map_err(|e| DbError::Invalid(format!("discard asset: {e}")))?;
                }
                if total > 0 {
                    total_asset_bytes = total_asset_bytes.saturating_add(total);
                    if kind == "media" {
                        media_count += 1;
                    } else {
                        attachment_count += 1;
                    }
                }
            }
        }

        let manifest = manifest.ok_or_else(|| DbError::Invalid("missing manifest.json".into()))?;
        if manifest.format != BACKUP_FORMAT {
            return Err(DbError::Invalid("unsupported backup format".into()));
        }
        if manifest.schema_version > BACKUP_SCHEMA_VERSION {
            return Err(DbError::Invalid("backup requires a newer Coreside".into()));
        }
        let (db_path, byte_size, hash) =
            db_extracted.ok_or_else(|| DbError::Invalid("missing database snapshot".into()))?;
        if hash != manifest.database.sha256 {
            return Err(DbError::Invalid("database hash mismatch".into()));
        }
        if byte_size != manifest.database.byte_size {
            return Err(DbError::Invalid("database size mismatch".into()));
        }

        // Recompute integrity from the snapshot — do not trust self-reported manifest fields.
        let snap = Database::open_path(&db_path)?;
        let quick = snap.quick_check()?;
        let fk = snap.foreign_key_check_summary()?;
        let integrity_ok = quick == "ok" && fk == "ok";

        let mut warnings = Vec::new();
        if manifest.schema_version <= BACKUP_SCHEMA_VERSION_V1 || !manifest.media.included {
            warnings.push(
                "Media files were not included in this backup (partial-profile / legacy).".into(),
            );
        }
        if manifest.schema_version <= BACKUP_SCHEMA_VERSION_V1 || !manifest.attachments.included {
            warnings.push(
                "Chat attachments were not included in this backup (partial-profile / legacy)."
                    .into(),
            );
        }
        if manifest.media.count != media_count || manifest.attachments.count != attachment_count {
            warnings.push("Backup manifest asset counts did not match archive entries.".into());
        }
        if manifest.media.completeness == "truncated"
            || manifest.attachments.completeness == "truncated"
        {
            warnings.push("Backup manifest reports truncated asset completeness.".into());
        }
        if manifest.media.completeness == "directory_enumerated" {
            warnings
                .push("Media assets were directory-enumerated (no DB references captured).".into());
        }
        if manifest.attachments.completeness == "directory_enumerated" {
            warnings.push(
                "Chat attachments were directory-enumerated (no DB storage_key references captured)."
                    .into(),
            );
        }
        if !manifest.media.entries.is_empty()
            && manifest.media.entries.len() as u64 != manifest.media.count
        {
            warnings.push("Media entry hash list length does not match media count.".into());
        }
        if !manifest.attachments.entries.is_empty()
            && manifest.attachments.entries.len() as u64 != manifest.attachments.count
        {
            warnings
                .push("Attachment entry hash list length does not match attachment count.".into());
        }
        if !integrity_ok {
            warnings.push("Database integrity checks did not pass for this backup.".into());
        }
        if manifest.integrity.database_quick_check != quick
            || manifest.integrity.foreign_key_check != fk
        {
            warnings
                .push("Backup manifest integrity fields did not match recomputed checks.".into());
        }

        Ok(RestorePreview {
            format: manifest.format,
            schema_version: manifest.schema_version,
            application_version: manifest.application_version,
            created_at: manifest.created_at,
            latest_migration: manifest.database.latest_migration,
            database_bytes: byte_size,
            media_count,
            attachment_count,
            integrity_ok,
            warnings,
        })
    })();

    let _ = std::fs::remove_file(&tmp_db);
    result
}

/// Extract the database snapshot from a validated archive into `dest_db`.
pub fn extract_database_from_archive(archive_path: &Path, dest_db: &Path) -> DbResult<()> {
    let preview = preview_profile_archive(archive_path)?;
    if !preview.integrity_ok {
        return Err(DbError::Invalid(
            "backup failed integrity checks; refusing extract".into(),
        ));
    }
    let file = File::open(archive_path)
        .map_err(|e| DbError::Invalid(format!("open backup failed: {e}")))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| DbError::Invalid(format!("invalid zip: {e}")))?;

    let expected_hash = {
        let mut manifest_entry = archive
            .by_name("manifest.json")
            .map_err(|_| DbError::Invalid("missing manifest.json".into()))?;
        let buf = read_limited_string(&mut manifest_entry, MAX_MANIFEST_BYTES)?;
        drop(manifest_entry);
        let manifest: BackupManifest = serde_json::from_str(&buf)
            .map_err(|e| DbError::Invalid(format!("manifest parse: {e}")))?;
        manifest.database.sha256
    };

    let mut entry = archive
        .by_name("database/coreside.db")
        .map_err(|_| DbError::Invalid("missing database snapshot".into()))?;
    if !is_safe_archive_entry_name(entry.name()) || entry.enclosed_name().is_none() {
        return Err(DbError::Invalid("archive path traversal rejected".into()));
    }
    if let Some(parent) = dest_db.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DbError::Invalid(format!("create dest dir: {e}")))?;
    }
    let (byte_size, hash) = copy_limited_hashed(&mut entry, dest_db, MAX_ARCHIVE_DB_BYTES)?;
    if hash != expected_hash || byte_size != preview.database_bytes {
        let _ = std::fs::remove_file(dest_db);
        return Err(DbError::Invalid(
            "extracted database integrity mismatch".into(),
        ));
    }
    Ok(())
}

/// Extract media/ and attachments/ entries into destination roots (typically restore staging).
pub fn extract_assets_from_archive(
    archive_path: &Path,
    media_root: &Path,
    attachments_root: &Path,
) -> DbResult<(u64, u64)> {
    let file = File::open(archive_path)
        .map_err(|e| DbError::Invalid(format!("open backup failed: {e}")))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| DbError::Invalid(format!("invalid zip: {e}")))?;

    let (expected_hashes, require_asset_hashes) = {
        let mut manifest_entry = archive
            .by_name("manifest.json")
            .map_err(|_| DbError::Invalid("missing manifest.json".into()))?;
        let buf = read_limited_string(&mut manifest_entry, MAX_MANIFEST_BYTES)?;
        drop(manifest_entry);
        let manifest: BackupManifest = serde_json::from_str(&buf)
            .map_err(|e| DbError::Invalid(format!("manifest parse: {e}")))?;
        let mut map = std::collections::HashMap::new();
        for entry in &manifest.media.entries {
            map.insert(format!("media/{}", entry.name), entry.sha256.clone());
        }
        for entry in &manifest.attachments.entries {
            map.insert(format!("attachments/{}", entry.name), entry.sha256.clone());
        }
        // Schema v2 archives that claim assets must carry per-asset hashes.
        let require = manifest.schema_version >= BACKUP_SCHEMA_VERSION
            && ((manifest.media.included && manifest.media.count > 0)
                || (manifest.attachments.included && manifest.attachments.count > 0));
        if require && map.is_empty() {
            return Err(DbError::Invalid(
                "schema v2 asset archive missing per-asset hashes".into(),
            ));
        }
        if require {
            if manifest.media.included
                && manifest.media.count > 0
                && manifest.media.entries.len() as u64 != manifest.media.count
            {
                return Err(DbError::Invalid(
                    "media hash entry count does not match media count".into(),
                ));
            }
            if manifest.attachments.included
                && manifest.attachments.count > 0
                && manifest.attachments.entries.len() as u64 != manifest.attachments.count
            {
                return Err(DbError::Invalid(
                    "attachment hash entry count does not match attachment count".into(),
                ));
            }
        }
        (map, require)
    };

    std::fs::create_dir_all(media_root)
        .map_err(|e| DbError::Invalid(format!("create media root: {e}")))?;
    std::fs::create_dir_all(attachments_root)
        .map_err(|e| DbError::Invalid(format!("create attachments root: {e}")))?;
    set_owner_only_permissions(media_root);
    set_owner_only_permissions(attachments_root);

    let mut media_count = 0u64;
    let mut attachment_count = 0u64;
    let mut total_asset_bytes = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| DbError::Invalid(format!("zip entry: {e}")))?;
        let name = entry.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        if !is_safe_archive_entry_name(&name) || entry.enclosed_name().is_none() {
            return Err(DbError::Invalid("archive path traversal rejected".into()));
        }
        let kind = classify_archive_entry(&name)
            .ok_or_else(|| DbError::Invalid(format!("unexpected archive entry: {name}")))?;
        let dest = match kind {
            "media" => {
                let file_name = name
                    .strip_prefix("media/")
                    .ok_or_else(|| DbError::Invalid("invalid media entry".into()))?;
                if file_name.is_empty() || file_name.contains('/') || file_name.contains('\\') {
                    return Err(DbError::Invalid("unsafe media entry name".into()));
                }
                media_root.join(file_name)
            }
            "attachments" => {
                let file_name = name
                    .strip_prefix("attachments/")
                    .ok_or_else(|| DbError::Invalid("invalid attachment entry".into()))?;
                if file_name.is_empty() || file_name.contains('/') || file_name.contains('\\') {
                    return Err(DbError::Invalid("unsafe attachment entry name".into()));
                }
                attachments_root.join(file_name)
            }
            _ => continue,
        };
        if dest.exists() {
            let file_name = dest.file_name().and_then(|s| s.to_str()).unwrap_or("asset");
            let quarantine =
                dest.with_file_name(format!("{file_name}.pre-restore-{}", uuid::Uuid::new_v4()));
            let _ = std::fs::rename(&dest, &quarantine);
        }
        if media_count.saturating_add(attachment_count) >= MAX_ARCHIVE_ASSET_COUNT {
            return Err(DbError::Invalid(
                "archive asset count limit exceeded".into(),
            ));
        }
        let remaining = MAX_ARCHIVE_TOTAL_ASSET_BYTES.saturating_sub(total_asset_bytes);
        let per_file_cap = remaining.min(MAX_ARCHIVE_ASSET_BYTES);
        if per_file_cap == 0 {
            return Err(DbError::Invalid(
                "archive total asset byte budget exceeded".into(),
            ));
        }
        let (bytes, hash) = copy_limited_hashed(&mut entry, &dest, per_file_cap)?;
        if bytes == 0 {
            let _ = std::fs::remove_file(&dest);
            continue;
        }
        if require_asset_hashes || expected_hashes.contains_key(&name) {
            let Some(expected) = expected_hashes.get(&name) else {
                let _ = std::fs::remove_file(&dest);
                return Err(DbError::Invalid(format!(
                    "asset missing from manifest hash list: {name}"
                )));
            };
            if &hash != expected {
                let _ = std::fs::remove_file(&dest);
                return Err(DbError::Invalid(format!("asset hash mismatch for {name}")));
            }
        }
        total_asset_bytes = total_asset_bytes.saturating_add(bytes);
        if kind == "media" {
            media_count += 1;
        } else {
            attachment_count += 1;
        }
    }
    Ok((media_count, attachment_count))
}

fn ensure_dir(path: &Path) -> DbResult<()> {
    std::fs::create_dir_all(path)
        .map_err(|e| DbError::Invalid(format!("create directory {}: {e}", path.display())))?;
    set_owner_only_permissions(path);
    Ok(())
}

/// Move a directory tree. Prefers `rename` (same volume). On rename failure,
/// falls back to recursive copy + delete of `src`. That fallback is **not**
/// atomic and must not be treated as cross-volume transactional safety.
fn rename_or_copy_tree(src: &Path, dest: &Path) -> DbResult<()> {
    match std::fs::rename(src, dest) {
        Ok(()) => {
            set_owner_only_permissions(dest);
            Ok(())
        }
        Err(e) => {
            if dest.exists() {
                let _ = std::fs::remove_dir_all(dest);
            }
            match copy_dir_recursive(src, dest) {
                Ok(()) => {
                    let _ = std::fs::remove_dir_all(src);
                    set_owner_only_permissions(dest);
                    Ok(())
                }
                Err(copy_err) => {
                    // Leave `src` intact; do not keep a partial destination.
                    let _ = std::fs::remove_dir_all(dest);
                    Err(DbError::Invalid(format!(
                        "asset root rename failed ({e}); copy fallback: {copy_err}"
                    )))
                }
            }
        }
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    ensure_dir_io(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dest.join(entry.file_name());
        if ty.is_symlink() {
            continue;
        }
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

fn ensure_dir_io(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

fn require_staged_dir(staged: &Path) -> DbResult<()> {
    let meta = std::fs::symlink_metadata(staged).map_err(|e| {
        DbError::Invalid(format!(
            "staged asset root missing ({}): {e}",
            staged.display()
        ))
    })?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(DbError::Invalid(format!(
            "staged asset root must be a directory ({})",
            staged.display()
        )));
    }
    Ok(())
}

/// Quarantine a live asset root and install the staged tree in its place.
///
/// Ordered per-root replacement only — not an atomic multi-root transaction,
/// and not cross-volume atomic even for a single root (copy fallback).
/// On failure after quarantine, leaves `quarantine` in place for the caller to
/// restore; does not delete `live` unless a partial install must be cleared.
fn replace_one_asset_root(staged: &Path, live: &Path, quarantine: &Path) -> DbResult<()> {
    require_staged_dir(staged)?;
    if let Some(parent) = quarantine.parent() {
        ensure_dir(parent)?;
    }
    if let Some(parent) = live.parent() {
        ensure_dir(parent)?;
    }

    if live.exists() {
        if quarantine.exists() {
            let _ = std::fs::remove_dir_all(quarantine);
        }
        rename_or_copy_tree(live, quarantine)?;
    }

    if let Err(e) = rename_or_copy_tree(staged, live) {
        // Clear a partial install; leave quarantine for caller rollback.
        let _ = std::fs::remove_dir_all(live);
        return Err(e);
    }
    Ok(())
}

/// Restore `live` from `quarantine` only when quarantine actually holds a tree.
/// If quarantine is absent, leave `live` untouched (early failure before move).
fn restore_quarantined_asset_root(live: &Path, quarantine: &Path) -> DbResult<()> {
    if !quarantine.exists() {
        return Ok(());
    }
    let _ = std::fs::remove_dir_all(live);
    rename_or_copy_tree(quarantine, live)
}

/// Replace live media/attachments trees with staged restore trees.
///
/// Prior live trees are moved into `quarantine_base` so assets absent from the
/// backup do not remain active. On failure, both roots are restored from
/// quarantine when present. Prefer a quarantine directory on the same volume;
/// cross-volume copy fallback is best-effort and non-atomic.
///
/// Failure paths keep `quarantine_base` if rollback cannot fully drain it, so a
/// mid-failure never deletes the only remaining copy of prior assets.
pub fn promote_restored_assets(
    staged_media: &Path,
    live_media: &Path,
    staged_attachments: &Path,
    live_attachments: &Path,
) -> DbResult<()> {
    let quarantine_base = live_media.parent().unwrap_or(live_media).join(format!(
        ".coreside-restore-quarantine-{}",
        uuid::Uuid::new_v4()
    ));
    replace_restored_asset_roots(
        staged_media,
        live_media,
        staged_attachments,
        live_attachments,
        &quarantine_base,
    )
}

/// Same as [`promote_restored_assets`] with an explicit quarantine base directory.
pub fn replace_restored_asset_roots(
    staged_media: &Path,
    live_media: &Path,
    staged_attachments: &Path,
    live_attachments: &Path,
    quarantine_base: &Path,
) -> DbResult<()> {
    ensure_dir(quarantine_base)?;
    let q_media = quarantine_base.join("media");
    let q_att = quarantine_base.join("attachments");

    if let Err(e) = replace_one_asset_root(staged_media, live_media, &q_media) {
        let restore_err = restore_quarantined_asset_root(live_media, &q_media).err();
        if !q_media.exists() && !q_att.exists() {
            let _ = std::fs::remove_dir_all(quarantine_base);
        }
        if let Some(restore_err) = restore_err {
            return Err(restore_err);
        }
        return Err(e);
    }
    if let Err(e) = replace_one_asset_root(staged_attachments, live_attachments, &q_att) {
        let att_err = restore_quarantined_asset_root(live_attachments, &q_att).err();
        let media_err = restore_quarantined_asset_root(live_media, &q_media).err();
        if !q_media.exists() && !q_att.exists() {
            let _ = std::fs::remove_dir_all(quarantine_base);
        }
        if let Some(restore_err) = att_err.or(media_err) {
            return Err(restore_err);
        }
        return Err(e);
    }
    // Success: drop quarantined prior trees (caller may retain a safety backup separately).
    let _ = std::fs::remove_dir_all(quarantine_base);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn archive_roundtrip_includes_media_and_attachments() {
        let dir = tempdir().unwrap();
        let media = dir.path().join("media");
        let attachments = dir.path().join("attachments");
        std::fs::create_dir_all(&media).unwrap();
        std::fs::create_dir_all(&attachments).unwrap();
        std::fs::write(media.join("a.png"), b"png-bytes").unwrap();
        std::fs::write(attachments.join("b.txt"), b"hello").unwrap();

        let src = Database::open_path(&dir.path().join("src.db")).unwrap();
        src.conn()
            .execute(
                "INSERT INTO media_assets
                 (id, category, title, local_filename, mime_type, byte_size, content_hash, validation_status)
                 VALUES ('m1', 'image', 'a', 'a.png', 'image/png', 9, 'hash', 'ok')",
                [],
            )
            .unwrap();
        src.conn()
            .execute(
                "INSERT INTO chat_attachments
                 (id, storage_key, original_filename, display_name, detected_mime, detected_format,
                  byte_size, content_hash, state, backup_eligible)
                 VALUES ('a1', 'b.txt', 'b.txt', 'b.txt', 'text/plain', 'text/plain', 5, 'hash', 'attached', 1)",
                [],
            )
            .unwrap();
        let archive = dir.path().join("profile.coreside-backup");
        let manifest = src
            .create_profile_archive_with_assets(&archive, "0.1.0", Some(&media), Some(&attachments))
            .unwrap();
        assert_eq!(manifest.schema_version, BACKUP_SCHEMA_VERSION);
        assert!(manifest.media.included);
        assert_eq!(manifest.media.count, 1);
        assert_eq!(manifest.media.entries.len(), 1);
        assert_eq!(manifest.media.entries[0].name, "a.png");
        assert_eq!(manifest.media.entries[0].sha256.len(), 64);
        assert_eq!(manifest.media.completeness, "complete");
        assert!(manifest.attachments.included);
        assert_eq!(manifest.attachments.count, 1);
        assert_eq!(manifest.attachments.entries.len(), 1);
        assert_eq!(manifest.attachments.completeness, "complete");

        let preview = preview_profile_archive(&archive).unwrap();
        assert!(preview.integrity_ok);
        assert_eq!(preview.media_count, 1);
        assert_eq!(preview.attachment_count, 1);

        let out_media = dir.path().join("out-media");
        let out_att = dir.path().join("out-att");
        let (m, a) = extract_assets_from_archive(&archive, &out_media, &out_att).unwrap();
        assert_eq!((m, a), (1, 1));
        assert_eq!(
            std::fs::read(out_media.join("a.png")).unwrap(),
            b"png-bytes"
        );
        assert_eq!(std::fs::read(out_att.join("b.txt")).unwrap(), b"hello");
    }

    #[test]
    fn archive_skips_orphans_when_attachment_table_empty() {
        let dir = tempdir().unwrap();
        let media = dir.path().join("media");
        let attachments = dir.path().join("attachments");
        std::fs::create_dir_all(&media).unwrap();
        std::fs::create_dir_all(&attachments).unwrap();
        std::fs::write(media.join("orphan.png"), b"x").unwrap();
        std::fs::write(attachments.join("orphan.txt"), b"y").unwrap();
        let src = Database::open_path(&dir.path().join("src.db")).unwrap();
        let archive = dir.path().join("profile.coreside-backup");
        let manifest = src
            .create_profile_archive_with_assets(&archive, "0.1.0", Some(&media), Some(&attachments))
            .unwrap();
        assert_eq!(manifest.media.count, 0);
        assert_eq!(manifest.attachments.count, 0);
        assert_eq!(manifest.media.completeness, "complete");
        assert_eq!(manifest.attachments.completeness, "complete");
    }

    #[test]
    fn archive_excludes_staged_not_backup_eligible() {
        let dir = tempdir().unwrap();
        let media = dir.path().join("media");
        let attachments = dir.path().join("attachments");
        let staging = attachments.join("staging");
        std::fs::create_dir_all(&media).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("draft.txt"), b"draft").unwrap();
        let src = Database::open_path(&dir.path().join("src.db")).unwrap();
        src.conn()
            .execute(
                "INSERT INTO chat_attachments
                 (id, storage_key, original_filename, display_name, detected_mime, detected_format,
                  byte_size, content_hash, state, backup_eligible)
                 VALUES ('a1', 'draft.txt', 'draft.txt', 'draft.txt', 'text/plain', 'text/plain', 5, 'hash', 'staged', 0)",
                [],
            )
            .unwrap();
        let archive = dir.path().join("profile.coreside-backup");
        let manifest = src
            .create_profile_archive_with_assets(&archive, "0.1.0", Some(&media), Some(&attachments))
            .unwrap();
        assert_eq!(manifest.attachments.count, 0);
    }

    #[test]
    fn promote_moves_staged_assets_into_live_roots() {
        let dir = tempdir().unwrap();
        let staged_media = dir.path().join("staged-media");
        let staged_att = dir.path().join("staged-att");
        let live_media = dir.path().join("live-media");
        let live_att = dir.path().join("live-att");
        std::fs::create_dir_all(&staged_media).unwrap();
        std::fs::create_dir_all(&staged_att).unwrap();
        std::fs::write(staged_media.join("a.png"), b"png").unwrap();
        std::fs::write(staged_att.join("b.txt"), b"txt").unwrap();
        promote_restored_assets(&staged_media, &live_media, &staged_att, &live_att).unwrap();
        assert_eq!(std::fs::read(live_media.join("a.png")).unwrap(), b"png");
        assert_eq!(std::fs::read(live_att.join("b.txt")).unwrap(), b"txt");
    }

    #[test]
    fn promote_replaces_entire_live_tree_and_drops_stale_assets() {
        let dir = tempdir().unwrap();
        let staged_media = dir.path().join("staged-media");
        let staged_att = dir.path().join("staged-att");
        let live_media = dir.path().join("live-media");
        let live_att = dir.path().join("live-att");
        let quarantine = dir.path().join("q");
        std::fs::create_dir_all(&staged_media).unwrap();
        std::fs::create_dir_all(&staged_att).unwrap();
        std::fs::create_dir_all(&live_media).unwrap();
        std::fs::create_dir_all(&live_att).unwrap();
        std::fs::write(live_media.join("stale.png"), b"old").unwrap();
        std::fs::write(live_att.join("stale.txt"), b"old").unwrap();
        std::fs::write(staged_media.join("a.png"), b"new-a").unwrap();
        std::fs::write(staged_att.join("b.txt"), b"new-b").unwrap();
        replace_restored_asset_roots(
            &staged_media,
            &live_media,
            &staged_att,
            &live_att,
            &quarantine,
        )
        .unwrap();
        assert_eq!(std::fs::read(live_media.join("a.png")).unwrap(), b"new-a");
        assert_eq!(std::fs::read(live_att.join("b.txt")).unwrap(), b"new-b");
        assert!(!live_media.join("stale.png").exists());
        assert!(!live_att.join("stale.txt").exists());
        assert!(!quarantine.exists());
    }

    #[test]
    fn promote_failure_restores_quarantined_media_root() {
        let dir = tempdir().unwrap();
        let staged_media = dir.path().join("staged-media");
        let staged_att = dir.path().join("staged-att");
        let live_media = dir.path().join("live-media");
        let live_att = dir.path().join("live-att");
        let quarantine = dir.path().join("q");
        std::fs::create_dir_all(&staged_media).unwrap();
        std::fs::create_dir_all(&live_media).unwrap();
        std::fs::write(live_media.join("keep.png"), b"original").unwrap();
        std::fs::write(staged_media.join("a.png"), b"new-a").unwrap();
        // File at staged_att path: attachments replace fails after media already swapped.
        std::fs::write(&staged_att, b"not-a-directory").unwrap();
        let err = replace_restored_asset_roots(
            &staged_media,
            &live_media,
            &staged_att,
            &live_att,
            &quarantine,
        );
        assert!(err.is_err());
        assert_eq!(
            std::fs::read(live_media.join("keep.png")).unwrap(),
            b"original",
            "media root must roll back when attachments replace fails"
        );
        assert!(!live_media.join("a.png").exists());
    }

    #[test]
    fn promote_early_media_failure_leaves_live_untouched() {
        let dir = tempdir().unwrap();
        let staged_media = dir.path().join("staged-media");
        let staged_att = dir.path().join("staged-att");
        let live_media = dir.path().join("live-media");
        let live_att = dir.path().join("live-att");
        let quarantine = dir.path().join("q");
        std::fs::create_dir_all(&staged_att).unwrap();
        std::fs::create_dir_all(&live_media).unwrap();
        std::fs::create_dir_all(&live_att).unwrap();
        std::fs::write(live_media.join("keep.png"), b"original").unwrap();
        std::fs::write(live_att.join("keep.txt"), b"att-original").unwrap();
        std::fs::write(staged_att.join("b.txt"), b"new-b").unwrap();
        // staged_media is a file → fail before quarantining live media
        std::fs::write(&staged_media, b"not-a-directory").unwrap();
        let err = replace_restored_asset_roots(
            &staged_media,
            &live_media,
            &staged_att,
            &live_att,
            &quarantine,
        );
        assert!(err.is_err());
        assert_eq!(
            std::fs::read(live_media.join("keep.png")).unwrap(),
            b"original"
        );
        assert_eq!(
            std::fs::read(live_att.join("keep.txt")).unwrap(),
            b"att-original"
        );
        assert!(!live_att.join("b.txt").exists());
    }

    #[test]
    fn archive_roundtrip_preview() {
        let dir = tempdir().unwrap();
        let src = Database::open_path(&dir.path().join("src.db")).unwrap();
        let archive = dir.path().join("profile.coreside-backup");
        let manifest = src.create_profile_archive(&archive, "0.1.0").unwrap();
        assert_eq!(manifest.format, BACKUP_FORMAT);
        let preview = preview_profile_archive(&archive).unwrap();
        assert!(preview.integrity_ok);
        assert_eq!(preview.application_version, "0.1.0");

        let extracted = dir.path().join("restored.db");
        extract_database_from_archive(&archive, &extracted).unwrap();
        let opened = Database::open_path(&extracted).unwrap();
        assert_eq!(opened.quick_check().unwrap(), "ok");
    }

    #[test]
    fn rejects_invalid_zip() {
        let dir = tempdir().unwrap();
        let bogus = dir.path().join("bad.zip");
        std::fs::write(&bogus, b"not a zip").unwrap();
        assert!(preview_profile_archive(&bogus).is_err());
    }

    #[test]
    fn rejects_unsafe_entry_names() {
        assert!(!is_safe_archive_entry_name("../etc/passwd"));
        assert!(!is_safe_archive_entry_name("/abs/path"));
        assert!(!is_safe_archive_entry_name("database/../../x"));
        assert!(is_safe_archive_entry_name("manifest.json"));
        assert!(is_safe_archive_entry_name("database/coreside.db"));
    }

    #[test]
    fn rejects_size_mismatch_in_manifest() {
        let dir = tempdir().unwrap();
        let src = Database::open_path(&dir.path().join("src.db")).unwrap();
        let archive = dir.path().join("profile.coreside-backup");
        let mut manifest = src.create_profile_archive(&archive, "0.1.0").unwrap();
        // Tamper: rewrite zip with wrong byte_size but matching hash requires rebuilding.
        // Instead open archive, rebuild manifest with wrong size, same hash of real db.
        let db_bytes = {
            let file = File::open(&archive).unwrap();
            let mut zip = ZipArchive::new(file).unwrap();
            let mut entry = zip.by_name("database/coreside.db").unwrap();
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).unwrap();
            buf
        };
        manifest.database.byte_size = manifest.database.byte_size.saturating_add(1);
        let tampered = dir.path().join("tampered.coreside-backup");
        {
            let file = File::create(&tampered).unwrap();
            let mut zip = ZipWriter::new(file);
            let opts =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("manifest.json", opts).unwrap();
            zip.write_all(&serde_json::to_vec_pretty(&manifest).unwrap())
                .unwrap();
            zip.start_file("database/coreside.db", opts).unwrap();
            zip.write_all(&db_bytes).unwrap();
            zip.finish().unwrap();
        }
        let err = preview_profile_archive(&tampered).unwrap_err().to_string();
        assert!(err.contains("size mismatch"), "{err}");
    }
}
