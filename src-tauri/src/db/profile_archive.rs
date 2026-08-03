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

fn copy_limited_hashed(reader: &mut impl Read, dest: &Path, max_bytes: u64) -> DbResult<(u64, String)> {
    let parent = dest.parent().ok_or_else(|| {
        DbError::Invalid("extract destination has no parent".into())
    })?;
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

fn collect_named_files(root: &Path, max_count: u64, max_total: u64) -> DbResult<(Vec<(String, PathBuf, u64)>, u64, Vec<String>)> {
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut warnings = Vec::new();
    if !root.exists() {
        return Ok((files, total, warnings));
    }
    let entries = std::fs::read_dir(root)
        .map_err(|e| DbError::Invalid(format!("read asset root: {e}")))?;
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
        if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
            warnings.push(format!("skipped unsafe asset name in {}", root.file_name().and_then(|s| s.to_str()).unwrap_or("assets")));
            continue;
        }
        let size = meta.len();
        if size > MAX_ARCHIVE_ASSET_BYTES {
            warnings.push(format!("skipped oversized asset ({name})"));
            continue;
        }
        if files.len() as u64 >= max_count {
            warnings.push("asset count limit reached; remaining files omitted".into());
            break;
        }
        if total.saturating_add(size) > max_total {
            warnings.push("asset byte budget reached; remaining files omitted".into());
            break;
        }
        total = total.saturating_add(size);
        files.push((name.to_string(), entry.path(), size));
    }
    Ok((files, total, warnings))
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
        let staging_dir = dest.parent().unwrap_or_else(|| Path::new(".")).join(format!(
            ".tmp-backup-{}",
            uuid::Uuid::new_v4()
        ));
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
            drop(snap);
            let byte_size = std::fs::metadata(&db_snap)
                .map(|m| m.len())
                .map_err(|e| DbError::Invalid(format!("snapshot stat failed: {e}")))?;
            let hash = sha256_file(&db_snap)?;

            let mut media_files = Vec::new();
            let mut media_bytes = 0u64;
            let mut attachment_files = Vec::new();
            let mut attachment_bytes = 0u64;
            let mut asset_warnings = Vec::new();
            if let Some(root) = media_root {
                let (files, total, warnings) =
                    collect_named_files(root, MAX_ARCHIVE_ASSET_COUNT, MAX_ARCHIVE_TOTAL_ASSET_BYTES)?;
                media_files = files;
                media_bytes = total;
                asset_warnings.extend(warnings);
            }
            if let Some(root) = attachments_root {
                let remaining_count =
                    MAX_ARCHIVE_ASSET_COUNT.saturating_sub(media_files.len() as u64);
                let remaining_bytes =
                    MAX_ARCHIVE_TOTAL_ASSET_BYTES.saturating_sub(media_bytes);
                let (files, total, warnings) =
                    collect_named_files(root, remaining_count, remaining_bytes)?;
                attachment_files = files;
                attachment_bytes = total;
                asset_warnings.extend(warnings);
            }
            let _ = asset_warnings; // reserved for future manifest warnings field

            let schema_version = if include_assets {
                BACKUP_SCHEMA_VERSION
            } else {
                BACKUP_SCHEMA_VERSION_V1
            };

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
                },
                attachments: BackupAssetSection {
                    included: include_assets,
                    count: attachment_files.len() as u64,
                    total_bytes: attachment_bytes,
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

                for (name, path, _) in &media_files {
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
                for (name, path, _) in &attachment_files {
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
            warnings.push("Media files were not included in this backup (partial-profile / legacy).".into());
        }
        if manifest.schema_version <= BACKUP_SCHEMA_VERSION_V1 || !manifest.attachments.included {
            warnings.push("Chat attachments were not included in this backup (partial-profile / legacy).".into());
        }
        if manifest.media.count != media_count || manifest.attachments.count != attachment_count {
            warnings.push(
                "Backup manifest asset counts did not match archive entries.".into(),
            );
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
        return Err(DbError::Invalid("extracted database integrity mismatch".into()));
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
            let file_name = dest
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("asset");
            let quarantine = dest.with_file_name(format!(
                "{file_name}.pre-restore-{}",
                uuid::Uuid::new_v4()
            ));
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
        let (bytes, _hash) = copy_limited_hashed(&mut entry, &dest, per_file_cap)?;
        if bytes == 0 {
            let _ = std::fs::remove_file(&dest);
            continue;
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

fn promote_one_root(staging: &Path, live: &Path) -> DbResult<()> {
    if !staging.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(live)
        .map_err(|e| DbError::Invalid(format!("create live asset root: {e}")))?;
    set_owner_only_permissions(live);
    let entries = std::fs::read_dir(staging)
        .map_err(|e| DbError::Invalid(format!("read staged assets: {e}")))?;
    for entry in entries {
        let entry = entry.map_err(|e| DbError::Invalid(format!("staged asset entry: {e}")))?;
        let meta = std::fs::symlink_metadata(entry.path())
            .map_err(|e| DbError::Invalid(format!("staged asset metadata: {e}")))?;
        if meta.file_type().is_symlink() || !meta.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.is_empty()
            || name_str.contains("..")
            || name_str.contains('/')
            || name_str.contains('\\')
            || name_str.contains('\0')
        {
            return Err(DbError::Invalid("unsafe staged asset name".into()));
        }
        let dest = live.join(name.as_os_str());
        if dest.exists() {
            let quarantine = dest.with_file_name(format!(
                "{name_str}.pre-restore-{}",
                uuid::Uuid::new_v4()
            ));
            let _ = std::fs::rename(&dest, &quarantine);
        }
        if let Err(e) = std::fs::rename(entry.path(), &dest) {
            // Cross-device fallback.
            std::fs::copy(entry.path(), &dest)
                .map_err(|copy_err| {
                    DbError::Invalid(format!("promote asset failed ({e}); copy: {copy_err}"))
                })?;
            let _ = std::fs::remove_file(entry.path());
        }
        set_owner_only_permissions(&dest);
    }
    Ok(())
}

/// Move staged restore assets into live media/attachments roots.
pub fn promote_restored_assets(
    staged_media: &Path,
    live_media: &Path,
    staged_attachments: &Path,
    live_attachments: &Path,
) -> DbResult<()> {
    promote_one_root(staged_media, live_media)?;
    promote_one_root(staged_attachments, live_attachments)?;
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
        let archive = dir.path().join("profile.coreside-backup");
        let manifest = src
            .create_profile_archive_with_assets(&archive, "0.1.0", Some(&media), Some(&attachments))
            .unwrap();
        assert_eq!(manifest.schema_version, BACKUP_SCHEMA_VERSION);
        assert!(manifest.media.included);
        assert_eq!(manifest.media.count, 1);
        assert!(manifest.attachments.included);
        assert_eq!(manifest.attachments.count, 1);

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
