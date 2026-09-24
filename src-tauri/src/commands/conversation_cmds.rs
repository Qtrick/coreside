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
    // Tombstone the conversation to prevent late-arriving provider turns
    // from resurrecting rows or emitting events after deletion.
    state.deleted_conversations.lock().insert(conversation_id.clone());
    // Quiesce in-flight provider work: active_requests is keyed by
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
    let active_keys: Vec<String> = state.active_requests.lock().keys().cloned().collect();
    for key in &active_keys {
        state.deleted_conversations.lock().insert(key.clone());
        state.cancel_request(key);
        state.take_request(key);
    }
    let mut db = state.db.lock();
    if let Ok(convs) = db::list_conversations(&db, None) {
        let mut dc = state.deleted_conversations.lock();
        for c in convs {
            dc.insert(c.id.clone());
            state.cancel_request(&c.id);
            state.take_request(&c.id);
        }
    }
    Ok(db::clear_conversations(&mut db)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::state::AppState;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn test_conversation_deletion_and_tombstone_barrier() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("test_conv.db")).unwrap();
        let ws = crate::db::ensure_default_workspace(&db).unwrap();
        let conv = crate::db::create_conversation(&mut db, &ws, "Test Chat", None).unwrap();

        assert!(crate::db::conversation_exists(&db, &conv.id));

        let state = AppState::new_for_test(db);

        // Simulate tombstone barrier
        state.deleted_conversations.lock().insert(conv.id.clone());
        state.cancel_request(&conv.id);
        state.take_request(&conv.id);

        let mut lock = state.db.lock();
        crate::db::delete_conversation(&mut lock, &conv.id).unwrap();

        assert!(!crate::db::conversation_exists(&lock, &conv.id));
        assert!(state.deleted_conversations.lock().contains(&conv.id));
    }

    #[test]
    fn test_clear_conversations_tombstones_all() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("test_clear.db")).unwrap();
        let ws = crate::db::ensure_default_workspace(&db).unwrap();
        let conv1 = crate::db::create_conversation(&mut db, &ws, "Chat 1", None).unwrap();
        let conv2 = crate::db::create_conversation(&mut db, &ws, "Chat 2", None).unwrap();

        let state = AppState::new_for_test(db);

        // Pre-register active request for conv1
        state.register_request(&conv1.id, CancellationToken::new());

        // Perform clear logic
        let active_keys: Vec<String> = state.active_requests.lock().keys().cloned().collect();
        for key in &active_keys {
            state.deleted_conversations.lock().insert(key.clone());
            state.cancel_request(key);
            state.take_request(key);
        }
        let mut lock = state.db.lock();
        if let Ok(convs) = crate::db::list_conversations(&lock, None) {
            let mut dc = state.deleted_conversations.lock();
            for c in convs {
                dc.insert(c.id.clone());
                state.cancel_request(&c.id);
                state.take_request(&c.id);
            }
        }
        crate::db::clear_conversations(&mut lock).unwrap();

        assert!(!crate::db::conversation_exists(&lock, &conv1.id));
        assert!(!crate::db::conversation_exists(&lock, &conv2.id));
        assert!(state.deleted_conversations.lock().contains(&conv1.id));
        assert!(state.deleted_conversations.lock().contains(&conv2.id));
    }
}

