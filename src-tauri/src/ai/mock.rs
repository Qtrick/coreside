//! Mock AI provider with fixture responses for tests and offline use.

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::provider::{
    AgentRequest, AgentResponse, AiProvider, ProviderHealth, ProviderStreamEvent, ProviderStreamTx,
    UsageMetadata,
};
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
            .find(|m| m.role == crate::ai::AgentRole::User)
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

    /// Live stream fixtures:
    /// - `live stream probe` — text-only progressive TextDelta (TS-1)
    /// - `progressive op preview` — NDJSON StreamEvent op frame before completion (RC3.3)
    async fn chat_stream(
        &self,
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        let user_text = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == crate::ai::AgentRole::User)
            .map(|m| m.content.as_str())
            .unwrap_or("");
        let lower = user_text.to_lowercase();
        let progressive_ops = lower.contains("progressive op preview");
        let live = progressive_ops || lower.contains("live stream probe");
        if request.cancel.is_cancelled() {
            let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
            return Err(AiError::Cancelled);
        }

        if !live {
            // Honest buffered fallback — no fake TextDelta (same contract as trait default).
            let _ = tx
                .send(ProviderStreamEvent::ResponseStarted {
                    provider_id: self.provider_id().to_string(),
                    model: String::new(),
                    live: false,
                })
                .await;
            let response = self.chat(request).await.map_err(|err| {
                // Best-effort terminal event; ignore send failures on closed channel.
                let ev = if matches!(err, AiError::Cancelled) {
                    ProviderStreamEvent::ResponseCancelled
                } else {
                    ProviderStreamEvent::ResponseFailed {
                        code: err.code().to_string(),
                        message: err.to_string(),
                    }
                };
                let _ = tx.try_send(ev);
                err
            })?;
            let _ = tx
                .send(ProviderStreamEvent::TextCompleted {
                    text: response.raw_text.clone(),
                })
                .await;
            let _ = tx
                .send(ProviderStreamEvent::ResponseCompleted {
                    response: response.clone(),
                    buffered: true,
                })
                .await;
            return Ok(response);
        }

        if progressive_ops {
            return Self::stream_progressive_op_preview(request, tx).await;
        }

        let raw = json!({
            "schemaVersion": SCHEMA_VERSION,
            "assistantMessage": "Live stream probe complete",
            "responseType": "message",
            "diagnostics": { "fixture": "live_stream_probe" }
        })
        .to_string();

        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: self.provider_id().to_string(),
                model: "mock-fixture".into(),
                live: true,
            })
            .await;
        let _ = tx
            .send(ProviderStreamEvent::TextDelta {
                text: r#"{"assistantMessage":"Live stream"#.into(),
            })
            .await;

        tokio::select! {
            _ = request.cancel.cancelled() => {
                let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                return Err(AiError::Cancelled);
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(120)) => {}
        }

        let _ = tx
            .send(ProviderStreamEvent::TextDelta {
                text: r#" probe complete","responseType":"message"}"#.into(),
            })
            .await;

        let response = AgentResponse {
            raw_text: raw.clone(),
            usage: UsageMetadata {
                prompt_tokens: Some(10),
                completion_tokens: Some(20),
                total_tokens: Some(30),
            },
            model: "mock-fixture".into(),
            provider_id: self.provider_id().to_string(),
        };
        let _ = tx
            .send(ProviderStreamEvent::TextCompleted { text: raw })
            .await;
        let _ = tx
            .send(ProviderStreamEvent::ResponseCompleted {
                response: response.clone(),
                buffered: false,
            })
            .await;
        Ok(response)
    }
}

