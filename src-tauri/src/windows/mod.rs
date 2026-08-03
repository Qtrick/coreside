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
pub fn is_safe_tool_window_id(tool_id: &str) -> bool {
    !tool_id.is_empty()
        && tool_id.len() <= 128
        && tool_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Tool-window label prefix used when opening secondary windows (`tool-{toolId}`).
pub const TOOL_WINDOW_PREFIX: &str = "tool-";

/// If the caller is a secondary tool window, return the tool id bound to that window.
///
/// Labels that start with `tool-` but fail the id safety check yield `Some("")` so
/// enforce_* denies access instead of treating the caller as unrestricted.
pub fn caller_bound_tool_id(window: &tauri::WebviewWindow) -> Option<String> {
    let label = window.label();
    let Some(id) = label.strip_prefix(TOOL_WINDOW_PREFIX) else {
        return None;
    };
    if is_safe_tool_window_id(id) {
        Some(id.to_string())
    } else {
        Some(String::new())
    }
}

/// Pure scope check used by [`enforce_caller_surface_scope`] (unit-testable).
pub fn surface_allowed_for_bound_tool(
    bound_tool_id: &str,
    surface_tool_id: Option<&str>,
    surface_id: &str,
) -> bool {
    if bound_tool_id.is_empty() || !is_safe_tool_window_id(bound_tool_id) {
        return false;
    }
    match surface_tool_id {
        Some(tid) => tid == bound_tool_id,
        // Canonical personal-tool surface ids are `surf-{toolId}` when tool_id is unset.
        None => surface_id == format!("surf-{bound_tool_id}"),
    }
}

/// Tool windows may only touch their own tool id. Main and other windows are unrestricted here
/// (Tauri ACL still applies).
pub fn enforce_caller_tool_scope(
    window: &tauri::WebviewWindow,
    resource_tool_id: &str,
) -> Result<(), CommandError> {
    let Some(bound) = caller_bound_tool_id(window) else {
        return Ok(());
    };
    if bound.is_empty() || bound != resource_tool_id {
        return Err(CommandError::new(
            "forbidden",
            "This tool window cannot access another tool.",
        ));
    }
    Ok(())
}

/// Tool windows may only read/write surfaces owned by their bound tool.
pub fn enforce_caller_surface_scope(
    window: &tauri::WebviewWindow,
    surface_tool_id: Option<&str>,
    surface_id: &str,
) -> Result<(), CommandError> {
    let Some(bound) = caller_bound_tool_id(window) else {
        return Ok(());
    };
    if surface_allowed_for_bound_tool(&bound, surface_tool_id, surface_id) {
        return Ok(());
    }
    Err(CommandError::new(
        "forbidden",
        "This tool window cannot access another surface.",
    ))
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
    // Match open_tool_window: only resize for tools that exist.
    {
        let state = app.state::<AppState>();
        let db = state.db.lock();
        db::get_tool(&db, &tool_id).map_err(CommandError::from)?;
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
    use super::{
        is_safe_tool_window_id, surface_allowed_for_bound_tool, TOOL_WINDOW_PREFIX,
    };

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

    #[test]
    fn tool_window_label_strips_single_prefix() {
        assert_eq!(TOOL_WINDOW_PREFIX, "tool-");
        let label = format!("{TOOL_WINDOW_PREFIX}{}", "tool-notes");
        assert_eq!(label.strip_prefix(TOOL_WINDOW_PREFIX), Some("tool-notes"));
    }

    #[test]
    fn surface_scope_requires_matching_tool_id() {
        assert!(surface_allowed_for_bound_tool(
            "notes",
            Some("notes"),
            "surf-other"
        ));
        assert!(!surface_allowed_for_bound_tool(
            "notes",
            Some("other"),
            "surf-notes"
        ));
        assert!(surface_allowed_for_bound_tool("notes", None, "surf-notes"));
        assert!(!surface_allowed_for_bound_tool(
            "notes",
            None,
            "surf-other"
        ));
        assert!(!surface_allowed_for_bound_tool("", None, "surf-"));
        assert!(!surface_allowed_for_bound_tool("../x", None, "surf-../x"));
        assert!(!surface_allowed_for_bound_tool("bad/id", Some("bad/id"), "x"));
    }
}
