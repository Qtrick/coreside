//! Tool CRUD, apply/undo, state, and window commands.

use serde_json::json;
use tauri::{AppHandle, State};

use super::CommandError;
use crate::ai::ToolChangePayload;
use crate::db::{self, ToolRecord, ToolVersionRecord, DEFAULT_WORKSPACE_ID};
use crate::runtime_v2::tool_change_to_operations;
use crate::state::AppState;
use crate::windows;

#[tauri::command]
pub fn list_tools(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
) -> Result<Vec<ToolRecord>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let mut tools = db::list_tools(&db, workspace_id.as_deref())?;
    for tool in &mut tools {
        tool.definition.normalize_for_frontend();
    }
    Ok(tools)
}

#[tauri::command]
pub fn get_tool(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<ToolRecord, CommandError> {
    state.require_profile()?;
    windows::enforce_caller_tool_scope(&window, &tool_id)?;
    let db = state.db.lock();
    let mut tool = db::get_tool(&db, &tool_id)?;
    tool.definition.normalize_for_frontend();
    Ok(tool)
}

#[tauri::command]
pub fn get_tool_versions(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<Vec<ToolVersionRecord>, CommandError> {
    state.require_profile()?;
    windows::enforce_caller_tool_scope(&window, &tool_id)?;
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
    state.require_profile()?;
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
        .as_ref()
        .ok_or_else(|| CommandError::new("invalid", "toolChange.tool is required"))?;

    // Reject protected core.* ids (branding, base settings, platform internals).
    super::settings_cmds::reject_protected_ids(&[tool_def.id.as_str()])?;
    if let Some(ref target) = tool_change.target_tool_id {
        super::settings_cmds::reject_protected_ids(&[target.as_str()])?;
    }

    let workspace_id = workspace_id.unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string());
    // Runtime V2's tool-surface transaction currently owns the personal
    // workspace. Do not silently redirect a legacy request for another
    // workspace into it.
    if workspace_id != DEFAULT_WORKSPACE_ID {
        return Err(CommandError::new(
            "unsupported",
            "Legacy tool changes support the personal workspace only",
        ));
    }
    let _ = db::ensure_default_workspace(&db)?;

    let summary = if tool_change.change_summary.trim().is_empty() {
        "Applied tool change".to_string()
    } else {
        tool_change.change_summary.clone()
    };

    // Legacy proposals are accepted by a user here, but their durable mutation
    // must still pass the Runtime V2 operation and capability-pack boundary.
    let operations = tool_change_to_operations(&tool_change);
    if operations.is_empty() {
        return Err(CommandError::new("invalid", "toolChange.tool is required"));
    }
    let target_tool_id = tool_change
        .target_tool_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or(&tool_def.id)
        .to_string();
    let mut bus = state.event_bus.lock();
    let result = crate::application_kernel::apply_change(
        &mut db,
        Some(&mut bus),
        crate::application_kernel::ChangeRequest {
            conversation_id: Some(conversation_id.clone()),
            project_id: None,
            turn_id: None,
            summary,
            operations,
            silent: false,
            source_type: "user".into(),
            provider: None,
            model: None,
            require_approval: false,
            approval_granted: true,
        },
    )
    .map_err(|e| CommandError::new(e.category(), e.user_message()))?;
    if !result.is_committed() {
        let (code, message) = if !result.conflicts.is_empty() {
            ("conflict", result.conflicts.join("; "))
        } else if result.proposal_id.is_some() {
            (
                "approval_required",
                "User approval is required to apply this change".into(),
            )
        } else {
            ("apply_failed", "Tool change was not committed".into())
        };
        return Err(CommandError::new(code, message));
    }
    let mut tool = db::get_tool(&db, &target_tool_id)?;
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
    state.require_profile()?;
    let mut db = state.db.lock();
    let mut tool = db::undo_tool_change(&mut db, &tool_id)?;
    tool.definition.normalize_for_frontend();
    Ok(tool)
}

#[tauri::command]
pub fn save_tool_state(
    window: tauri::WebviewWindow,
    app_state: State<'_, AppState>,
    tool_id: String,
    state: serde_json::Value,
) -> Result<(), CommandError> {
    app_state.require_profile()?;
    windows::enforce_caller_tool_scope(&window, &tool_id)?;
    let mut db = app_state.db.lock();
    Ok(db::save_tool_state(&mut db, &tool_id, &state)?)
}

#[tauri::command]
pub fn get_tool_state(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<serde_json::Value, CommandError> {
    state.require_profile()?;
    windows::enforce_caller_tool_scope(&window, &tool_id)?;
    let db = state.db.lock();
    Ok(db::get_tool_state(&db, &tool_id)?.unwrap_or_else(|| json!({})))
}

#[tauri::command]
pub fn clear_tools(state: State<'_, AppState>) -> Result<u64, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::clear_tools(&mut db)?)
}

#[tauri::command]
pub fn open_tool_window(
    window: tauri::WebviewWindow,
    app: AppHandle,
    tool_id: String,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<(), CommandError> {
    // Defense in depth: tool windows must not spawn further windows even if ACL drifts.
    if windows::caller_bound_tool_id(&window).is_some() {
        return Err(CommandError::new(
            "forbidden",
            "Tool windows cannot open other tool windows.",
        ));
    }
    windows::open_tool_window(&app, &tool_id, width, height)
}
