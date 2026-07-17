//! Secondary tool windows.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::commands::CommandError;
use crate::db;
use crate::state::AppState;

/// Open (or focus) a secondary window for a tool: label `tool-{id}`, URL `/#/tool/{id}`.
pub fn open_tool_window(app: &AppHandle, tool_id: &str) -> Result<(), CommandError> {
    let label = format!("tool-{tool_id}");

    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }

    let state = app.state::<AppState>();
    let tool_name = {
        let db = state.db.lock();
        match db::get_tool(&db, tool_id) {
            Ok(t) => t.name,
            Err(_) => tool_id.to_string(),
        }
    };

    let title = format!("Coreside — {tool_name}");
    let url = WebviewUrl::App(format!("/#/tool/{tool_id}").into());

    WebviewWindowBuilder::new(app, &label, url)
        .title(title)
        .inner_size(720.0, 640.0)
        .min_inner_size(400.0, 320.0)
        .resizable(true)
        .build()
        .map_err(|e| CommandError::new("window", format!("Failed to open tool window: {e}")))?;

    Ok(())
}
