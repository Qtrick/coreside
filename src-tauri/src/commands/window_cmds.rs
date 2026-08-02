//! Main-window orchestrator and secondary tool-window commands.

use tauri::AppHandle;

use super::CommandError;
use crate::windows::{self, ExpandDirection, ExpansionDecision, OrchestratorInspect};

#[tauri::command]
pub fn window_orchestrator_inspect(
    app: AppHandle,
) -> Result<OrchestratorInspect, CommandError> {
    windows::inspect(&app)
}

#[tauri::command]
pub fn window_orchestrator_expand(
    app: AppHandle,
    tool_id: String,
    min_useful_width: f64,
    min_useful_height: f64,
    direction: Option<ExpandDirection>,
    reduced_motion: Option<bool>,
) -> Result<ExpansionDecision, CommandError> {
    windows::expand(
        &app,
        tool_id,
        min_useful_width,
        min_useful_height,
        direction.unwrap_or(ExpandDirection::Automatic),
        reduced_motion.unwrap_or(false),
    )
}

#[tauri::command]
pub fn window_orchestrator_restore(
    app: AppHandle,
    reduced_motion: Option<bool>,
) -> Result<ExpansionDecision, CommandError> {
    windows::restore(&app, reduced_motion.unwrap_or(false))
}

#[tauri::command]
pub fn window_orchestrator_cancel() -> Result<(), CommandError> {
    windows::cancel_animation()
}
