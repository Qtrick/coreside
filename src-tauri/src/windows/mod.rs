//! Secondary tool windows and main-window orchestration.

mod orchestrator;

pub use orchestrator::{ExpandDirection, ExpansionDecision, OrchestratorInspect};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::commands::CommandError;
use crate::db;
use crate::state::AppState;

/// Open (or focus) a secondary window for a tool: label `tool-{id}`, URL `/#/tool/{id}`.
pub fn open_tool_window(
    app: &AppHandle,
    tool_id: &str,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<(), CommandError> {
    let tool_id = tool_id.trim();
    if !is_safe_tool_window_id(tool_id) {
        return Err(CommandError::new(
            "invalid",
            "toolId must be a non-empty ascii id (letters, digits, -, _; max 128 chars).",
        ));
    }

    let label = format!("tool-{tool_id}");

    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }

    let state = app.state::<AppState>();
    let tool_name = {
        let db = state.db.lock();
        // Refuse opening windows for unknown tools — prevents label/URL probing.
        db::get_tool(&db, tool_id)
            .map(|tool| tool.name)
            .map_err(CommandError::from)?
    };

    let (w, h) = orchestrator::clamp_tool_window_size(app, width, height)?;
    let title = format!("Coreside — {tool_name}");
    let url = WebviewUrl::App(format!("/#/tool/{tool_id}").into());

    WebviewWindowBuilder::new(app, &label, url)
        .title(title)
        .inner_size(w, h)
        .min_inner_size(400.0, 320.0)
        .resizable(true)
        .build()
        .map_err(|e| CommandError::new("window", format!("Failed to open tool window: {e}")))?;

    Ok(())
}

/// Window labels are `tool-{id}` and must match the `tool-*` capability pattern.
fn is_safe_tool_window_id(tool_id: &str) -> bool {
    !tool_id.is_empty()
        && tool_id.len() <= 128
        && tool_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn inspect(app: &AppHandle) -> Result<OrchestratorInspect, CommandError> {
    orchestrator::inspect(app)
}

pub fn expand(
    app: &AppHandle,
    tool_id: String,
    min_useful_width: f64,
    min_useful_height: f64,
    direction: ExpandDirection,
    reduced_motion: bool,
) -> Result<ExpansionDecision, CommandError> {
    let tool_id = tool_id.trim().to_string();
    if !is_safe_tool_window_id(&tool_id) {
        return Err(CommandError::new(
            "invalid",
            "toolId must be a non-empty ascii id (letters, digits, -, _; max 128 chars).",
        ));
    }
    orchestrator::expand(
        app,
        tool_id,
        min_useful_width,
        min_useful_height,
        direction,
        reduced_motion,
    )
}

pub fn restore(app: &AppHandle, reduced_motion: bool) -> Result<ExpansionDecision, CommandError> {
    orchestrator::restore(app, reduced_motion)
}

pub fn cancel_animation() -> Result<(), CommandError> {
    orchestrator::cancel_animation()
}

#[cfg(test)]
mod tests {
    use super::is_safe_tool_window_id;

    #[test]
    fn tool_window_ids_reject_path_and_empty() {
        assert!(is_safe_tool_window_id("tool-abc-123"));
        assert!(is_safe_tool_window_id("a_b"));
        assert!(is_safe_tool_window_id("a"));
        assert!(!is_safe_tool_window_id(""));
        assert!(!is_safe_tool_window_id("../etc"));
        assert!(!is_safe_tool_window_id("tool/id"));
        assert!(!is_safe_tool_window_id("tool id"));
        assert!(!is_safe_tool_window_id("tool%2eid"));
        assert!(!is_safe_tool_window_id(&"x".repeat(129)));
    }
}
