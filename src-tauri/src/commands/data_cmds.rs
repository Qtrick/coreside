//! Data integrity and backup-related settings commands.

use serde::Serialize;
use tauri::State;

use super::CommandError;
use crate::db::{product_data_dir, DatabaseHealthReport};
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
}

#[tauri::command]
pub fn create_profile_backup(state: State<'_, AppState>) -> Result<BackupCreated, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let base = dirs::data_dir().ok_or_else(|| {
        CommandError::new(
            "storage_unavailable",
            "Could not resolve the application data folder.",
        )
    })?;
    let backup_dir = product_data_dir(&base).join("backups");
    std::fs::create_dir_all(&backup_dir).map_err(|e| {
        CommandError::new(
            "storage_unavailable",
            format!("Could not create backups folder: {e}"),
        )
    })?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let dest = backup_dir.join(format!("coreside-profile-{stamp}.db"));
    db.snapshot_to_path(&dest)?;
    let meta = std::fs::metadata(&dest).map_err(|e| {
        CommandError::new(
            "export_failed",
            format!("Backup written but unreadable: {e}"),
        )
    })?;
    let schema = db.latest_migration_name()?;
    Ok(BackupCreated {
        path: dest
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("backup.db")
            .to_string(),
        byte_size: meta.len(),
        schema_version: schema,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSummary {
    pub chat_count: u64,
    pub tool_count: u64,
    pub project_count: u64,
    pub database_bytes: Option<u64>,
    pub profile_ready: bool,
}

#[tauri::command]
pub fn get_storage_summary(state: State<'_, AppState>) -> Result<StorageSummary, CommandError> {
    if !state.profile_ready() {
        return Ok(StorageSummary {
            chat_count: 0,
            tool_count: 0,
            project_count: 0,
            database_bytes: None,
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
    Ok(StorageSummary {
        chat_count,
        tool_count,
        project_count,
        database_bytes,
        profile_ready: true,
    })
}
