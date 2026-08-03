//! AI provider trait and shared request/response types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::errors::AiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    /// True progressive streaming when the adapter supports it.
    /// Default: honest buffered fallback via `chat` — emits no fake TextDelta events.
    async fn chat_stream(
        &self,
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: self.provider_id().to_string(),
                model: String::new(),
                live: false,
            })
            .await;
        let response = match self.chat(request).await {
            Ok(r) => r,
            Err(err) => {
                let _ = tx
                    .send(ProviderStreamEvent::ResponseFailed {
                        code: err.code().to_string(),
                        message: err.to_string(),
                    })
                    .await;
                return Err(err);
            }
        };
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
