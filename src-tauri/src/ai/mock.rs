//! Mock AI provider with fixture responses for tests and offline use.

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::provider::{AgentRequest, AgentResponse, AiProvider, ProviderHealth, UsageMetadata};
use super::response_schema::SCHEMA_VERSION;

pub struct MockAiProvider;

impl MockAiProvider {
    pub fn new() -> Self {
        Self
    }

    fn fixture_for(user_text: &str) -> String {
        let lower = user_text.to_lowercase();

        if lower.contains("water") || lower.contains("hydrat") {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "I created a simple water tracker for you. Review the preview and apply when ready.",
                "responseType": "tool_change",
                "toolChange": {
                    "action": "create",
                    "targetToolId": null,
                    "changeSummary": "Create a daily water intake tracker",
                    "tool": {
                        "id": "tool-water-tracker",
                        "name": "Water Tracker",
                        "description": "Track daily glasses of water",
                        "layout": { "type": "single-column" },
                        "components": [
                            { "id": "wt-heading", "type": "heading", "props": { "text": "Water Tracker", "level": 1 } },
                            { "id": "wt-stat", "type": "stat", "props": { "label": "Today", "valueKey": "glasses" } },
                            { "id": "wt-counter", "type": "counter", "props": { "label": "Glasses", "stateKey": "glasses", "min": 0, "max": 20 } },
                            { "id": "wt-progress", "type": "progress", "props": { "label": "Goal", "valueKey": "glasses", "max": 8 } },
                            {
                                "id": "wt-buttons",
                                "type": "buttonGroup",
                                "props": {},
                                "children": [
                                    {
                                        "id": "wt-add",
                                        "type": "button",
                                        "props": { "label": "+1" },
                                        "actions": [{ "type": "increment", "target": "glasses", "amount": 1 }]
                                    },
                                    {
                                        "id": "wt-minus",
                                        "type": "button",
                                        "props": { "label": "−1" },
                                        "actions": [{ "type": "decrement", "target": "glasses", "amount": 1 }]
                                    },
                                    {
                                        "id": "wt-reset",
                                        "type": "button",
                                        "props": { "label": "Reset" },
                                        "actions": [{ "type": "reset", "target": "glasses", "value": 0 }]
                                    }
                                ]
                            }
                        ]
                    }
                },
                "diagnostics": { "fixture": "water_tracker" }
            })
            .to_string();
        }

        if lower.contains("quiz") {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "Here's a short quiz tool. Apply it to open and try the questions.",
                "responseType": "tool_change",
                "toolChange": {
                    "action": "create",
                    "targetToolId": null,
                    "changeSummary": "Create a sample quiz",
                    "tool": {
                        "id": "tool-sample-quiz",
                        "name": "Sample Quiz",
                        "description": "A short knowledge check",
                        "layout": { "type": "single-column" },
                        "components": [
                            { "id": "qz-heading", "type": "heading", "props": { "text": "Quick Quiz", "level": 1 } },
                            {
                                "id": "qz-quiz",
                                "type": "quiz",
                                "props": {
                                    "questions": [
                                        {
                                            "id": "q1",
                                            "prompt": "What is the capital of France?",
                                            "options": ["Berlin", "Paris", "Rome"],
                                            "answer": "Paris",
                                            "explanation": "Paris is the capital of France."
                                        },
                                        {
                                            "id": "q2",
                                            "prompt": "Which ocean is the largest?",
                                            "options": ["Atlantic", "Indian", "Pacific"],
                                            "answer": "Pacific"
                                        },
                                        {
                                            "id": "q3",
                                            "prompt": "Mount Everest is in which mountain range?",
                                            "options": ["Andes", "Alps", "Himalayas"],
                                            "answer": "Himalayas"
                                        }
                                    ]
                                }
                            }
                        ]
                    }
                },
                "diagnostics": { "fixture": "quiz" }
            })
            .to_string();
        }

        if lower.contains("todo") || lower.contains("checklist") || lower.contains("task") {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "I drafted a checklist tool for your tasks.",
                "responseType": "tool_change",
                "toolChange": {
                    "action": "create",
                    "targetToolId": null,
                    "changeSummary": "Create a personal checklist",
                    "tool": {
                        "id": "tool-checklist",
                        "name": "Checklist",
                        "description": "Track personal tasks",
                        "layout": { "type": "single-column" },
                        "components": [
                            { "id": "cl-heading", "type": "heading", "props": { "text": "Tasks", "level": 1 } },
                            { "id": "cl-list", "type": "checklist", "props": { "stateKey": "items", "placeholder": "Add a task" } },
                            { "id": "cl-input", "type": "textInput", "props": { "label": "New task", "stateKey": "draft" } },
                            { "id": "cl-add", "type": "button", "props": { "label": "Add", "actions": [{ "type": "appendItem", "target": "items", "item": { "fromState": "draft" } }] } }
                        ]
                    }
                },
                "diagnostics": { "fixture": "checklist" }
            })
            .to_string();
        }

        if lower.contains("progress")
            || lower.contains("encourag")
            || lower.contains("goal message")
        {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "I updated your water tracker with a progress cue and a goal celebration message.",
                "responseType": "tool_change",
                "toolChange": {
                    "action": "update",
                    "targetToolId": "tool-water-tracker",
                    "changeSummary": "Add progress encouragement",
                    "tool": {
                        "id": "tool-water-tracker",
                        "name": "Water Tracker",
                        "description": "Track daily glasses of water",
                        "layout": { "type": "single-column" },
                        "components": [
                            { "id": "wt-heading", "type": "heading", "props": { "text": "Water Tracker", "level": 1 } },
                            { "id": "wt-stat", "type": "stat", "props": { "label": "Today", "valueKey": "glasses" } },
                            { "id": "wt-counter", "type": "counter", "props": { "label": "Glasses", "stateKey": "glasses", "min": 0, "max": 20 } },
                            { "id": "wt-progress", "type": "progress", "props": { "label": "Goal", "valueKey": "glasses", "max": 8 } },
                            { "id": "wt-encourage", "type": "text", "props": { "text": "Nice work — you reached your daily goal!" } },
                            {
                                "id": "wt-buttons",
                                "type": "buttonGroup",
                                "props": {},
                                "children": [
                                    {
                                        "id": "wt-add",
                                        "type": "button",
                                        "props": { "label": "+1" },
                                        "actions": [{ "type": "increment", "target": "glasses", "amount": 1 }]
                                    },
                                    {
                                        "id": "wt-minus",
                                        "type": "button",
                                        "props": { "label": "−1" },
                                        "actions": [{ "type": "decrement", "target": "glasses", "amount": 1 }]
                                    },
                                    {
                                        "id": "wt-reset",
                                        "type": "button",
                                        "props": { "label": "Reset" },
                                        "actions": [{ "type": "reset", "target": "glasses", "value": 0 }]
                                    }
                                ]
                            }
                        ]
                    }
                },
                "diagnostics": { "fixture": "water_tracker_update" }
            })
            .to_string();
        }

        if lower.contains("noop") || lower.contains("never mind") {
            return json!({
                "schemaVersion": SCHEMA_VERSION,
                "assistantMessage": "Okay — no changes.",
                "responseType": "noop",
                "toolChange": null,
                "diagnostics": { "fixture": "noop" }
            })
            .to_string();
        }

        json!({
            "schemaVersion": SCHEMA_VERSION,
            "assistantMessage": format!(
                "I'm the Coreside mock agent. Ask me to build something like a water tracker, quiz, or checklist.\n\nYou said: {}",
                user_text.chars().take(200).collect::<String>()
            ),
            "responseType": "message",
            "toolChange": null,
            "diagnostics": { "fixture": "default_message" }
        })
        .to_string()
    }
}

