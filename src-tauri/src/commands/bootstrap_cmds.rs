//! Startup bootstrap / recovery-safe commands.

use tauri::{AppHandle, State};

use super::CommandError;
use crate::db::{self, BootstrapStatus};
use crate::state::AppState;

#[tauri::command]
pub fn get_bootstrap_status(state: State<'_, AppState>) -> BootstrapStatus {
    state.bootstrap_status()
}

#[tauri::command]
pub fn retry_open_database(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BootstrapStatus, CommandError> {
    if state.profile_ready() {
        return Ok(BootstrapStatus::Ready);
    }

    match db::Database::open_default() {
        Ok(db) => {
            let _ = db::ensure_default_workspace(&db);
            state.replace_profile_database(db);
            let _ = crate::branding::reconcile_dock_on_startup(&app, &state.db);
            tracing::info!("profile database reopened successfully");
            Ok(BootstrapStatus::Ready)
        }
        Err(err) => {
            // Keep the recovery shell; return an updated RecoveryRequired status
            // so the UI can refresh the reason without treating this as a crash.
            let (code, message) = db::bootstrap::classify_open_error(&err);
            tracing::warn!(error = %err, reason = code, "retry open database failed");
            let profile_path = db::default_db_path_for_diagnostics().ok();
            let status = BootstrapStatus::recovery(code, message, profile_path, true);
            *state.bootstrap.lock() = status.clone();
            Ok(status)
        }
    }
}
