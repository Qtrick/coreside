//! Data integrity and backup-related settings commands.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::CommandError;
use crate::app_paths::AppPaths;
use crate::automations::SchedulerHandle;
use crate::db::{preview_profile_archive, DatabaseHealthReport, RestorePreview};
use crate::state::AppState;

#[tauri::command]
pub fn get_database_health(state: State<'_, AppState>) -> Result<DatabaseHealthReport, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(db.health_report()?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreated {
    pub path: String,
    pub byte_size: u64,
    pub schema_version: Option<String>,
    pub format: String,
}

#[tauri::command]
pub fn create_profile_backup(state: State<'_, AppState>) -> Result<BackupCreated, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let paths = AppPaths::resolve().map_err(|e| {
        CommandError::new("storage_unavailable", e.to_string())
    })?;
    paths.ensure_dirs().map_err(|e| {
        CommandError::new("storage_unavailable", e.to_string())
    })?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let dest = paths
        .backups
        .join(format!("coreside-profile-{stamp}.coreside-backup"));
    let version = env!("CARGO_PKG_VERSION");
    let manifest = db.create_profile_archive_with_assets(
        &dest,
        version,
        Some(&paths.media),
        Some(&paths.attachments),
    )?;
    let archive_bytes = std::fs::metadata(&dest)
        .map(|m| m.len())
        .map_err(|e| {
            CommandError::new(
                "export_failed",
                format!("Backup written but unreadable: {e}"),
            )
        })?;
    Ok(BackupCreated {
        path: dest
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("backup.coreside-backup")
            .to_string(),
        byte_size: archive_bytes,
        schema_version: manifest.database.latest_migration,
        format: manifest.format,
    })
}

#[tauri::command]
pub fn preview_restore_backup(
    _state: State<'_, AppState>,
    path: String,
) -> Result<RestorePreview, CommandError> {
    // Recovery-safe: preview must work when the profile cannot open.
    let (_paths, resolved_canon) = resolve_managed_backup(&path)?;
    Ok(preview_profile_archive(&resolved_canon).map_err(|e| match e {
        crate::db::DbError::Invalid(msg) => CommandError::new("archive_invalid", msg),
        other => CommandError::from(other),
    })?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedBackupEntry {
    pub path: String,
    pub byte_size: u64,
}

/// List managed backup filenames (recovery-safe).
#[tauri::command]
pub fn list_managed_backups(
    _state: State<'_, AppState>,
) -> Result<Vec<ManagedBackupEntry>, CommandError> {
    let paths = AppPaths::resolve().map_err(|e| {
        CommandError::new("storage_unavailable", e.to_string())
    })?;
    paths.ensure_dirs().map_err(|e| {
        CommandError::new("storage_unavailable", e.to_string())
    })?;
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&paths.backups).map_err(|e| {
        CommandError::new("storage_unavailable", format!("Cannot read backups: {e}"))
    })?;
    for entry in entries.flatten() {
        // Do not follow symlinks — list only real files inside the managed backups root.
        let meta = std::fs::symlink_metadata(entry.path()).ok();
        let Some(meta) = meta else { continue };
        if meta.file_type().is_symlink() || !meta.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".coreside-backup") {
            continue;
        }
        out.push(ManagedBackupEntry {
            path: name.to_string(),
            byte_size: meta.len(),
        });
    }
    out.sort_by(|a, b| b.path.cmp(&a.path));
    Ok(out)
}

fn resolve_managed_backup(
    path: &str,
) -> Result<(crate::app_paths::AppPaths, std::path::PathBuf), CommandError> {
    let paths = AppPaths::resolve().map_err(|e| {
        CommandError::new("storage_unavailable", e.to_string())
    })?;
    paths.ensure_dirs().map_err(|e| {
        CommandError::new("storage_unavailable", e.to_string())
    })?;
    if path.trim().is_empty() {
        return Err(CommandError::new(
            "archive_invalid",
            "Backup path is required.",
        ));
    }
    let candidate = std::path::PathBuf::from(path);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        let name = candidate
            .file_name()
            .ok_or_else(|| CommandError::new("archive_invalid", "Backup path is invalid."))?;
        paths.backups.join(name)
    };
    let backups_canon = paths.backups.canonicalize().map_err(|e| {
        CommandError::new(
            "storage_unavailable",
            format!("Backups folder is unavailable: {e}"),
        )
    })?;
    let resolved_canon = resolved.canonicalize().map_err(|_| {
        CommandError::new("archive_invalid", "Backup file could not be opened.")
    })?;
    if !resolved_canon.starts_with(&backups_canon) {
        return Err(CommandError::new(
            "permission_denied",
            "Restore is limited to managed backups.",
        ));
    }
    Ok((paths, resolved_canon))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreApplied {
    pub safety_backup_path: String,
    pub restored_from: String,
    pub restart_recommended: bool,
}

