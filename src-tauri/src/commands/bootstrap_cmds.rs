//! Startup bootstrap / recovery-safe commands.

use tauri::State;

use super::CommandError;
use crate::db::{self, BootstrapStatus};
use crate::state::AppState;

#[tauri::command]
pub fn get_bootstrap_status(state: State<'_, AppState>) -> BootstrapStatus {
    state.bootstrap_status()
}

#[tauri::command]
pub fn retry_open_database(state: State<'_, AppState>) -> Result<BootstrapStatus, CommandError> {
    if state.profile_ready() {
        return Ok(BootstrapStatus::Ready);
    }

    match db::Database::open_default() {
        Ok(db) => {
            let _ = db::ensure_default_workspace(&db);
            state.replace_profile_database(db);
            tracing::info!("profile database reopened successfully");
            Ok(BootstrapStatus::Ready)
        }
        Err(err) => Err(CommandError::sanitized(
            "database_unavailable",
            err,
            None,
        )),
    }
}
