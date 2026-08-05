//! AI provider trait and shared request/response types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::structured_user_input::{flatten_parts_for_provider, AgentContentPart, AgentRole};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    pub role: AgentRole,
    /// Display / legacy flattened body. Prefer `provider_text()` when sending upstream.
    pub content: String,
    /// Typed parts (authoritative for StructuredUserInput trust). Empty = text-only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<AgentContentPart>,
    /// OpenAI-family native tool result id (`role=tool`). Sealed per tool round.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl AgentMessage {
    pub fn text(role: AgentRole, content: impl Into<String>) -> Self {
        let content = content.into();
        Self {
            role,
            content,
            parts: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn with_parts(
        role: AgentRole,
        display_content: impl Into<String>,
        parts: Vec<AgentContentPart>,
    ) -> Self {
        let content = display_content.into();
        Self {
            role,
            content,
            parts,
            tool_call_id: None,
        }
    }

    /// Tool-result turn for provider follow-ups (display summary + sealed envelope).
    pub fn tool_result(
        display_summary: impl Into<String>,
        parts: Vec<AgentContentPart>,
        tool_call_id: impl Into<String>,
    ) -> Self {
        Self {
            role: AgentRole::ToolResult,
            content: display_summary.into(),
            parts,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// Upstream provider body for tool-result turns (JSON envelope, not display summary).
    pub fn tool_result_upstream_content(&self) -> String {
        for part in &self.parts {
            if let AgentContentPart::ToolResultEnvelope { envelope_json } = part {
                return envelope_json.clone();
            }
        }
        self.content.clone()
    }

    /// Body for BYOK providers that lack native structured parts.
    pub fn provider_text(&self) -> String {
        if self.role == AgentRole::ToolResult {
            // Display summary only — upstream uses ToolResultEnvelope JSON.
            return self.content.clone();
        }
        if self.parts.is_empty() {
            self.content.clone()
        } else {
            flatten_parts_for_provider(&self.parts)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct AgentRequest {
    pub system_prompt: String,
    pub messages: Vec<AgentMessage>,
    pub cancel: CancellationToken,
    /// Stable identity for one logical user/provider operation. Retries must reuse this key.
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponse {
    pub raw_text: String,
    pub usage: UsageMetadata,
    pub model: String,
    pub provider_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub ok: bool,
    pub message: String,
    pub models: Vec<String>,
}

/// Provider-neutral stream events. Adapters must not leak raw vendor event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProviderStreamEvent {
    ResponseStarted {
        provider_id: String,
        model: String,
        /// `true` only when events are progressive provider deltas.
        live: bool,
    },
    TextDelta {
        text: String,
    },
    TextCompleted {
        text: String,
    },
    ToolCallStarted {
        call_id: String,
        name: String,
    },
    ToolCallArgumentsDelta {
        call_id: String,
        arguments_delta: String,
    },
    ToolCallCompleted {
        call_id: String,
        name: String,
        arguments: String,
    },
    UsageUpdated {
        usage: UsageMetadata,
    },
    ResponseCompleted {
        response: AgentResponse,
        /// Buffered fallback collected a full response without live deltas.
        buffered: bool,
    },
    ResponseCancelled,
    ResponseFailed {
        code: String,
        message: String,
    },
}

pub type ProviderStreamTx = mpsc::Sender<ProviderStreamEvent>;
pub type ProviderStreamRx = mpsc::Receiver<ProviderStreamEvent>;

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn display_name(&self) -> &str;

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError>;

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError>;

    /// Progressive streaming when the adapter supports live deltas.
    ///
    /// Default: honest **buffered** fallback via `chat`:
    /// - `ResponseStarted.live = false`
    /// - **no** `TextDelta` events (do not fabricate typing from a complete body)
    /// - `ResponseCompleted.buffered = true`
    ///
    /// Callers must not treat this path as live SSE. Production `chat_with_auto`
    /// consumes `chat_stream` and only sets `streamed_live` when live deltas arrive.
    async fn chat_stream(
        &self,
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: self.provider_id().to_string(),
                // Model is unknown until `chat` returns; live=false signals buffered.
                model: String::new(),
                live: false,
            })
            .await;
        if request.cancel.is_cancelled() {
            let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
            return Err(AiError::Cancelled);
        }
        let response = match self.chat(request).await {
            Ok(r) => r,
            Err(err) => {
                if matches!(err, AiError::Cancelled) {
                    let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                } else {
                    let _ = tx
                        .send(ProviderStreamEvent::ResponseFailed {
                            code: err.code().to_string(),
                            message: err.to_string(),
                        })
                        .await;
                }
                return Err(err);
            }
        };
        // Single completion only — never slice raw_text into fake TextDelta events.
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
        Ok(response)
    }

    /// Optional cheap availability check. Auto intentionally does not call this
    /// (probe+chat doubles rate-limit usage); kept for health diagnostics.
    #[allow(dead_code)]
    async fn probe(&self, cancel: CancellationToken) -> Result<(), AiError> {
        let _ = cancel;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::capability_registry::{seal_tool_result_envelope, ToolCallResult};
    use serde_json::json;

    #[test]
    fn tool_result_upstream_content_returns_envelope_json() {
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
        let upstream = msg.tool_result_upstream_content();
        assert!(upstream.contains("untrusted_tool_output"));
        assert!(upstream.contains("envelopeHash"));
        assert_ne!(upstream, msg.content);
    }

    #[test]
    fn provider_text_for_tool_result_uses_display_content_not_envelope_flatten() {
        let results = vec![ToolCallResult {
            capability: "web_search".into(),
            ok: true,
            output: json!({"hits": 3}),
            error: None,
            pending_approval: None,
        }];
        let envelope = seal_tool_result_envelope(&results);
        let display = "Tool results (1) (web_search)";
        let msg = AgentMessage::with_parts(AgentRole::ToolResult, display, vec![envelope]);
        let text = msg.provider_text();
        assert_eq!(text, display);
        assert!(!text.contains("untrusted_tool_output"));
        assert!(!text.contains("[tool_result envelope bytes="));
    }

    #[test]
    fn provider_text_for_user_with_parts_flattens_typed_parts() {
        let msg = AgentMessage::with_parts(
            AgentRole::User,
            "ignored when parts present",
            vec![
                AgentContentPart::Text {
                    text: "hello".into(),
                },
                AgentContentPart::Image {
                    attachment_id: "att-1".into(),
                    mime_type: "image/png".into(),
                    data_base64: None,
                },
            ],
        );
        let text = msg.provider_text();
        assert!(text.contains("hello"));
        assert!(text.contains("[image attachmentId=att-1"));
        assert_ne!(text, msg.content);
    }

    #[test]
    fn provider_text_text_only_message_returns_content() {
        let msg = AgentMessage::text(AgentRole::Assistant, "plain body");
        assert_eq!(msg.provider_text(), "plain body");
    }
}
