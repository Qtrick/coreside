//! Tool CRUD, apply/undo, state, and window commands.

use serde_json::json;
use tauri::{AppHandle, State};

use super::CommandError;
use crate::ai::ToolChangePayload;
use crate::db::{self, ToolRecord, ToolVersionRecord, DEFAULT_WORKSPACE_ID};
use crate::state::AppState;
use crate::windows;

#[tauri::command]
pub fn list_tools(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
) -> Result<Vec<ToolRecord>, CommandError> {
    let db = state.db.lock();
    let mut tools = db::list_tools(&db, workspace_id.as_deref())?;
    for tool in &mut tools {
        tool.definition.normalize_for_frontend();
    }
    Ok(tools)
}

#[tauri::command]
pub fn get_tool(state: State<'_, AppState>, tool_id: String) -> Result<ToolRecord, CommandError> {
    let db = state.db.lock();
    let mut tool = db::get_tool(&db, &tool_id)?;
    tool.definition.normalize_for_frontend();
    Ok(tool)
}

#[tauri::command]
pub fn get_tool_versions(
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<Vec<ToolVersionRecord>, CommandError> {
    let db = state.db.lock();
    let mut versions = db::get_tool_versions(&db, &tool_id)?;
    for v in &mut versions {
        v.definition.normalize_for_frontend();
    }
    Ok(versions)
}

/// Accepts frontend invoke shape:
/// `{ conversationId, messageId, toolChange: { action, targetToolId, tool, changeSummary } }`
#[tauri::command]
pub fn apply_tool_change(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: String,
    tool_change: ToolChangePayload,
    workspace_id: Option<String>,
) -> Result<ToolRecord, CommandError> {
    let mut db = state.db.lock();

    // Validate conversation / message exist
    let _ = db::get_conversation(&db, &conversation_id)?;
    let msg = db::get_message(&db, &message_id)?;
    if msg.conversation_id != conversation_id {
        return Err(CommandError::new(
            "invalid",
            "message does not belong to conversation",
        ));
    }

    let tool_def = tool_change
        .tool
        .ok_or_else(|| CommandError::new("invalid", "toolChange.tool is required"))?;

    let workspace_id = workspace_id.unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string());
    let _ = db::ensure_default_workspace(&db)?;

    let action = tool_change.action.as_str();
    let summary = if tool_change.change_summary.trim().is_empty() {
        "Applied tool change".to_string()
    } else {
        tool_change.change_summary.clone()
    };

    let mut tool = db::apply_tool_change(
        &mut db,
        &workspace_id,
        &tool_def,
        action,
        tool_change.target_tool_id.as_deref(),
        &summary,
    )?;
    tool.definition.normalize_for_frontend();

    // Mark message metadata as applied
    let mut meta = msg.metadata.unwrap_or_else(|| json!({}));
    if let Some(obj) = meta.as_object_mut() {
        obj.insert("toolChangeStatus".into(), json!("applied"));
        obj.insert("pending".into(), json!(false));
        obj.insert("appliedToolId".into(), json!(tool.id));
    }
    let _ = db::update_message_metadata(&mut db, &message_id, &meta);

    Ok(tool)
}

#[tauri::command]
pub fn undo_tool_change(
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<ToolRecord, CommandError> {
    let mut db = state.db.lock();
    let mut tool = db::undo_tool_change(&mut db, &tool_id)?;
    tool.definition.normalize_for_frontend();
    Ok(tool)
}

#[tauri::command]
pub fn save_tool_state(
    app_state: State<'_, AppState>,
    tool_id: String,
    state: serde_json::Value,
) -> Result<(), CommandError> {
    let mut db = app_state.db.lock();
    Ok(db::save_tool_state(&mut db, &tool_id, &state)?)
}

#[tauri::command]
pub fn get_tool_state(
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<serde_json::Value, CommandError> {
    let db = state.db.lock();
    Ok(db::get_tool_state(&db, &tool_id)?.unwrap_or_else(|| json!({})))
}

#[tauri::command]
pub fn clear_tools(state: State<'_, AppState>) -> Result<u64, CommandError> {
    let mut db = state.db.lock();
    Ok(db::clear_tools(&mut db)?)
}

#[tauri::command]
pub fn open_tool_window(app: AppHandle, tool_id: String) -> Result<(), CommandError> {
    windows::open_tool_window(&app, &tool_id)
}
