//! Parse and validate structured agent JSON responses.

use serde_json::Value;

use super::response_schema::{
    AgentResponsePayload, ResponseType, SCHEMA_VERSION,
};

#[derive(Debug, Clone)]
pub struct ParsedAgentResponse {
    pub payload: AgentResponsePayload,
    pub recovered: bool,
    pub parse_warnings: Vec<String>,
}

/// Parse JSON agent response. On failure, recover `assistantMessage` when possible.
pub fn parse_agent_response(raw: &str) -> Result<ParsedAgentResponse, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Empty AI response".into());
    }

    // Strip markdown fences if present.
    let json_str = strip_code_fence(trimmed);

    match serde_json::from_str::<AgentResponsePayload>(json_str) {
        Ok(mut payload) => {
            let mut warnings = Vec::new();
            if payload.schema_version != SCHEMA_VERSION && payload.schema_version != "2" {
                warnings.push(format!(
                    "Unexpected schemaVersion '{}', expected '{}' or '2'",
                    payload.schema_version, SCHEMA_VERSION
                ));
                payload.schema_version = SCHEMA_VERSION.to_string();
            }
            match payload.validate() {
                Ok(()) => Ok(ParsedAgentResponse {
                    payload,
                    recovered: false,
                    parse_warnings: warnings,
                }),
                Err(verr) => {
                    // Incomplete tool_change (missing payload/tool) → keep the prose as a message.
                    // Incomplete settings_change (missing/empty payload) → same.
                    // Do not soft-recover protected-id / invalid wallpaper / other validation failures.
                    let incomplete_tool = matches!(payload.response_type, ResponseType::ToolChange)
                        && !payload.assistant_message.trim().is_empty()
                        && (payload.tool_change.is_none()
                            || payload
                                .tool_change
                                .as_ref()
                                .is_some_and(|tc| tc.tool.is_none()));
                    let incomplete_settings =
                        matches!(payload.response_type, ResponseType::SettingsChange)
                            && !payload.assistant_message.trim().is_empty()
                            && (payload.settings_change.is_none()
                                || payload
                                    .settings_change
                                    .as_ref()
                                    .is_some_and(|sc| sc.is_empty()));
                    if incomplete_tool || incomplete_settings {
                        let label = if incomplete_settings {
                            "settings_change"
                        } else {
                            "tool_change"
                        };
                        warnings.push(format!("Recovered incomplete {label}: {verr}"));
                        payload.response_type = ResponseType::Message;
                        if incomplete_tool {
                            payload.tool_change = None;
                        }
                        if incomplete_settings {
                            payload.settings_change = None;
                        }
                        // Drop invalid optional settings payload when recovering a tool miss.
                        if incomplete_tool {
                            if let Some(sc) = &payload.settings_change {
                                if sc.validate().is_err() {
                                    payload.settings_change = None;
                                }
                            }
                        }
                        return Ok(ParsedAgentResponse {
                            payload,
                            recovered: true,
                            parse_warnings: warnings,
                        });
                    }
                    Err(verr)
                }
            }
        }
        Err(primary_err) => {
            // Try lenient Value parse and recover assistantMessage.
            if let Ok(value) = serde_json::from_str::<Value>(json_str) {
                if let Some(recovered) = try_recover_from_value(&value) {
                    recovered.payload.validate()?;
                    return Ok(recovered);
                }
            }

            // Last resort: treat entire text as assistant message.
            let recovered = ParsedAgentResponse {
                payload: AgentResponsePayload {
                    schema_version: SCHEMA_VERSION.to_string(),
                    assistant_message: trimmed.to_string(),
                    response_type: ResponseType::Message,
                    tool_change: None,
                    settings_change: None,
                    tool_calls: None,
                    citations: None,
                    diagnostics: Some(serde_json::json!({
                        "recoveredFromParseError": true,
                        "error": primary_err.to_string(),
                    })),
                    operations: None,
                    silent: None,
                    turn_id: None,
                    assistant_messages: None,
                },
                recovered: true,
                parse_warnings: vec![format!("Recovered plain text after parse error: {primary_err}")],
            };
            Ok(recovered)
        }
    }
}

fn strip_code_fence(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        return rest.trim_end_matches("```").trim();
    }
    if let Some(rest) = s.strip_prefix("```") {
        return rest.trim_end_matches("```").trim();
    }
    s
}

