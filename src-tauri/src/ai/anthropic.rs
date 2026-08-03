//! Anthropic Messages API provider.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::provider::{
    AgentMessage, AgentRequest, AgentResponse, AiProvider, ProviderHealth, UsageMetadata,
};
use super::response_schema::SCHEMA_VERSION;
use crate::security::redact_secrets;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            api_key,
            model,
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }

    fn auth_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .header("x-api-key", self.api_key.trim())
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
    }

    fn build_body(system_prompt: &str, messages: &[AgentMessage], model: &str) -> Value {
        let mut api_messages = Vec::new();
        for m in messages {
            let role = match m.role.as_str() {
                "assistant" | "model" => "assistant",
                // tool_result maps to user for API shape; content is untrusted-enveloped.
                "tool_result" => "user",
                _ => "user",
            };
            api_messages.push(json!({
                "role": role,
                "content": m.content
            }));
        }
        json!({
            "model": model,
            "max_tokens": 8192,
            "temperature": 0.4,
            "system": system_prompt,
            "messages": api_messages
        })
    }

    async fn post_messages(
        &self,
        body: Value,
        cancel: CancellationToken,
    ) -> Result<Value, AiError> {
        let request = self
            .auth_headers(self.client.post(self.messages_url()))
            .json(&body);
        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = request.send() => {
                result.map_err(|e| {
                    if e.is_timeout() {
                        AiError::Timeout
                    } else {
                        AiError::Http(redact_secrets(&e.to_string(), Some(&self.api_key)))
                    }
                })?
            }
        };
        let status = response.status();
        let text = super::http_limits::read_response_text_bounded(
            response,
            &cancel,
            super::http_limits::MAX_PROVIDER_RESPONSE_BYTES,
            Some(&self.api_key),
        )
        .await?;
        if !status.is_success() {
            return Err(AiError::Provider(format!(
                "Anthropic HTTP {}: {}",
                status.as_u16(),
                redact_secrets(&text, Some(&self.api_key))
            )));
        }
        serde_json::from_str(&text).map_err(|e| {
            AiError::Parse(format!(
                "Invalid Anthropic JSON: {} — {}",
                e,
                redact_secrets(
                    &text.chars().take(200).collect::<String>(),
                    Some(&self.api_key)
                )
            ))
        })
    }

    fn extract_text(payload: &Value) -> Result<String, AiError> {
        let content = payload
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| AiError::Parse("Missing content array".into()))?;
        let mut text = String::new();
        for part in content {
            if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                }
            }
        }
        if text.trim().is_empty() {
            return Err(AiError::Parse("Empty model text".into()));
        }
        Ok(text)
    }

    fn extract_usage(payload: &Value) -> UsageMetadata {
        let usage = payload.get("usage");
        let input = usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let output = usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        UsageMetadata {
            prompt_tokens: input,
            completion_tokens: output,
            total_tokens: match (input, output) {
                (Some(a), Some(b)) => Some(a + b),
                _ => None,
            },
        }
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn provider_id(&self) -> &str {
        "anthropic"
    }

    fn display_name(&self) -> &str {
        "Anthropic"
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        let body = Self::build_body(
            "Reply with ok.",
            &[AgentMessage {
                role: "user".into(),
                content: "ping".into(),
            }],
            &self.model,
        );
        // Use a tiny max_tokens override for health.
        let mut body = body;
        if let Some(obj) = body.as_object_mut() {
            obj.insert("max_tokens".into(), json!(16));
        }
        let _ = self.post_messages(body, cancel).await?;
        Ok(ProviderHealth {
            ok: true,
            message: "messages API reachable".into(),
            models: vec![self.model.clone()],
        })
    }

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }
        let _ = SCHEMA_VERSION;
        let body = Self::build_body(&request.system_prompt, &request.messages, &self.model);
        let payload = self.post_messages(body, request.cancel.clone()).await?;
        let raw_text = Self::extract_text(&payload)?;
        let usage = Self::extract_usage(&payload);
        let model = payload
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or(&self.model)
            .to_string();
        Ok(AgentResponse {
            raw_text,
            usage,
            model,
            provider_id: self.provider_id().to_string(),
        })
    }
}