fn db_sidecar(path: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}{suffix}", path.display()))
}

fn remove_db_sidecars(path: &std::path::Path) {
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(db_sidecar(path, suffix));
    }
}

/// Move live DB main + WAL/SHM into `quarantine_dir`. On partial failure, moves already
/// completed are rolled back before returning the error.
fn quarantine_db_tree(
    live_path: &std::path::Path,
    quarantine_dir: &std::path::Path,
) -> Result<Vec<(std::path::PathBuf, std::path::PathBuf)>, CommandError> {
    let mut moved = Vec::new();
    for suffix in ["", "-wal", "-shm"] {
        let src = db_sidecar(live_path, suffix);
        if !src.exists() {
            continue;
        }
        let dest = quarantine_dir.join(
            src.file_name()
                .map(|s| s.to_owned())
                .unwrap_or_else(|| std::ffi::OsString::from("coreside.db")),
        );
        if let Err(e) = std::fs::rename(&src, &dest) {
            restore_quarantined_tree(&moved);
            return Err(CommandError::new(
                "restore_failed",
                format!("Could not quarantine live database: {e}"),
            ));
        }
        moved.push((src, dest));
    }
    Ok(moved)
}

fn restore_quarantined_tree(moved: &[(std::path::PathBuf, std::path::PathBuf)]) {
    for (src, dest) in moved.iter().rev() {
        let _ = std::fs::rename(dest, src);
    }
}

fn try_reopen_live_into(
    db_guard: &mut crate::db::Database,
    live_path: &std::path::Path,
) -> bool {
    match crate::db::Database::open_path(live_path) {
        Ok(orig) => {
            *db_guard = orig;
            true
        }
        Err(err) => {
            tracing::error!(
                error = %err,
                path = %live_path.display(),
                "failed to reopen live database after restore rollback"
            );
            false
        }
    }
}

fn mark_restore_rollback_recovery(state: &AppState, live_path: &std::path::Path) {
    *state.bootstrap.lock() = crate::db::BootstrapStatus::recovery(
        "restore_rollback_failed",
        "Restore did not finish and the previous profile could not be reopened. Restart Coreside and use Recovery.",
        Some(live_path.to_path_buf()),
        true,
    );
}

/// After a failed restore swap: reopen the live profile, or a fresh recovery shell.
/// Never leave a staging/hold database attached while reporting Recovery.
fn recover_connection_after_failed_restore(
    state: &AppState,
    db_guard: &mut crate::db::Database,
    live_path: &std::path::Path,
) {
    if try_reopen_live_into(db_guard, live_path) {
        return;
    }
    match crate::db::bootstrap::open_shell_database() {
        Ok(shell) => {
            *db_guard = shell;
            mark_restore_rollback_recovery(state, live_path);
        }
        Err(err) => {
            tracing::error!(
                error = %err,
                path = %live_path.display(),
                "failed to open recovery shell after restore rollback"
            );
            mark_restore_rollback_recovery(state, live_path);
        }
    }
}

