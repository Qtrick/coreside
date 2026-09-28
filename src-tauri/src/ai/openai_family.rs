//! Shared OpenAI-compatible chat message mapping (OpenAI, OpenRouter, hosted gateway).
//!
//! Multimodal Image parts map to `image_url` data URLs. Tool results use native
//! `role=tool` + `tool_call_id` with sealed JSON envelope content — never flattened
//! into user text.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::openai::map_image_part_to_openai_content;
use super::provider::AgentMessage;
use super::structured_user_input::{flatten_parts_for_provider, AgentContentPart, AgentRole};

/// Build OpenAI-compatible chat `messages` array including system prompt.
pub fn build_openai_chat_messages(system_prompt: &str, messages: &[AgentMessage]) -> Vec<Value> {
    let mut out = vec![json!({
        "role": "system",
        "content": system_prompt
    })];
    for m in messages {
        out.push(openai_family_message_json(m));
    }
    out
}

/// Map one agent message to an OpenAI chat completion message object.
pub fn openai_family_message_json(message: &AgentMessage) -> Value {
    if message.role == AgentRole::ToolResult {
        return openai_tool_result_message_json(message);
    }
    json!({
        "role": message.role.as_openai_role(),
        "content": openai_family_message_content(message),
    })
}

/// OpenAI Chat Completions `tool_call_id` — bounded alphanumeric id from Rust only.
const MAX_OPENAI_TOOL_CALL_ID_LEN: usize = 128;