impl Default for MockAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiProvider for MockAiProvider {
    fn provider_id(&self) -> &str {
        "mock"
    }

    fn display_name(&self) -> &str {
        "Mock AI"
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        if cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }
        Ok(ProviderHealth {
            ok: true,
            message: "Mock provider ready".into(),
            models: vec!["mock-fixture".into()],
        })
    }

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        let user_text = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("");

        // Tiny yield so cancellation can race in tests.
        tokio::task::yield_now().await;
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        Ok(AgentResponse {
            raw_text: Self::fixture_for(user_text),
            usage: UsageMetadata {
                prompt_tokens: Some(10),
                completion_tokens: Some(50),
                total_tokens: Some(60),
            },
            model: "mock-fixture".into(),
            provider_id: self.provider_id().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::response_schema::ResponseType;
    use crate::ai::{parse_agent_response, AgentMessage};

    #[tokio::test]
    async fn water_fixture_parses_as_tool_change() {
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage {
                    role: "user".into(),
                    content: "Create a simple water tracker".into(),
                }],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        assert_eq!(parsed.payload.response_type, ResponseType::ToolChange);
        let tool = parsed
            .payload
            .tool_change
            .as_ref()
            .unwrap()
            .tool
            .as_ref()
            .unwrap();
        assert_eq!(tool.id, "tool-water-tracker");
        assert!(tool
            .components
            .iter()
            .any(|c| c.component_type == "counter"));
    }

    #[tokio::test]
    async fn quiz_fixture_has_three_questions() {
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage {
                    role: "user".into(),
                    content: "Create a geography quiz".into(),
                }],
                cancel: CancellationToken::new(),
                idempotency_key: None,
            })
            .await
            .unwrap();
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        let tool = parsed
            .payload
            .tool_change
            .as_ref()
            .unwrap()
            .tool
            .as_ref()
            .unwrap();
        let quiz = tool
            .components
            .iter()
            .find(|c| c.component_type == "quiz")
            .unwrap();
        let questions = quiz.props.as_ref().unwrap()["questions"]
            .as_array()
            .unwrap();
        assert_eq!(questions.len(), 3);
    }
}
