//! AI provider trait and shared request/response types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
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

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn display_name(&self) -> &str;

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError>;

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError>;
}