fn is_trusted_openai_tool_call_id(id: &str) -> bool {
    let id = id.trim();
    if id.is_empty() || id.len() > MAX_OPENAI_TOOL_CALL_ID_LEN {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Native Chat Completions tool result — envelope JSON in `content`, not user flatten.
fn openai_tool_result_message_json(message: &AgentMessage) -> Value {
    let tool_call_id = message
        .tool_call_id
        .as_deref()
        .filter(|s| is_trusted_openai_tool_call_id(s))
        .map(str::trim)
        .map(str::to_string)
        .unwrap_or_else(|| fallback_openai_tool_call_id(message));
    json!({
        "role": "tool",
        "tool_call_id": tool_call_id,
        "content": message.tool_result_upstream_content(),
    })
}

fn fallback_openai_tool_call_id(message: &AgentMessage) -> String {
    let upstream = message.tool_result_upstream_content();
    let hash = hex::encode(Sha256::digest(upstream.as_bytes()));
    format!("coreside-{}", &hash[..16.min(hash.len())])
}

/// Message `content` for OpenAI-family APIs.
pub fn openai_family_message_content(message: &AgentMessage) -> Value {
    if message.role == AgentRole::ToolResult {
        return json!(message.tool_result_upstream_content());
    }
    if message.parts.is_empty() {
        return json!(message.content);
    }
    let has_image = message
        .parts
        .iter()
        .any(|p| matches!(p, AgentContentPart::Image { .. }));
    if !has_image {
        return json!(text_parts_for_openai_family(&message.parts));
    }

    let mut content_parts: Vec<Value> = Vec::new();
    let text = text_parts_for_openai_family(
        &message
            .parts
            .iter()
            .filter(|p| !matches!(p, AgentContentPart::Image { .. }))
            .cloned()
            .collect::<Vec<_>>(),
    );
    if !text.trim().is_empty() {
        content_parts.push(json!({
            "type": "text",
            "text": text
        }));
    }
    for part in &message.parts {
        if matches!(part, AgentContentPart::Image { .. }) {
            match map_image_part_to_openai_content(part) {
                Ok(v) => content_parts.push(v),
                Err(reason) => {
                    tracing::info!(target: "coreside::ai::openai_family", "{reason}");
                    content_parts.push(json!({
                        "type": "text",
                        "text": format!("[image not sent to model: {reason}]"),
                    }));
                }
            }
        }
    }
    if content_parts.is_empty() {
        return json!(message.content);
    }
    if content_parts.len() == 1
        && content_parts[0].get("type").and_then(|t| t.as_str()) == Some("text")
    {
        return content_parts[0]
            .get("text")
            .cloned()
            .unwrap_or_else(|| json!(message.content));
    }
    Value::Array(content_parts)
}

fn text_parts_for_openai_family(parts: &[AgentContentPart]) -> String {
    let filtered: Vec<AgentContentPart> = parts
        .iter()
        .filter(|p| !matches!(p, AgentContentPart::ToolResultEnvelope { .. }))
        .cloned()
        .collect();
    if filtered.is_empty() {
        return String::new();
    }
    flatten_parts_for_provider(&filtered)
}

/// Whether this OpenAI-family model ID is known to accept `response_format: json_schema`.
///
/// Older Chat Completions models only support `json_object`. Unknown / compatible
/// endpoints fall back to `json_object` and rely on local response validation.
pub fn model_supports_json_schema_response_format(model: &str) -> bool {
    let m = model.trim().to_ascii_lowercase();
    if m.is_empty() {
        return false;
    }
    // Strip OpenRouter vendor prefix (`openai/gpt-4o` → `gpt-4o`).
    let bare = m.rsplit('/').next().unwrap_or(&m);
    let supports = bare.starts_with("gpt-4o")
        || bare.starts_with("gpt-4.1")
        || bare.starts_with("gpt-4.5")
        || bare.starts_with("gpt-5")
        || bare.starts_with("o1")
        || bare.starts_with("o3")
        || bare.starts_with("o4")
        || bare.contains("gpt-4o")
        || bare.contains("gpt-4.1");
    // Explicitly exclude legacy turbo / 3.5 / instruct which reject json_schema.
    let legacy = bare.contains("gpt-3.5")
        || bare.contains("gpt-4-turbo")
        || bare.contains("gpt-4-0613")
        || bare.contains("instruct");
    supports && !legacy
}

/// Guidance schema for Coreside agent JSON. Nested free-form trees (components,
/// operations payloads) cannot be fully closed under OpenAI `strict: true`, so
/// this uses `strict: false` + local `response_parser` validation as the
/// authority. Prefer this over bare `json_object`, which provides no schema.
pub fn agent_response_json_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "schemaVersion": { "type": "string" },
            "assistantMessage": { "type": "string" },
            "responseType": {
                "type": "string",
                "enum": ["message", "tool_change", "tool_use", "settings_change", "noop"]
            },
            "toolCalls": {
                "type": ["array", "null"],
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "capability": { "type": "string" },
                        "arguments": { "type": "object" }
                    },
                    "required": ["capability"]
                }
            },
            "citations": {
                "type": ["array", "null"],
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "id": { "type": "string" },
                        "title": { "type": "string" },
                        "url": { "type": "string" },
                        "displayDomain": { "type": "string" },
                        "snippet": { "type": "string" }
                    },
                    "required": ["id", "title", "url"]
                }
            },
            "toolChange": {
                "type": ["object", "null"],
                "additionalProperties": false,
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["create", "update", "replace"]
                    },
                    "targetToolId": { "type": ["string", "null"] },
                    "changeSummary": { "type": "string" },
                    "tool": {
                        "type": ["object", "null"],
                        "additionalProperties": false,
                        "properties": {
                            "id": { "type": "string" },
                            "name": { "type": "string" },
                            "description": { "type": ["string", "null"] },
                            "layout": { "type": ["object", "null"] },
                            "components": {
                                "type": "array",
                                "items": { "type": "object" }
                            },
                            "stateContracts": {
                                "type": ["array", "null"],
                                "items": { "type": "object" }
                            },
                            "interactive": {
                                "type": ["object", "null"],
                                "description": "Optional rules-engine definition. Rust validates and owns state."
                            }
                        },
                        "required": ["id", "name", "components"]
                    }
                },
                "required": ["action", "changeSummary"]
            },
            "operations": {
                "type": ["array", "null"],
                "description": "Canonical Runtime V2 operations.",
                "items": { "type": "object" }
            },
            "assistantMessages": {
                "type": ["array", "null"],
                "items": { "type": "object" }
            },
            "turnId": { "type": ["string", "null"] },
            "silent": { "type": ["boolean", "null"] },
            "settingsChange": { "type": ["object", "null"] },
            "diagnostics": { "type": ["object", "null"] }
        },
        "required": ["schemaVersion", "assistantMessage", "responseType"]
    })
}

