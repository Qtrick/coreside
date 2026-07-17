//! Agent turn: send_message + cancel_request.

use serde::Serialize;
use serde_json::json;
use tauri::State;
use tokio_util::sync::CancellationToken;

use super::CommandError;
use crate::ai::{
    build_agent_prompt, create_provider, parse_agent_response, AgentMessage, AgentRequest,
    ParsedAgentResponse, ResponseType, ToolChangePayload, PROMPT_VERSION,
};
use crate::db::{self, Message};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResult {
    /// Assistant message id (primary id for apply/discard).
    pub message_id: String,
    /// Assistant message text content.
    pub assistant_message: String,
    pub response_type: ResponseType,
    pub tool_change: Option<ToolChangePayload>,
    pub diagnostics: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_message_id: Option<String>,
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    content: String,
    conversation_id: String,
    active_tool_id: Option<String>,
) -> Result<SendMessageResult, CommandError> {
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err(CommandError::new("invalid", "Message content cannot be empty"));
    }

    let config = state.config.clone();
    let api_key = config.api_key.clone();

    // 1. Persist user message + load context (sync DB section)
    let (user_message, history, active_tool, request_key) = {
        let mut db = state.db.lock();
        let _ = db::get_conversation(&db, &conversation_id)?;

        let user_message = db::insert_message(&mut db, &conversation_id, "user", &content, None)?;
        let _ = db::maybe_rename_conversation_from_message(&mut db, &conversation_id, &content)?;

        let history = db::get_recent_messages(&db, &conversation_id, 40)?;

        let active_tool = if let Some(ref tid) = active_tool_id {
            match db::get_tool(&db, tid) {
                Ok(t) => {
                    let mut def = t.definition;
                    def.normalize_for_frontend();
                    Some(def)
                }
                Err(crate::db::DbError::NotFound(_)) => None,
                Err(e) => return Err(e.into()),
            }
        } else {
            None
        };

        let request_key = conversation_id.clone();
        (user_message, history, active_tool, request_key)
    };

    // 2–4. Build prompts and call provider
    let system_prompt = build_agent_prompt(active_tool.as_ref(), None);

    let messages: Vec<AgentMessage> = history
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| AgentMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let cancel = CancellationToken::new();
    state.register_request(&request_key, cancel.clone());

    let provider = create_provider(&config).map_err(|e| {
        state.take_request(&request_key);
        CommandError::sanitized(e.code(), e, api_key.as_deref())
    })?;

    let agent_result = provider
        .chat(AgentRequest {
            system_prompt,
            messages,
            cancel: cancel.clone(),
        })
        .await;

    state.take_request(&request_key);

    let agent_response = agent_result.map_err(|e| {
        CommandError::sanitized(e.code(), e, api_key.as_deref())
    })?;

    // 5. Parse / validate
    let mut parsed: ParsedAgentResponse = parse_agent_response(&agent_response.raw_text).map_err(|e| {
        CommandError::sanitized("parse", e, api_key.as_deref())
    })?;
    parsed.payload.normalize_for_frontend();

    let diagnostics = {
        let mut d = json!({
            "promptVersion": PROMPT_VERSION,
            "provider": agent_response.provider_id,
            "model": agent_response.model,
            "usage": agent_response.usage,
            "recovered": parsed.recovered,
            "parseWarnings": parsed.parse_warnings,
        });
        if let Some(extra) = &parsed.payload.diagnostics {
            d["providerDiagnostics"] = extra.clone();
        }
        Some(d)
    };

    let mut tool_change = parsed.payload.tool_change.clone();
    if let Some(tc) = tool_change.as_mut() {
        tc.normalize_for_frontend();
    }

    let metadata = json!({
        "schemaVersion": parsed.payload.schema_version,
        "responseType": parsed.payload.response_type.as_str(),
        "toolChange": tool_change,
        "toolChangeStatus": if tool_change.is_some() { "pending" } else { "none" },
        "pending": tool_change.is_some(),
        "recovered": parsed.recovered,
        "diagnostics": diagnostics,
    });

    // 6. Persist assistant message with pending tool change preview in metadata
    let assistant_message = {
        let mut db = state.db.lock();
        db::insert_message(
            &mut db,
            &conversation_id,
            "assistant",
            &parsed.payload.assistant_message,
            Some(&metadata),
        )?
    };

    // 7. Return frontend-shaped result
    Ok(SendMessageResult {
        message_id: assistant_message.id,
        assistant_message: parsed.payload.assistant_message,
        response_type: parsed.payload.response_type,
        tool_change,
        diagnostics,
        user_message_id: Some(user_message.id),
    })
}

#[tauri::command]
pub fn cancel_request(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
) -> Result<bool, CommandError> {
    match conversation_id {
        Some(id) if !id.trim().is_empty() => Ok(state.cancel_request(&id)),
        _ => {
            state.cancel_all();
            Ok(true)
        }
    }
}

#[tauri::command]
pub fn discard_tool_change(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<Message, CommandError> {
    let mut db = state.db.lock();
    let mut msg = db::get_message(&db, &message_id)?;
    let mut meta = msg.metadata.unwrap_or_else(|| json!({}));
    if let Some(obj) = meta.as_object_mut() {
        obj.insert("toolChangeStatus".into(), json!("discarded"));
        obj.insert("pending".into(), json!(false));
        obj.insert("discarded".into(), json!(true));
    }
    db::update_message_metadata(&mut db, &message_id, &meta)?;
    msg.metadata = Some(meta);
    Ok(msg)
}
