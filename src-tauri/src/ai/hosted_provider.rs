//! Hosted Coreside AI via Supabase Edge Function `ai-gateway`.
//! Uses the user JWT from OS keyring — never MCP tokens or desktop provider keys.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::errors::AiError;
use super::provider::{
    AgentMessage, AgentRequest, AgentResponse, AiProvider, ProviderHealth, ProviderStreamEvent,
    ProviderStreamTx, UsageMetadata,
};
use crate::security::redact_secrets;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_STREAM_EVENTS: usize = 50_000;
const MAX_STREAM_TEXT_BYTES: usize = super::http_limits::MAX_PROVIDER_RESPONSE_BYTES;

pub struct HostedAiProvider {
    publishable_key: String,
    supabase_base: String,
    gateway_url: String,
    client: reqwest::Client,
}

/// Build hosted provider when keyring session + publishable Supabase config exist.
pub fn try_from_session() -> Result<Option<Arc<dyn AiProvider>>, AiError> {
    if !crate::credentials::has_stored_session() {
        return Ok(None);
    }
    let Some((supabase_url, publishable_key)) = crate::config::supabase_publishable_config() else {
        return Ok(None);
    };
    Ok(Some(Arc::new(HostedAiProvider::new(
        publishable_key,
        supabase_url,
    ))))
}

impl HostedAiProvider {
    pub fn new(publishable_key: String, supabase_url: String) -> Self {
        let base = supabase_url.trim_end_matches('/').to_string();
        // Fail closed: never fall back to Client::new() (default follows redirects).
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("reqwest Client");
        Self {
            publishable_key,
            supabase_base: base.clone(),
            gateway_url: format!("{base}/functions/v1/ai-gateway"),
            client,
        }
    }

    fn request_client(&self) -> Result<reqwest::Client, AiError> {
        super::platform::validate_and_build_credential_client(
            &self.supabase_base,
            super::platform::EndpointClass::HostedCoresideGateway,
            false,
            false,
            REQUEST_TIMEOUT,
        )
        .map(|(_, c)| c)
        .map_err(|e| AiError::Validation(e.to_string()))
    }

    async fn access_token(&self) -> Result<String, AiError> {
        crate::credentials::ensure_fresh_access_token()
            .await
            .map_err(|e| AiError::NotConfigured(e.to_string()))
    }

    fn build_messages(system_prompt: &str, messages: &[AgentMessage]) -> Vec<Value> {
        super::openai_family::build_openai_chat_messages(system_prompt, messages)
    }

    fn chat_body(request: &AgentRequest, stream: bool) -> Value {
        json!({
            "messages": Self::build_messages(&request.system_prompt, &request.messages),
            "profile": "balanced",
            "idempotencyKey": request
                .idempotency_key
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            "stream": stream
        })
    }

    fn parse_completed_json(payload: &Value) -> Result<String, AiError> {
        payload
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
            .ok_or_else(|| AiError::Parse("Empty Coreside AI response".into()))
            .map(str::to_string)
    }

