//! Hosted Coreside AI via Supabase Edge Function `ai-gateway`.
//! Uses the user JWT from OS keyring — never MCP tokens or desktop provider keys.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::errors::AiError;
use super::provider::{
    AgentMessage, AgentRequest, AgentResponse, AiProvider, ProviderHealth, UsageMetadata,
};
use crate::security::redact_secrets;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub struct HostedAiProvider {
    access_token: String,
    publishable_key: String,
    gateway_url: String,
    client: reqwest::Client,
}

/// Build hosted provider when keyring session + publishable Supabase config exist.
pub fn try_from_session() -> Result<Option<Arc<dyn AiProvider>>, AiError> {
    let Some(access_token) = crate::credentials::access_token() else {
        return Ok(None);
    };
    let Some((supabase_url, publishable_key)) = crate::config::supabase_publishable_config() else {
        return Ok(None);
    };
    Ok(Some(Arc::new(HostedAiProvider::new(
        access_token,
        publishable_key,
        supabase_url,
    ))))
}

impl HostedAiProvider {
    pub fn new(access_token: String, publishable_key: String, supabase_url: String) -> Self {
        let base = supabase_url.trim_end_matches('/');
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            access_token,
            publishable_key,
            gateway_url: format!("{base}/functions/v1/ai-gateway"),
            client,
        }
    }

    fn build_messages(system_prompt: &str, messages: &[AgentMessage]) -> Vec<Value> {
        let mut out = vec![json!({
            "role": "system",
            "content": system_prompt
        })];
        for m in messages {
            out.push(json!({
                "role": m.role.as_openai_role(),
                "content": m.provider_text()
            }));
        }
        out
    }

    async fn post_gateway(&self, body: Value, cancel: CancellationToken) -> Result<Value, AiError> {
        let request = self
            .client
            .post(&self.gateway_url)
            .bearer_auth(self.access_token.trim())
            .header("apikey", self.publishable_key.trim())
            .header("Content-Type", "application/json")
            .json(&body);

        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = request.send() => {
                result.map_err(|e| {
                    if e.is_timeout() {
                        AiError::Timeout
                    } else {
                        AiError::Http(redact_secrets(&e.to_string(), Some(&self.access_token)))
                    }
                })?
            }
        };

        let status = response.status();
        let text = super::http_limits::read_response_text_bounded(
            response,
            &cancel,
            super::http_limits::MAX_PROVIDER_RESPONSE_BYTES,
            Some(&self.access_token),
        )
        .await?;

        if !status.is_success() {
            let consumer = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .unwrap_or_else(|| "Coreside AI request failed".into());
            return Err(AiError::Provider(consumer));
        }

        serde_json::from_str(&text).map_err(|e| {
            AiError::Parse(format!(
                "Invalid Coreside AI JSON: {} — {}",
                e,
                redact_secrets(
                    &text.chars().take(200).collect::<String>(),
                    Some(&self.access_token)
                )
            ))
        })
    }
}

#[async_trait]
impl AiProvider for HostedAiProvider {
    fn provider_id(&self) -> &str {
        "coreside_hosted"
    }

    fn display_name(&self) -> &str {
        "Coreside AI"
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        let request = self
            .client
            .get(&self.gateway_url)
            .bearer_auth(self.access_token.trim())
            .header("apikey", self.publishable_key.trim())
            .timeout(Duration::from_secs(20));
        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = request.send() => {
                result.map_err(|e| AiError::Http(redact_secrets(&e.to_string(), Some(&self.access_token))))?
            }
        };
        if response.status().is_success() {
            return Ok(ProviderHealth {
                ok: true,
                message: "Coreside AI gateway reachable".into(),
                models: vec!["auto".into()],
            });
        }
        Err(AiError::Provider("Coreside AI is unavailable".into()))
    }

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }
        let body = json!({
            "messages": Self::build_messages(&request.system_prompt, &request.messages),
            "profile": "balanced",
            "idempotencyKey": request
                .idempotency_key
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            "stream": false
        });
        let payload = self.post_gateway(body, request.cancel).await?;
        // Prefer `text`; accept completed idempotent replays that only echo `resultReference`.
        // Empty `text` must fall through — do not treat "" as a successful Option.
        let text = payload
            .get("text")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                payload
                    .get("resultReference")
                    .and_then(|v| v.as_str())
                    .filter(|s| {
                        payload.get("status").and_then(|st| st.as_str()) == Some("completed")
                            && !s.trim().is_empty()
                    })
            })
            .ok_or_else(|| AiError::Parse("Empty Coreside AI response".into()))?
            .to_string();
        Ok(AgentResponse {
            raw_text: text,
            usage: UsageMetadata::default(),
            model: "auto".into(),
            provider_id: "coreside_hosted".into(),
        })
    }
}