/// Restore a validated managed backup.
/// Works from a healthy profile **or** bootstrap recovery (disaster restore).
/// Requires explicit `confirm: true`. Main protected UI only.
#[tauri::command]
pub fn restore_profile_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    confirm: bool,
) -> Result<RestoreApplied, CommandError> {
    if !confirm {
        return Err(CommandError::new(
            "validation_failed",
            "Restore requires explicit confirmation.",
        ));
    }
    // Intentionally no require_profile(): disaster recovery must restore when the profile cannot open.
    let started_without_profile = !state.profile_ready();
    state.cancel_all();

    let (paths, archive_path) = resolve_managed_backup(&path)?;
    let preview = preview_profile_archive(&archive_path).map_err(|e| match e {
        crate::db::DbError::Invalid(msg) => CommandError::new("archive_invalid", msg),
        other => CommandError::from(other),
    })?;
    if !preview.integrity_ok {
        return Err(CommandError::new(
            "archive_invalid",
            "Backup failed integrity checks.",
        ));
    }

    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let version = env!("CARGO_PKG_VERSION");
    let safety_backup_path = if !started_without_profile {
        let safety_dest = paths.backups.join(format!(
            "coreside-safety-before-restore-{stamp}.coreside-backup"
        ));
        {
            let db = state.db.lock();
            db.create_profile_archive_with_assets(
                &safety_dest,
                version,
                Some(&paths.media),
                Some(&paths.attachments),
            )?;
        }
        safety_dest
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("safety.coreside-backup")
            .to_string()
    } else {
        // Broken profile: do not snapshot the recovery shell as a safety backup.
        "none-recovery-restore".to_string()
    };

    let staging_db = paths.restore_staging.join(format!("restore-{stamp}.db"));
    let asset_media_staging = paths
        .restore_staging
        .join(format!("assets-media-{stamp}"));
    let asset_att_staging = paths.restore_staging.join(format!("assets-att-{stamp}"));
    let cleanup_staging = |staging: &std::path::Path, hold: Option<&std::path::Path>| {
        let _ = std::fs::remove_file(staging);
        remove_db_sidecars(staging);
        if let Some(hold_path) = hold {
            let _ = std::fs::remove_file(hold_path);
            remove_db_sidecars(hold_path);
        }
    };
    let cleanup_asset_staging = || {
        let _ = std::fs::remove_dir_all(&asset_media_staging);
        let _ = std::fs::remove_dir_all(&asset_att_staging);
    };

    if let Err(e) = crate::db::extract_database_from_archive(&archive_path, &staging_db) {
        cleanup_staging(&staging_db, None);
        return Err(match e {
            crate::db::DbError::Invalid(msg) => CommandError::new("restore_failed", msg),
            other => CommandError::from(other),
        });
    }

    // Validate staged database, then seal WAL so the install copy is self-contained.
    {
        let staged = match crate::db::Database::open_path(&staging_db) {
            Ok(db) => db,
            Err(e) => {
                cleanup_staging(&staging_db, None);
                return Err(CommandError::new(
                    "restore_failed",
                    format!("Staged database could not open: {e}"),
                ));
            }
        };
        let quick = match staged.quick_check() {
            Ok(v) => v,
            Err(e) => {
                drop(staged);
                cleanup_staging(&staging_db, None);
                return Err(CommandError::from(e));
            }
        };
        if quick != "ok" {
            drop(staged);
            cleanup_staging(&staging_db, None);
            return Err(CommandError::new(
                "restore_failed",
                "Staged database failed quick_check.",
            ));
        }
        let fk = match staged.foreign_key_check_summary() {
            Ok(v) => v,
            Err(e) => {
                drop(staged);
                cleanup_staging(&staging_db, None);
                return Err(CommandError::from(e));
            }
        };
        if fk != "ok" {
            drop(staged);
            cleanup_staging(&staging_db, None);
            return Err(CommandError::new(
                "restore_failed",
                "Staged database failed foreign_key_check.",
            ));
        }
        if let Err(e) = staged.conn().execute_batch("PRAGMA wal_checkpoint(TRUNCATE);") {
            drop(staged);
            cleanup_staging(&staging_db, None);
            return Err(CommandError::new(
                "restore_failed",
                format!("Staged WAL checkpoint failed: {e}"),
            ));
        }
    }
    remove_db_sidecars(&staging_db);

    // Always restore into the canonical profile database path — never into the recovery shell DB.
    let live_path = paths.database.clone();
    if live_path.starts_with(&paths.recovery)
        || live_path
            .file_name()
            .is_some_and(|n| n == std::ffi::OsStr::new("shell.db"))
    {
        cleanup_staging(&staging_db, None);
        return Err(CommandError::new(
            "restore_failed",
            "Refusing to restore into the recovery shell database.",
        ));
    }
    if let Some(parent) = live_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            cleanup_staging(&staging_db, None);
            CommandError::new(
                "restore_failed",
                format!("Could not create profile directory: {e}"),
            )
        })?;
    }

    let quarantine = paths.quarantine.join(format!("pre-restore-{stamp}"));
    std::fs::create_dir_all(&quarantine).map_err(|e| {
        cleanup_staging(&staging_db, None);
        CommandError::new("restore_failed", format!("Could not create quarantine: {e}"))
    })?;

    // Hold connection on a *copy* of the sealed staged DB so live files unlock for rename.
    let hold_db = paths.restore_staging.join(format!("hold-{stamp}.db"));
    if let Err(e) = std::fs::copy(&staging_db, &hold_db) {
        cleanup_staging(&staging_db, None);
        cleanup_asset_staging();
        return Err(CommandError::new(
            "restore_failed",
            format!("Could not stage hold copy: {e}"),
        ));
    }

    // Extract media/attachments into staging BEFORE committing the DB swap so a bad
    // archive cannot leave a restored profile with incomplete live assets.
    if let Err(e) = crate::db::extract_assets_from_archive(
        &archive_path,
        &asset_media_staging,
        &asset_att_staging,
    ) {
        cleanup_staging(&staging_db, Some(&hold_db));
        cleanup_asset_staging();
        return Err(match e {
            crate::db::DbError::Invalid(msg) => CommandError::new("restore_failed", msg),
            other => CommandError::from(other),
        });
    }

    // Critical section: keep the profile mutex held for quarantine + install + reopen so
    // concurrent commands cannot observe the temporary hold database or a half-swapped tree.
    // ponytail: global db lock for the filesystem swap; upgrade to a dedicated restore latch if
    // lock hold time becomes a UX problem.
    let quarantined = {
        let mut db_guard = state.db.lock();
        let hold = crate::db::Database::open_path(&hold_db).map_err(|e| {
            cleanup_staging(&staging_db, Some(&hold_db));
            cleanup_asset_staging();
            CommandError::new("restore_failed", format!("Could not open hold DB: {e}"))
        })?;
        *db_guard = hold;

        let quarantined = match quarantine_db_tree(&live_path, &quarantine) {
            Ok(moved) => moved,
            Err(err) => {
                recover_connection_after_failed_restore(&state, &mut db_guard, &live_path);
                cleanup_staging(&staging_db, Some(&hold_db));
                cleanup_asset_staging();
                return Err(err);
            }
        };

        if let Err(e) = std::fs::copy(&staging_db, &live_path) {
            restore_quarantined_tree(&quarantined);
            recover_connection_after_failed_restore(&state, &mut db_guard, &live_path);
            cleanup_staging(&staging_db, Some(&hold_db));
            cleanup_asset_staging();
            return Err(CommandError::new(
                "restore_failed",
                format!("Could not install restored database: {e}"),
            ));
        }

        let restored = match crate::db::Database::open_path(&live_path) {
            Ok(db) => db,
            Err(e) => {
                let _ = std::fs::remove_file(&live_path);
                remove_db_sidecars(&live_path);
                restore_quarantined_tree(&quarantined);
                recover_connection_after_failed_restore(&state, &mut db_guard, &live_path);
                cleanup_staging(&staging_db, Some(&hold_db));
                cleanup_asset_staging();
                return Err(CommandError::new(
                    "restore_failed",
                    format!("Restored database could not reopen: {e}"),
                ));
            }
        };
        *db_guard = restored;
        quarantined
    };

    // Promote staged assets into live roots. On failure, roll the DB back from quarantine
    // so we never leave a restored profile with missing media/attachments.
    if let Err(e) = crate::db::promote_restored_assets(
        &asset_media_staging,
        &paths.media,
        &asset_att_staging,
        &paths.attachments,
    ) {
        {
            let mut db_guard = state.db.lock();
            let _ = std::fs::remove_file(&live_path);
            remove_db_sidecars(&live_path);
            restore_quarantined_tree(&quarantined);
            recover_connection_after_failed_restore(&state, &mut db_guard, &live_path);
        }
        cleanup_staging(&staging_db, Some(&hold_db));
        cleanup_asset_staging();
        return Err(match e {
            crate::db::DbError::Invalid(msg) => CommandError::new("restore_failed", msg),
            other => CommandError::from(other),
        });
    }
    cleanup_asset_staging();

    // Refresh derived state now that the live profile is installed under the mutex.
    {
        let db = state.db.lock();
        *state.event_bus.lock() = crate::runtime_v2::EventBus::load_from_db(&db);
    }
    *state.bootstrap.lock() = crate::db::BootstrapStatus::Ready;

    cleanup_staging(&staging_db, Some(&hold_db));

    // Recovery shells skip scheduler at setup; start it once the profile is restored.
    if started_without_profile {
        if let Some(handle) = app.try_state::<Arc<SchedulerHandle>>() {
            crate::automations::spawn_scheduler(app.clone(), handle.inner().clone());
        }
    }

    Ok(RestoreApplied {
        safety_backup_path,
        restored_from: archive_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("backup.coreside-backup")
            .to_string(),
        restart_recommended: true,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSummary {
    pub chat_count: u64,
    pub tool_count: u64,
    pub project_count: u64,
    pub database_bytes: Option<u64>,
    pub backup_bytes: Option<u64>,
    pub media_bytes: Option<u64>,
    pub profile_ready: bool,
}

fn dir_size(path: &std::path::Path) -> Option<u64> {
    let mut total = 0u64;
    let entries = std::fs::read_dir(path).ok()?;
    for entry in entries.flatten() {
        // Do not follow symlinks — storage summary must stay inside managed roots.
        let meta = std::fs::symlink_metadata(entry.path()).ok()?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_file() {
            total = total.saturating_add(meta.len());
        } else if meta.is_dir() {
            if let Some(n) = dir_size(&entry.path()) {
                total = total.saturating_add(n);
            }
        }
    }
    Some(total)
}

#[tauri::command]
pub fn get_storage_summary(state: State<'_, AppState>) -> Result<StorageSummary, CommandError> {
    if !state.profile_ready() {
        return Ok(StorageSummary {
            chat_count: 0,
            tool_count: 0,
            project_count: 0,
            database_bytes: None,
            backup_bytes: None,
            media_bytes: None,
            profile_ready: false,
        });
    }
    let db = state.db.lock();
    let chat_count: u64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
        .unwrap_or(0);
    let tool_count: u64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM tools", [], |r| r.get(0))
        .unwrap_or(0);
    let project_count: u64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap_or(0);
    let database_bytes = std::fs::metadata(db.path()).ok().map(|m| m.len());
    let paths = AppPaths::resolve().ok();
    let backup_bytes = paths.as_ref().and_then(|p| dir_size(&p.backups));
    let media_bytes = paths.as_ref().and_then(|p| dir_size(&p.media));
    Ok(StorageSummary {
        chat_count,
        tool_count,
        project_count,
        database_bytes,
        backup_bytes,
        media_bytes,
        profile_ready: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn quarantine_tree_rolls_back_partial_move() {
        let dir = tempdir().unwrap();
        let live = dir.path().join("coreside.db");
        let wal = dir.path().join("coreside.db-wal");
        let quarantine = dir.path().join("q");
        fs::create_dir_all(&quarantine).unwrap();
        fs::write(&live, b"main").unwrap();
        fs::write(&wal, b"wal").unwrap();

        // Occupy the shm destination so the third rename fails after two succeed.
        let shm_dest = quarantine.join("coreside.db-shm");
        fs::create_dir_all(&shm_dest).unwrap();
        fs::write(dir.path().join("coreside.db-shm"), b"shm").unwrap();

        let err = quarantine_db_tree(&live, &quarantine).unwrap_err();
        assert_eq!(err.code, "restore_failed");
        assert!(live.exists(), "main db should be restored after rollback");
        assert!(wal.exists(), "wal should be restored after rollback");
        assert_eq!(fs::read(&live).unwrap(), b"main");
        assert_eq!(fs::read(&wal).unwrap(), b"wal");
    }

    #[test]
    fn quarantine_tree_moves_sidecars() {
        let dir = tempdir().unwrap();
        let live = dir.path().join("coreside.db");
        fs::write(&live, b"main").unwrap();
        fs::write(dir.path().join("coreside.db-wal"), b"wal").unwrap();
        let quarantine = dir.path().join("q");
        fs::create_dir_all(&quarantine).unwrap();
        let moved = quarantine_db_tree(&live, &quarantine).unwrap();
        assert_eq!(moved.len(), 2);
        assert!(!live.exists());
        assert!(quarantine.join("coreside.db").exists());
        assert!(quarantine.join("coreside.db-wal").exists());
        restore_quarantined_tree(&moved);
        assert!(live.exists());
        assert_eq!(fs::read(&live).unwrap(), b"main");
    }
}
