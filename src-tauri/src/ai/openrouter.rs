//! OpenRouter provider (OpenAI-compatible chat completions).

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

pub struct OpenRouterProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenRouterProvider {
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

    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn models_url(&self) -> String {
        format!("{}/models", self.base_url)
    }

    fn auth_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let key = self.api_key.trim();
        builder
            .bearer_auth(key)
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://coreside.app")
            .header("X-Title", "Coreside")
    }

    fn build_messages(system_prompt: &str, messages: &[AgentMessage]) -> Vec<Value> {
        let mut out = vec![json!({
            "role": "system",
            "content": system_prompt
        })];
        for m in messages {
            let role = match m.role.as_str() {
                "assistant" | "model" => "assistant",
                "system" => "system",
                _ => "user",
            };
            out.push(json!({
                "role": role,
                "content": m.content
            }));
        }
        out
    }

    async fn post_chat(&self, body: Value, cancel: CancellationToken) -> Result<Value, AiError> {
        let url = self.chat_url();
        let request = self.auth_headers(self.client.post(&url)).json(&body);

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
        let text = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = response.text() => {
                result.map_err(|e| AiError::Http(redact_secrets(&e.to_string(), Some(&self.api_key))))?
            }
        };

        if cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        if !status.is_success() {
            return Err(AiError::Provider(format!(
                "OpenRouter HTTP {}: {}",
                status.as_u16(),
                redact_secrets(&text, Some(&self.api_key))
            )));
        }

        serde_json::from_str(&text).map_err(|e| {
            AiError::Parse(format!(
                "Invalid OpenRouter JSON: {} — {}",
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
            .pointer("/choices/0/message/content")
            .ok_or_else(|| AiError::Parse("Missing choices[0].message.content".into()))?;

        if let Some(s) = content.as_str() {
            if s.trim().is_empty() {
                return Err(AiError::Parse("Empty model text".into()));
            }
            return Ok(s.to_string());
        }

        // Some models return content as an array of parts.
        if let Some(arr) = content.as_array() {
            let mut text = String::new();
            for part in arr {
                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                } else if let Some(t) = part.as_str() {
                    text.push_str(t);
                }
            }
            if text.trim().is_empty() {
                return Err(AiError::Parse("Empty model text parts".into()));
            }
            return Ok(text);
        }

        Err(AiError::Parse("Unexpected content shape".into()))
    }

    fn extract_usage(payload: &Value) -> UsageMetadata {
        let usage = payload.get("usage");
        UsageMetadata {
            prompt_tokens: usage
                .and_then(|u| u.get("prompt_tokens"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            completion_tokens: usage
                .and_then(|u| u.get("completion_tokens"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            total_tokens: usage
                .and_then(|u| u.get("total_tokens"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        }
    }

    /// Lightweight probe — used by health_check fallback (Auto does not probe+chat).
    pub async fn probe_model(&self, cancel: CancellationToken) -> Result<(), AiError> {
        let body = json!({
            "model": self.model,
            "messages": [
                { "role": "user", "content": "ping" }
            ],
            "max_tokens": 8,
            "temperature": 0
        });
        let _ = self.post_chat(body, cancel).await?;
        Ok(())
    }
}

#[async_trait]
impl AiProvider for OpenRouterProvider {
    fn provider_id(&self) -> &str {
        "openrouter"
    }

    fn display_name(&self) -> &str {
        "OpenRouter"
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        let list_url = self.models_url();
        let list_req = self
            .auth_headers(self.client.get(&list_url))
            .timeout(Duration::from_secs(20));

        let list_result = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = list_req.send() => result,
        };

        if let Ok(resp) = list_result {
            if resp.status().is_success() {
                let body: Value = resp.json().await.unwrap_or(json!({}));
                let models = body
                    .get("data")
                    .and_then(|m| m.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                m.get("id").and_then(|n| n.as_str()).map(str::to_string)
                            })
                            .take(40)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                return Ok(ProviderHealth {
                    ok: true,
                    message: format!("Listed {} models", models.len()),
                    models,
                });
            }
        }

        self.probe_model(cancel).await?;
        Ok(ProviderHealth {
            ok: true,
            message: "chat/completions reachable".into(),
            models: vec![self.model.clone()],
        })
    }

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        let messages = Self::build_messages(&request.system_prompt, &request.messages);
        if messages.len() < 2 {
            return Err(AiError::Validation("No messages to send".into()));
        }

        let _ = SCHEMA_VERSION;
        let body = json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.4,
            "response_format": { "type": "json_object" }
        });

        let payload = self.post_chat(body, request.cancel.clone()).await?;
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

    async fn probe(&self, cancel: CancellationToken) -> Result<(), AiError> {
        self.probe_model(cancel).await
    }
}
