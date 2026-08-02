//! Versioned `.coreside-backup` profile archives (SQLite snapshot + manifest).

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use super::{Database, DbError, DbResult};

pub const BACKUP_FORMAT: &str = "coreside-backup";
pub const BACKUP_SCHEMA_VERSION: u32 = 1;
/// Hard ceiling for a database snapshot inside a backup (ZIP bomb / OOM guard).
/// ponytail: raise only with streaming restore UX; media is not embedded yet.
const MAX_ARCHIVE_DB_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

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
pub struct BackupAssetSection {
    pub included: bool,
    pub count: u64,
    pub total_bytes: u64,
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

fn copy_limited_hashed(reader: &mut impl Read, dest: &Path, max_bytes: u64) -> DbResult<(u64, String)> {
    let mut out = File::create(dest)
        .map_err(|e| DbError::Invalid(format!("create extract dest: {e}")))?;
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
        out.write_all(&buf[..n])
            .map_err(|e| DbError::Invalid(format!("write extract dest: {e}")))?;
    }
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
        chrono::Utc::now().timestamp_millis()
    ))
}

impl Database {
    /// Create a versioned `.coreside-backup` archive containing a consistent DB snapshot.
    /// Media/attachments inclusion is staged; current phase embeds empty asset sections.
    pub fn create_profile_archive(
        &self,
        dest: &Path,
        application_version: &str,
    ) -> DbResult<BackupManifest> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DbError::Invalid(format!("Failed to create backup dir: {e}")))?;
        }
        let staging_dir = dest
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(
                ".tmp-backup-{}",
                chrono::Utc::now().timestamp_millis()
            ));
        std::fs::create_dir_all(&staging_dir)
            .map_err(|e| DbError::Invalid(format!("Failed to create staging: {e}")))?;
        let db_snap = staging_dir.join("coreside.db");
        let result = (|| {
            self.snapshot_to_path(&db_snap)?;
            let snap = Database::open_path(&db_snap)?;
            let quick = snap.quick_check()?;
            let fk = snap.foreign_key_check_summary()?;
            let latest = snap.latest_migration_name()?;
            drop(snap);
            let byte_size = std::fs::metadata(&db_snap)
                .map(|m| m.len())
                .map_err(|e| DbError::Invalid(format!("snapshot stat failed: {e}")))?;
            let hash = sha256_file(&db_snap)?;
            let manifest = BackupManifest {
                format: BACKUP_FORMAT.into(),
                schema_version: BACKUP_SCHEMA_VERSION,
                application_version: application_version.into(),
                created_at: chrono::Utc::now().to_rfc3339(),
                database: BackupDatabaseEntry {
                    path: "database/coreside.db".into(),
                    sha256: hash,
                    byte_size,
                    latest_migration: latest,
                },
                media: BackupAssetSection {
                    included: false,
                    count: 0,
                    total_bytes: 0,
                },
                attachments: BackupAssetSection {
                    included: false,
                    count: 0,
                    total_bytes: 0,
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
                return Err(DbError::Invalid(
                    "Snapshot failed foreign_key_check".into(),
                ));
            }

            let tmp_zip = staging_dir.join("archive.zip");
            {
                let file = File::create(&tmp_zip)
                    .map_err(|e| DbError::Invalid(format!("zip create failed: {e}")))?;
                let mut zip = ZipWriter::new(file);
                let opts = SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                zip.start_file("manifest.json", opts)
                    .map_err(|e| DbError::Invalid(format!("zip manifest entry: {e}")))?;
                let body = serde_json::to_vec_pretty(&manifest)
                    .map_err(|e| DbError::Invalid(format!("manifest serialize: {e}")))?;
                zip.write_all(&body)
                    .map_err(|e| DbError::Invalid(format!("write manifest: {e}")))?;
                zip.start_file("database/coreside.db", opts)
                    .map_err(|e| DbError::Invalid(format!("zip db entry: {e}")))?;
                let mut db_file = File::open(&db_snap)
                    .map_err(|e| DbError::Invalid(format!("open snapshot: {e}")))?;
                std::io::copy(&mut db_file, &mut zip)
                    .map_err(|e| DbError::Invalid(format!("copy db into zip: {e}")))?;
                zip.finish()
                    .map_err(|e| DbError::Invalid(format!("zip finish: {e}")))?;
            }

            // Validate by reopening (recomputes integrity; does not trust manifest alone).
            let _preview = preview_profile_archive(&tmp_zip)?;
            std::fs::rename(&tmp_zip, dest)
                .map_err(|e| DbError::Invalid(format!("finalize backup rename: {e}")))?;
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
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| DbError::Invalid(format!("zip entry: {e}")))?;
            let name = entry.name().to_string();
            if !is_safe_archive_entry_name(&name) || entry.enclosed_name().is_none() {
                return Err(DbError::Invalid("archive path traversal rejected".into()));
            }
            if name == "manifest.json" {
                let buf = read_limited_string(&mut entry, MAX_MANIFEST_BYTES)?;
                manifest = Some(
                    serde_json::from_str(&buf)
                        .map_err(|e| DbError::Invalid(format!("manifest parse: {e}")))?,
                );
            } else if name == "database/coreside.db" {
                let (byte_size, hash) =
                    copy_limited_hashed(&mut entry, &tmp_db, MAX_ARCHIVE_DB_BYTES)?;
                db_extracted = Some((tmp_db.clone(), byte_size, hash));
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
        if !manifest.media.included {
            warnings.push("Media files were not included in this backup.".into());
        }
        if !manifest.attachments.included {
            warnings.push("Chat attachments were not included in this backup.".into());
        }
        if !integrity_ok {
            warnings.push("Database integrity checks did not pass for this backup.".into());
        }
        if manifest.integrity.database_quick_check != quick
            || manifest.integrity.foreign_key_check != fk
        {
            warnings.push("Backup manifest integrity fields did not match recomputed checks.".into());
        }

        Ok(RestorePreview {
            format: manifest.format,
            schema_version: manifest.schema_version,
            application_version: manifest.application_version,
            created_at: manifest.created_at,
            latest_migration: manifest.database.latest_migration,
            database_bytes: byte_size,
            media_count: manifest.media.count,
            attachment_count: manifest.attachments.count,
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
        return Err(DbError::Invalid("extracted database integrity mismatch".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
            let opts = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
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