/// Build Chat Completions `response_format` for an OpenAI-family model.
///
/// - Known schema-capable models → `json_schema` (guided; nested free-form is
///   not provider-strict — local validation remains authoritative).
/// - Legacy / unknown → `json_object` compatibility path.
pub fn openai_response_format(model: &str) -> Value {
    if model_supports_json_schema_response_format(model) {
        json!({
            "type": "json_schema",
            "json_schema": {
                "name": "coreside_agent_response",
                "strict": false,
                "schema": agent_response_json_schema()
            }
        })
    } else {
        json!({ "type": "json_object" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::capability_registry::{seal_tool_result_envelope, ToolCallResult};
    use crate::ai::structured_user_input::image_part_from_authorized_bytes;

    #[test]
    fn tool_result_uses_name_and_json_envelope_not_user_flatten() {
        let results = vec![ToolCallResult {
            capability: "web_search".into(),
            ok: true,
            output: json!({"hits": 1}),
            error: None,
            pending_approval: None,
        }];
        let envelope = seal_tool_result_envelope(&results);
        let msg =
            AgentMessage::with_parts(AgentRole::ToolResult, "Tool results (1)", vec![envelope]);
        let mut msg = msg;
        msg.tool_call_id = Some("call_web_search_1".into());
        let mapped = openai_family_message_json(&msg);
        assert_eq!(mapped["role"], "tool");
        assert_eq!(mapped["tool_call_id"], "call_web_search_1");
        assert!(mapped.get("name").is_none());
        let content = mapped["content"].as_str().expect("string envelope");
        assert!(content.contains("untrusted_tool_output"));
        assert!(content.contains("web_search"));
        assert!(!content.contains("[image attachmentId="));
    }

    #[test]
    fn multimodal_user_message_emits_image_url_array() {
        let png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE,
            0xD4, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let image = image_part_from_authorized_bytes("att-1", "image/png", png).unwrap();
        let msg = AgentMessage::with_parts(
            AgentRole::User,
            "see image",
            vec![
                AgentContentPart::Text {
                    text: "see image".into(),
                },
                image,
            ],
        );
        let content = openai_family_message_content(&msg);
        assert!(content.is_array());
        let arr = content.as_array().unwrap();
        assert!(arr.iter().any(|p| p["type"] == "image_url"));
        assert!(!arr.iter().any(|p| {
            p["type"] == "text" && p["text"].as_str().unwrap_or("").contains("[image")
        }));
    }

    #[test]
    fn tool_result_without_envelope_part_falls_back_to_display_content() {
        let msg = AgentMessage::with_parts(
            AgentRole::ToolResult,
            "legacy tool summary",
            vec![AgentContentPart::Text {
                text: "should not flatten into upstream".into(),
            }],
        );
        let mapped = openai_family_message_json(&msg);
        assert_eq!(mapped["role"], "tool");
        assert!(mapped["tool_call_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("coreside-")));
        assert!(mapped.get("name").is_none());
        assert_eq!(
            mapped["content"].as_str(),
            Some("legacy tool summary"),
            "missing envelope must fall back to display content, not flatten parts"
        );
    }

    #[test]
    fn tool_result_rejects_untrusted_tool_call_id() {
        let results = vec![ToolCallResult {
            capability: "web_search".into(),
            ok: true,
            output: json!({"hits": 1}),
            error: None,
            pending_approval: None,
        }];
        let envelope = seal_tool_result_envelope(&results);
        let msg =
            AgentMessage::with_parts(AgentRole::ToolResult, "Tool results (1)", vec![envelope]);
        let mut msg = msg;
        msg.tool_call_id = Some("call_inject\"; DROP TABLE--".into());
        let mapped = openai_family_message_json(&msg);
        assert!(
            mapped["tool_call_id"]
                .as_str()
                .is_some_and(|id| id.starts_with("coreside-")),
            "untrusted tool_call_id must not be forwarded upstream"
        );
    }

    #[test]
    fn tool_result_without_call_id_derives_stable_fallback_id() {
        let results = vec![ToolCallResult {
            capability: "web_search".into(),
            ok: true,
            output: json!({"hits": 1}),
            error: None,
            pending_approval: None,
        }];
        let envelope = seal_tool_result_envelope(&results);
        let msg =
            AgentMessage::with_parts(AgentRole::ToolResult, "Tool results (1)", vec![envelope]);
        let a = openai_family_message_json(&msg);
        let b = openai_family_message_json(&msg);
        assert_eq!(a["tool_call_id"], b["tool_call_id"]);
    }

    #[test]
    fn assistant_and_system_messages_map_role_without_tool_result_name() {
        let assistant = AgentMessage::text(AgentRole::Assistant, "Here is the answer.");
        let assistant_json = openai_family_message_json(&assistant);
        assert_eq!(assistant_json["role"], "assistant");
        assert!(assistant_json.get("name").is_none());
        assert_eq!(
            assistant_json["content"].as_str(),
            Some("Here is the answer.")
        );

        let system = AgentMessage::text(AgentRole::System, "Follow safety rules.");
        let system_json = openai_family_message_json(&system);
        assert_eq!(system_json["role"], "system");
        assert!(system_json.get("name").is_none());
        assert_eq!(
            system_json["content"].as_str(),
            Some("Follow safety rules.")
        );
    }

    #[test]
    fn build_openai_chat_messages_prepends_system_prompt() {
        let messages = vec![AgentMessage::text(AgentRole::User, "hello")];
        let out = build_openai_chat_messages("You are Coreside.", &messages);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["role"], "system");
        assert_eq!(out[0]["content"], "You are Coreside.");
        assert_eq!(out[1]["role"], "user");
    }

    #[test]
    fn skipped_image_without_bytes_includes_honest_not_sent_note() {
        let msg = AgentMessage::with_parts(
            AgentRole::User,
            "see attachment",
            vec![AgentContentPart::Image {
                attachment_id: "att-missing".into(),
                mime_type: "image/png".into(),
                data_base64: None,
            }],
        );
        let content = openai_family_message_content(&msg);
        let text = content.as_str().expect("string with skip note");
        assert!(text.contains("not sent to model"));
        assert!(text.contains("att-missing"));
        assert!(!text.contains("[image attachmentId="));
    }

    #[test]
    fn skipped_image_with_text_includes_not_sent_note_in_array() {
        let msg = AgentMessage::with_parts(
            AgentRole::User,
            "see attachment",
            vec![
                AgentContentPart::Text {
                    text: "describe this".into(),
                },
                AgentContentPart::Image {
                    attachment_id: "att-missing".into(),
                    mime_type: "image/png".into(),
                    data_base64: None,
                },
            ],
        );
        let content = openai_family_message_content(&msg);
        assert!(content.is_array());
        let arr = content.as_array().unwrap();
        assert!(arr.iter().any(|p| {
            p["type"] == "text"
                && p["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("not sent to model")
        }));
    }

    #[test]
    fn gpt4o_uses_json_schema_response_format_not_bare_json_object() {
        let fmt = openai_response_format("gpt-4o-2024-08-06");
        assert_eq!(fmt["type"], "json_schema");
        assert_eq!(fmt["json_schema"]["name"], "coreside_agent_response");
        assert_eq!(fmt["json_schema"]["strict"], false);
        assert_eq!(
            fmt["json_schema"]["schema"]["required"]
                .as_array()
                .map(|a| a.len()),
            Some(3)
        );
    }

    #[test]
    fn openrouter_prefixed_gpt4o_uses_json_schema() {
        let fmt = openai_response_format("openai/gpt-4o");
        assert_eq!(fmt["type"], "json_schema");
    }

    #[test]
    fn legacy_turbo_falls_back_to_json_object() {
        let fmt = openai_response_format("gpt-4-turbo");
        assert_eq!(fmt["type"], "json_object");
        assert!(fmt.get("json_schema").is_none());
    }

    #[test]
    fn unknown_compatible_model_falls_back_to_json_object() {
        assert_eq!(
            openai_response_format("my-local-llama")["type"],
            "json_object"
        );
        assert!(!model_supports_json_schema_response_format(""));
    }
}