    fn stream_delta_text(chunk: &Value) -> Option<String> {
        chunk
            .pointer("/choices/0/delta/content")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    async fn handle_sse_data(
        data: &str,
        full_text: &mut String,
        events: &mut usize,
        tx: &ProviderStreamTx,
        redact: Option<&str>,
    ) -> Result<bool, AiError> {
        let data = data.trim();
        if data.is_empty() {
            return Ok(false);
        }
        // Gateway contract: `data: [DONE]` is emitted only after successful settlement.
        // Treat it as the stream terminal; connection close should follow immediately.
        if data == "[DONE]" {
            return Ok(true);
        }
        *events += 1;
        if *events > MAX_STREAM_EVENTS {
            return Err(AiError::Provider("stream event count exceeded".into()));
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            return Ok(false);
        };
        if let Some(delta) = Self::stream_delta_text(&value) {
            if full_text.len().saturating_add(delta.len()) > MAX_STREAM_TEXT_BYTES {
                return Err(AiError::Provider("stream text exceeded byte limit".into()));
            }
            full_text.push_str(&delta);
            let _ = tx
                .send(ProviderStreamEvent::TextDelta { text: delta })
                .await;
        } else if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                if full_text.len().saturating_add(text.len()) > MAX_STREAM_TEXT_BYTES {
                    return Err(AiError::Provider("stream text exceeded byte limit".into()));
                }
                full_text.push_str(text);
                let _ = tx
                    .send(ProviderStreamEvent::TextDelta {
                        text: text.to_string(),
                    })
                    .await;
            }
        } else if value.get("error").is_some() {
            let message = value
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Coreside AI request failed");
            return Err(AiError::Provider(redact_secrets(message, redact)));
        }
        Ok(false)
    }

    async fn consume_sse_gateway_stream(
        &self,
        response: reqwest::Response,
        cancel: &CancellationToken,
        tx: &ProviderStreamTx,
        redact: Option<&str>,
    ) -> Result<String, AiError> {
        let mut buf: Vec<u8> = Vec::new();
        let mut full_text = String::new();
        let mut events = 0usize;
        let mut total_raw = 0usize;
        let mut stream = response.bytes_stream();

        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => {
                    drop(stream);
                    let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                    return Err(AiError::Cancelled);
                }
                item = stream.next() => item,
            };
            match next {
                None => break,
                Some(Err(e)) => {
                    drop(stream);
                    return Err(AiError::Http(redact_secrets(&e.to_string(), redact)));
                }
                Some(Ok(chunk)) => {
                    total_raw = total_raw.saturating_add(chunk.len());
                    if total_raw > MAX_STREAM_TEXT_BYTES.saturating_mul(2) {
                        drop(stream);
                        return Err(AiError::Provider("stream body exceeded byte limit".into()));
                    }
                    if buf.len().saturating_add(chunk.len())
                        > MAX_STREAM_TEXT_BYTES.saturating_mul(2)
                    {
                        drop(stream);
                        return Err(AiError::Provider("stream body exceeded byte limit".into()));
                    }
                    buf.extend_from_slice(&chunk);
                    while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                        let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                        let line = std::str::from_utf8(&line_bytes).map_err(|_| {
                            AiError::Parse("Coreside AI stream contained invalid UTF-8".into())
                        })?;
                        let trimmed = line.trim().trim_end_matches('\r');
                        if trimmed.is_empty() || trimmed.starts_with(':') {
                            continue;
                        }
                        let Some(data) = trimmed.strip_prefix("data:") else {
                            continue;
                        };
                        if Self::handle_sse_data(data, &mut full_text, &mut events, tx, redact)
                            .await?
                        {
                            drop(stream);
                            if full_text.trim().is_empty() {
                                return Err(AiError::Parse("Empty Coreside AI stream".into()));
                            }
                            return Ok(full_text);
                        }
                    }
                }
            }
        }

        drop(stream);
        // Process final line without trailing newline.
        if !buf.is_empty() {
            let line = std::str::from_utf8(&buf)
                .map_err(|_| AiError::Parse("Coreside AI stream contained invalid UTF-8".into()))?;
            let trimmed = line.trim().trim_end_matches('\r');
            if let Some(data) = trimmed.strip_prefix("data:") {
                if Self::handle_sse_data(data, &mut full_text, &mut events, tx, redact).await? {
                    if full_text.trim().is_empty() {
                        return Err(AiError::Parse("Empty Coreside AI stream".into()));
                    }
                    return Ok(full_text);
                }
            }
        }

        Err(AiError::Parse(
            "Coreside AI stream ended without a [DONE] terminal event".into(),
        ))
    }

    async fn post_gateway_json(
        &self,
        body: Value,
        cancel: CancellationToken,
    ) -> Result<Value, AiError> {
        let access_token = self.access_token().await?;
        let client = self.request_client()?;
        let request = client
            .post(&self.gateway_url)
            .bearer_auth(access_token.trim())
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
                        AiError::Http(redact_secrets(&e.to_string(), Some(&access_token)))
                    }
                })?
            }
        };

        let status = response.status();
        let text = super::http_limits::read_response_text_bounded(
            response,
            &cancel,
            super::http_limits::MAX_PROVIDER_RESPONSE_BYTES,
            Some(&access_token),
        )
        .await?;

        if !status.is_success() {
            let consumer = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .unwrap_or_else(|| "Coreside AI request failed".into());
            return Err(AiError::Provider(redact_secrets(
                &consumer,
                Some(&access_token),
            )));
        }

        serde_json::from_str(&text).map_err(|e| {
            AiError::Parse(format!(
                "Invalid Coreside AI JSON: {} — {}",
                e,
                redact_secrets(
                    &text.chars().take(200).collect::<String>(),
                    Some(&access_token)
                )
            ))
        })
    }

    async fn post_gateway_stream(
        &self,
        body: Value,
        cancel: CancellationToken,
        tx: &ProviderStreamTx,
    ) -> Result<String, AiError> {
        let access_token = self.access_token().await?;
        let client = self.request_client()?;
        let request = client
            .post(&self.gateway_url)
            .bearer_auth(access_token.trim())
            .header("apikey", self.publishable_key.trim())
            .header("Content-Type", "application/json")
            .json(&body);

        let response = tokio::select! {
            _ = cancel.cancelled() => {
                let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                return Err(AiError::Cancelled);
            }
            result = request.send() => {
                result.map_err(|e| {
                    if e.is_timeout() {
                        AiError::Timeout
                    } else {
                        AiError::Http(redact_secrets(&e.to_string(), Some(&access_token)))
                    }
                })?
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let text = super::http_limits::read_response_text_bounded(
                response,
                &cancel,
                super::http_limits::MAX_PROVIDER_RESPONSE_BYTES,
                Some(&access_token),
            )
            .await
            .unwrap_or_default();
            let err = AiError::Provider(format!(
                "Coreside AI HTTP {}: {}",
                status.as_u16(),
                redact_secrets(&text, Some(&access_token))
            ));
            let _ = tx
                .send(ProviderStreamEvent::ResponseFailed {
                    code: err.code().to_string(),
                    message: err.to_string(),
                })
                .await;
            return Err(err);
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if content_type.contains("application/json") {
            let text = super::http_limits::read_response_text_bounded(
                response,
                &cancel,
                super::http_limits::MAX_PROVIDER_RESPONSE_BYTES,
                Some(&access_token),
            )
            .await?;
            let payload: Value = serde_json::from_str(&text)
                .map_err(|e| AiError::Parse(format!("Invalid Coreside AI JSON: {e}")))?;
            return Self::parse_completed_json(&payload);
        }

        self.consume_sse_gateway_stream(response, &cancel, tx, Some(&access_token))
            .await
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
        let access_token = self.access_token().await?;
        let client = self.request_client()?;
        let request = client
            .get(&self.gateway_url)
            .bearer_auth(access_token.trim())
            .header("apikey", self.publishable_key.trim())
            .timeout(Duration::from_secs(20));
        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = request.send() => {
                result.map_err(|e| AiError::Http(redact_secrets(&e.to_string(), Some(&access_token))))?
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
        let body = Self::chat_body(&request, false);
        let payload = self.post_gateway_json(body, request.cancel).await?;
        let text = Self::parse_completed_json(&payload)?;
        Ok(AgentResponse {
            raw_text: text,
            usage: UsageMetadata::default(),
            model: "auto".into(),
            provider_id: "coreside_hosted".into(),
        })
    }

    async fn chat_stream(
        &self,
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
            return Err(AiError::Cancelled);
        }
        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: self.provider_id().to_string(),
                model: "auto".into(),
                live: true,
            })
            .await;

        let body = Self::chat_body(&request, true);
        let text = match self
            .post_gateway_stream(body, request.cancel.clone(), &tx)
            .await
        {
            Ok(t) => t,
            Err(err) => {
                if matches!(err, AiError::Cancelled) {
                    let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                } else if !matches!(err, AiError::Cancelled) {
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

        let response = AgentResponse {
            raw_text: text.clone(),
            usage: UsageMetadata::default(),
            model: "auto".into(),
            provider_id: "coreside_hosted".into(),
        };
        let _ = tx.send(ProviderStreamEvent::TextCompleted { text }).await;
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
mod hosted_admission_tests {
    use super::*;
    use serde_json::Value;

    const GATEWAY_CONTRACT: &str =
        include_str!("../../../supabase/functions/ai-gateway/contract.json");
    const FREE_PLAN_MIGRATION: &str = include_str!(
        "../../../supabase/migrations/20260804120000_hosted_ai_free_plan_entitlements.sql"
    );

    #[test]
    fn gateway_contract_documents_free_plan_production_defaults() {
        let contract: Value =
            serde_json::from_str(GATEWAY_CONTRACT).expect("gateway contract json");
        let entitlements = contract.get("entitlements").expect("entitlements section");
        assert_eq!(
            entitlements.get("defaultPlan").and_then(|v| v.as_str()),
            Some("free")
        );
        let defaults = entitlements
            .get("productionDefaults")
            .expect("productionDefaults");
        assert_eq!(
            defaults.get("hosted_ai_enabled").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            defaults.get("allowance_amount").and_then(|v| v.as_i64()),
            Some(0)
        );
    }

    #[test]
    fn free_plan_migration_disables_hosted_ai_by_default() {
        assert!(
            FREE_PLAN_MIGRATION.contains("ai_plan_catalog"),
            "migration should define ai_plan_catalog"
        );
        assert!(
            FREE_PLAN_MIGRATION.contains("('free', 'Free', false, false, 0)"),
            "free plan row should disable hosted AI and search with zero allowance"
        );
        assert!(
            FREE_PLAN_MIGRATION.contains("WHERE plan_id = 'free'"),
            "new-user bootstrap should read free plan from catalog"
        );
    }

    #[test]
    fn gateway_contract_documents_body_byte_limit() {
        let contract: Value =
            serde_json::from_str(GATEWAY_CONTRACT).expect("gateway contract json");
        let limits = contract.get("limits").expect("limits section");
        assert_eq!(
            limits.get("maxBodyBytes").and_then(|v| v.as_i64()),
            Some(262_144)
        );
    }

    #[test]
    fn hosted_sse_delta_parser_extracts_openai_shape() {
        let chunk = json!({
            "choices": [{ "delta": { "content": "hello" } }]
        });
        assert_eq!(
            HostedAiProvider::stream_delta_text(&chunk).as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn hosted_completed_json_prefers_text_over_empty_result_reference() {
        let payload = json!({
            "text": "ok",
            "resultReference": "",
            "status": "completed"
        });
        assert_eq!(
            HostedAiProvider::parse_completed_json(&payload).expect("text"),
            "ok"
        );
    }

    #[test]
    fn hosted_sse_done_marker_is_terminal() {
        // handle_sse_data returns Ok(true) only for [DONE] — EOF without it must fail.
        let chunk = json!({ "choices": [{ "delta": { "content": "x" } }] });
        assert!(HostedAiProvider::stream_delta_text(&chunk).is_some());
    }
}