impl MockAiProvider {
    /// NDJSON `operation.frame_completed` arrives as TextDelta before ResponseCompleted.
    async fn stream_progressive_op_preview(
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        let op_frame = json!({
            "type": "operation.frame_completed",
            "operation": {
                "id": "op-progressive-preview",
                "type": "chat.status",
                "target": {},
                "payload": { "message": "progressive preview" }
            }
        })
        .to_string()
            + "\n";

        let raw = json!({
            // Runtime V2 apply path requires schemaVersion "2" (distinct from SCHEMA_VERSION="1").
            "schemaVersion": "2",
            "assistantMessage": "Progressive op preview complete",
            "responseType": "message",
            "operations": [],
            "diagnostics": { "fixture": "progressive_op_preview" }
        })
        .to_string();

        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: "mock".into(),
                model: "mock-fixture".into(),
                live: true,
            })
            .await;
        // Split the frame across two deltas so the parser buffers until newline.
        let mid = op_frame.len() / 2;
        let _ = tx
            .send(ProviderStreamEvent::TextDelta {
                text: op_frame[..mid].to_string(),
            })
            .await;

        tokio::select! {
            _ = request.cancel.cancelled() => {
                let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                return Err(AiError::Cancelled);
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(80)) => {}
        }

        let _ = tx
            .send(ProviderStreamEvent::TextDelta {
                text: op_frame[mid..].to_string(),
            })
            .await;

        let response = AgentResponse {
            raw_text: raw.clone(),
            usage: UsageMetadata {
                prompt_tokens: Some(10),
                completion_tokens: Some(24),
                total_tokens: Some(34),
            },
            model: "mock-fixture".into(),
            provider_id: "mock".into(),
        };
        let _ = tx
            .send(ProviderStreamEvent::TextCompleted {
                text: raw.clone(),
            })
            .await;
        let _ = tx
            .send(ProviderStreamEvent::ResponseCompleted {
                response: response.clone(),
                buffered: false,
            })
            .await;
        Ok(response)
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
                messages: vec![AgentMessage::text(
                    crate::ai::AgentRole::User,
                    "Create a simple water tracker",
                )],
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
    async fn live_stream_probe_emits_delta_before_completion() {
        let provider = MockAiProvider::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let cancel = CancellationToken::new();
        let request = AgentRequest {
            system_prompt: "test".into(),
            messages: vec![AgentMessage::text(
                crate::ai::AgentRole::User,
                "please run live stream probe now",
            )],
            cancel: cancel.clone(),
            idempotency_key: Some("probe".into()),
        };
        let started = std::time::Instant::now();
        let join = tokio::spawn(async move { provider.chat_stream(request, tx).await });
        let mut first_delta_at = None;
        while let Some(ev) = rx.recv().await {
            if matches!(ev, ProviderStreamEvent::TextDelta { .. }) && first_delta_at.is_none() {
                first_delta_at = Some(started.elapsed());
            }
            if matches!(ev, ProviderStreamEvent::ResponseCompleted { .. }) {
                break;
            }
        }
        let _ = join.await.unwrap().unwrap();
        let first = first_delta_at.expect("expected live TextDelta");
        assert!(
            first < std::time::Duration::from_millis(80),
            "probe delta should arrive before delayed completion, got {first:?}"
        );
    }

    #[tokio::test]
    async fn progressive_op_preview_completes_ndjson_frame_before_response_completed() {
        use crate::runtime_v2::{ingest_live_chunk, NdjsonFrameParser, PreviewTransaction};

        let provider = MockAiProvider::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let request = AgentRequest {
            system_prompt: "test".into(),
            messages: vec![AgentMessage::text(
                crate::ai::AgentRole::User,
                "please run progressive op preview now",
            )],
            cancel: CancellationToken::new(),
            idempotency_key: Some("progressive".into()),
        };
        let join = tokio::spawn(async move { provider.chat_stream(request, tx).await });

        let mut parser = NdjsonFrameParser::new();
        let mut preview = PreviewTransaction::new("test-turn", None);
        let mut saw_preview = false;
        let mut preview_before_complete = false;

        while let Some(ev) = rx.recv().await {
            match ev {
                ProviderStreamEvent::TextDelta { text } => {
                    let events = ingest_live_chunk(&mut parser, &mut preview, &text);
                    if events.iter().any(|e| e.status == "preview") {
                        saw_preview = true;
                    }
                }
                ProviderStreamEvent::ResponseCompleted { .. } => {
                    preview_before_complete = saw_preview;
                    break;
                }
                _ => {}
            }
        }
        let response = join.await.unwrap().unwrap();
        assert!(
            preview_before_complete,
            "Operation preview must fire before ResponseCompleted"
        );
        assert_eq!(preview.accepted_operations().len(), 1);
        assert_eq!(
            preview.accepted_operations()[0].id,
            "op-progressive-preview"
        );
        let parsed = parse_agent_response(&response.raw_text).unwrap();
        assert_eq!(parsed.payload.schema_version, "2");
        assert!(parsed
            .payload
            .assistant_message
            .contains("Progressive op preview complete"));
    }

    #[tokio::test]
    async fn buffered_chat_stream_has_no_fake_deltas() {
        let provider = MockAiProvider::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let request = AgentRequest {
            system_prompt: "test".into(),
            messages: vec![AgentMessage::text(crate::ai::AgentRole::User, "hello")],
            cancel: CancellationToken::new(),
            idempotency_key: None,
        };
        let join = tokio::spawn(async move { provider.chat_stream(request, tx).await });
        let mut saw_delta = false;
        let mut buffered_complete = false;
        while let Some(ev) = rx.recv().await {
            match ev {
                ProviderStreamEvent::TextDelta { .. } => saw_delta = true,
                ProviderStreamEvent::ResponseCompleted { buffered, .. } => {
                    buffered_complete = buffered;
                    break;
                }
                _ => {}
            }
        }
        let _ = join.await.unwrap().unwrap();
        assert!(!saw_delta, "buffered path must not fabricate TextDelta");
        assert!(buffered_complete);
    }

    #[tokio::test]
    async fn quiz_fixture_has_three_questions() {
        let provider = MockAiProvider::new();
        let response = provider
            .chat(AgentRequest {
                system_prompt: "test".into(),
                messages: vec![AgentMessage::text(
                    crate::ai::AgentRole::User,
                    "Create a geography quiz",
                )],
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
