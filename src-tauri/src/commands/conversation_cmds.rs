//! Conversation CRUD commands.

use tauri::State;

use super::CommandError;
use crate::db::{self, Conversation, Message, DEFAULT_WORKSPACE_ID};
use crate::state::AppState;

#[tauri::command]
pub fn list_conversations(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
) -> Result<Vec<Conversation>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(db::list_conversations(&db, workspace_id.as_deref())?)
}

#[tauri::command]
pub fn create_conversation(
    state: State<'_, AppState>,
    title: Option<String>,
    workspace_id: Option<String>,
) -> Result<Conversation, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    let ws = workspace_id.unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string());
    let _ = db::ensure_default_workspace(&db)?;
    let title = title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "New chat".to_string());
    Ok(db::create_conversation(&mut db, &ws, &title, None)?)
}

#[tauri::command]
pub fn delete_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    // Quiesce in-flight provider work first: active_requests is keyed by
    // conversation, and the post-provider commit path can otherwise resurrect
    // messages, ledger rows, and action events after the rows are deleted.
    state.cancel_request(&conversation_id);
    state.take_request(&conversation_id);
    let mut db = state.db.lock();
    Ok(db::delete_conversation(&mut db, &conversation_id)?)
}

#[tauri::command]
pub fn get_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<Message>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(db::get_messages(&db, &conversation_id)?)
}

/// Remove a message and everything after it in the conversation (used by edit & resend).
#[tauri::command]
pub fn delete_messages_from(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<u64, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::delete_messages_from(&mut db, &message_id)?)
}

#[tauri::command]
pub fn clear_conversations(state: State<'_, AppState>) -> Result<u64, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::clear_conversations(&mut db)?)
}