fn try_recover_from_value(value: &Value) -> Option<ParsedAgentResponse> {
    let obj = value.as_object()?;
    let msg = obj
        .get("assistantMessage")
        .or_else(|| obj.get("assistant_message"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if msg.is_empty() {
        return None;
    }

    let response_type = obj
        .get("responseType")
        .or_else(|| obj.get("response_type"))
        .and_then(|v| v.as_str())
        .and_then(|s| match s {
            "message" => Some(ResponseType::Message),
            "tool_change" => Some(ResponseType::ToolChange),
            "tool_use" => Some(ResponseType::ToolUse),
            "settings_change" => Some(ResponseType::SettingsChange),
            "noop" => Some(ResponseType::Noop),
            _ => None,
        })
        .unwrap_or(ResponseType::Message);

    // Full re-parse if possible after fixing types; else message-only recovery.
    if let Ok(mut payload) = serde_json::from_value::<AgentResponsePayload>(value.clone()) {
        if payload.validate().is_ok() {
            return Some(ParsedAgentResponse {
                payload,
                recovered: true,
                parse_warnings: vec!["Recovered via lenient JSON value parse".into()],
            });
        }
        let incomplete = matches!(payload.response_type, ResponseType::ToolChange)
            && !payload.assistant_message.trim().is_empty()
            && (payload.tool_change.is_none()
                || payload
                    .tool_change
                    .as_ref()
                    .is_some_and(|tc| tc.tool.is_none()));
        if incomplete {
            payload.response_type = ResponseType::Message;
            payload.tool_change = None;
            return Some(ParsedAgentResponse {
                payload,
                recovered: true,
                parse_warnings: vec![
                    "Recovered incomplete tool_change via lenient JSON value parse".into(),
                ],
            });
        }
    }

    let safe_type = if matches!(response_type, ResponseType::ToolChange) {
        ResponseType::Message
    } else {
        response_type
    };

    Some(ParsedAgentResponse {
        payload: AgentResponsePayload {
            schema_version: SCHEMA_VERSION.to_string(),
            assistant_message: msg,
            response_type: safe_type,
            tool_change: None,
            settings_change: None,
            tool_calls: None,
            citations: None,
            diagnostics: Some(serde_json::json!({ "recoveredFromPartialJson": true })),
            operations: None,
            silent: None,
            turn_id: None,
            assistant_messages: None,
        },
        recovered: true,
        parse_warnings: vec!["Recovered assistantMessage from partial JSON".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::response_schema::{ToolAction, ToolChangePayload, ToolDefinition};

    #[test]
    fn parses_settings_change_response() {
        let raw = r##"{
            "schemaVersion": "1",
            "assistantMessage": "Switched to dark mode.",
            "responseType": "settings_change",
            "settingsChange": {
                "theme": "dark",
                "accentPrimary": { "light": "#2f6f8f", "dark": "#6ab0d4" },
                "changeSummary": "Dark theme"
            }
        }"##;
        let parsed = parse_agent_response(raw).unwrap();
        assert_eq!(parsed.payload.response_type, ResponseType::SettingsChange);
        let sc = parsed.payload.settings_change.unwrap();
        assert_eq!(sc.theme.as_deref(), Some("dark"));
        assert_eq!(
            sc.accent_primary.as_ref().unwrap().light.as_deref(),
            Some("#2f6f8f")
        );
    }

    #[test]
    fn parses_valid_message_response() {
        let raw = r#"{
            "schemaVersion": "1",
            "assistantMessage": "Hello!",
            "responseType": "message",
            "toolChange": null
        }"#;
        let parsed = parse_agent_response(raw).unwrap();
        assert!(!parsed.recovered);
        assert_eq!(parsed.payload.assistant_message, "Hello!");
        assert_eq!(parsed.payload.response_type, ResponseType::Message);
    }

    #[test]
    fn parses_tool_change_response() {
        let raw = r#"{
            "schemaVersion": "1",
            "assistantMessage": "Created a water tracker.",
            "responseType": "tool_change",
            "toolChange": {
                "action": "create",
                "targetToolId": null,
                "tool": {
                    "id": "tool-water",
                    "name": "Water Tracker",
                    "description": "Track daily water",
                    "layout": { "type": "single-column" },
                    "components": [
                        { "id": "c1", "type": "heading", "props": { "text": "Water" } },
                        { "id": "c2", "type": "counter", "props": { "label": "Glasses" } }
                    ]
                },
                "changeSummary": "Created water tracker"
            }
        }"#;
        let parsed = parse_agent_response(raw).unwrap();
        assert_eq!(parsed.payload.response_type, ResponseType::ToolChange);
        let tc = parsed.payload.tool_change.unwrap();
        assert_eq!(tc.action, ToolAction::Create);
        assert_eq!(tc.tool.unwrap().name, "Water Tracker");
    }

    #[test]
    fn recovers_incomplete_tool_change_to_message() {
        let raw = r#"{
            "schemaVersion": "1",
            "assistantMessage": "I'll build a schedule planner for you.",
            "responseType": "tool_change"
        }"#;
        let parsed = parse_agent_response(raw).unwrap();
        assert!(parsed.recovered);
        assert_eq!(parsed.payload.response_type, ResponseType::Message);
        assert!(parsed.payload.tool_change.is_none());
        assert!(parsed.payload.assistant_message.contains("schedule planner"));
    }

    #[test]
    fn recovers_assistant_message_on_bad_json() {
        let raw = "Sure, I can help with that.";
        let parsed = parse_agent_response(raw).unwrap();
        assert!(parsed.recovered);
        assert_eq!(parsed.payload.assistant_message, "Sure, I can help with that.");
        assert_eq!(parsed.payload.response_type, ResponseType::Message);
    }

    #[test]
    fn recovers_from_fenced_json() {
        let raw = "```json\n{\"schemaVersion\":\"1\",\"assistantMessage\":\"Hi\",\"responseType\":\"noop\"}\n```";
        let parsed = parse_agent_response(raw).unwrap();
        assert!(!parsed.recovered);
        assert_eq!(parsed.payload.response_type, ResponseType::Noop);
    }

    #[test]
    fn parses_tool_use_response() {
        let raw = r#"{
            "schemaVersion": "1",
            "assistantMessage": "Let me search the web for that.",
            "responseType": "tool_use",
            "toolCalls": [
                { "capability": "web_search", "arguments": { "query": "rust async book" } }
            ]
        }"#;
        let parsed = parse_agent_response(raw).unwrap();
        assert_eq!(parsed.payload.response_type, ResponseType::ToolUse);
        let calls = parsed.payload.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].capability, "web_search");
    }

    #[test]
    fn rejects_tool_change_without_tool() {
        let payload = AgentResponsePayload {
            schema_version: "1".into(),
            assistant_message: "x".into(),
            response_type: ResponseType::ToolChange,
            tool_change: Some(ToolChangePayload {
                action: ToolAction::Create,
                target_tool_id: None,
                tool: None,
                change_summary: "x".into(),
            }),
            settings_change: None,
            tool_calls: None,
            citations: None,
            diagnostics: None,
            operations: None,
            silent: None,
            turn_id: None,
            assistant_messages: None,
        };
        assert!(payload.validate().is_err());
    }

    #[test]
    fn rejects_protected_core_tool_ids() {
        let payload = AgentResponsePayload {
            schema_version: "1".into(),
            assistant_message: "Updating branding".into(),
            response_type: ResponseType::ToolChange,
            tool_change: Some(ToolChangePayload {
                action: ToolAction::Create,
                target_tool_id: None,
                tool: Some(ToolDefinition {
                    id: "core.branding".into(),
                    name: "Branding".into(),
                    description: String::new(),
                    layout: serde_json::json!({ "type": "single-column" }),
                    components: vec![],
                }),
                change_summary: "brand".into(),
            }),
            settings_change: None,
            tool_calls: None,
            citations: None,
            diagnostics: None,
            operations: None,
            silent: None,
            turn_id: None,
            assistant_messages: None,
        };
        let err = payload.validate().unwrap_err();
        assert!(err.contains("Protected core resource"), "{err}");

        let raw = r#"{
            "schemaVersion": "1",
            "assistantMessage": "Changing appearance",
            "responseType": "tool_change",
            "toolChange": {
                "action": "create",
                "tool": {
                    "id": "core.settings.appearance",
                    "name": "Appearance",
                    "description": "",
                    "layout": "single-column",
                    "components": []
                },
                "changeSummary": "nope"
            }
        }"#;
        assert!(parse_agent_response(raw).is_err());
    }

    #[test]
    fn validates_update_requires_target() {
        let payload = AgentResponsePayload {
            schema_version: "1".into(),
            assistant_message: "Updated".into(),
            response_type: ResponseType::ToolChange,
            tool_change: Some(ToolChangePayload {
                action: ToolAction::Update,
                target_tool_id: None,
                tool: Some(ToolDefinition {
                    id: "t1".into(),
                    name: "T".into(),
                    description: String::new(),
                    layout: serde_json::json!({ "type": "single-column" }),
                    components: vec![],
                }),
                change_summary: "u".into(),
            }),
            settings_change: None,
            tool_calls: None,
            citations: None,
            diagnostics: None,
            operations: None,
            silent: None,
            turn_id: None,
            assistant_messages: None,
        };
        assert!(payload.validate().is_err());
    }

    #[test]
    fn accepts_string_layout_and_normalizes() {
        let raw = r#"{
            "schemaVersion": "1",
            "assistantMessage": "Created.",
            "responseType": "tool_change",
            "toolChange": {
                "action": "create",
                "targetToolId": null,
                "tool": {
                    "id": "tool-x",
                    "name": "X",
                    "description": "",
                    "layout": "stack",
                    "components": [
                        {
                            "id": "x-heading",
                            "type": "heading",
                            "props": { "text": "X" }
                        }
                    ]
                },
                "changeSummary": "create"
            }
        }"#;
        let mut parsed = parse_agent_response(raw).unwrap();
        parsed.payload.normalize_for_frontend();
        let layout = &parsed
            .payload
            .tool_change
            .as_ref()
            .unwrap()
            .tool
            .as_ref()
            .unwrap()
            .layout;
        assert_eq!(layout["type"], "stack");
    }

    #[test]
    fn tool_action_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&ToolAction::Create).unwrap(),
            "\"create\""
        );
        assert_eq!(
            serde_json::to_string(&ToolAction::Update).unwrap(),
            "\"update\""
        );
        assert_eq!(
            serde_json::to_string(&ToolAction::Replace).unwrap(),
            "\"replace\""
        );
        assert_eq!(
            serde_json::to_string(&ResponseType::ToolChange).unwrap(),
            "\"tool_change\""
        );
    }
}
