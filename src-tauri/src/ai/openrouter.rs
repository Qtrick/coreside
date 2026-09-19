//! OpenRouter provider (OpenAI-compatible chat completions).

use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::provider::{
    AgentMessage, AgentRequest, AgentResponse, AiProvider, ProviderHealth, ProviderStreamEvent,
    ProviderStreamTx, UsageMetadata,
};
use super::response_schema::SCHEMA_VERSION;
use crate::security::redact_secrets;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_STREAM_EVENTS: usize = 30_000;
const MAX_STREAM_TEXT_BYTES: usize = 2_000_000;

pub struct OpenRouterProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenRouterProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        // Fail closed: never fall back to Client::new() (default follows redirects).
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("reqwest Client");
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

    fn request_client(&self) -> Result<reqwest::Client, AiError> {
        super::platform::validate_and_build_credential_client(
            &self.base_url,
            super::platform::EndpointClass::VettedRemoteProviderPreset,
            false,
            false,
            REQUEST_TIMEOUT,
        )
        .map(|(_, c)| c)
        .map_err(|e| AiError::Validation(e.to_string()))
    }

    fn build_messages(system_prompt: &str, messages: &[AgentMessage]) -> Vec<Value> {
        super::openai_family::build_openai_chat_messages(system_prompt, messages)
    }

    async fn post_chat(&self, body: Value, cancel: CancellationToken) -> Result<Value, AiError> {
        let url = self.chat_url();
        let client = self.request_client()?;
        let request = self.auth_headers(client.post(&url)).json(&body);

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

    fn stream_delta_text(payload: &Value) -> Option<String> {
        let delta = payload.pointer("/choices/0/delta")?;
        if let Some(s) = delta.get("content").and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
        if let Some(arr) = delta.get("content").and_then(|v| v.as_array()) {
            let mut out = String::new();
            for part in arr {
                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                    out.push_str(t);
                }
            }
            if !out.is_empty() {
                return Some(out);
            }
        }
        None
    }

    async fn handle_sse_data(
        &self,
        data: &str,
        full_text: &mut String,
        usage: &mut UsageMetadata,
        model: &mut String,
        events: &mut usize,
        tx: &ProviderStreamTx,
    ) -> Result<bool, AiError> {
        let data = data.trim();
        if data.is_empty() {
            return Ok(false);
        }
        if data == "[DONE]" {
            return Ok(true);
        }
        *events += 1;
        if *events > MAX_STREAM_EVENTS {
            return Err(AiError::Provider("stream event count exceeded".into()));
        }
        // Malformed JSON: skip the event (do not abort the whole stream).
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            return Ok(false);
        };
        // Check for mid-stream provider error payload
        if let Some(err) = value.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("mid-stream error from provider");
            return Err(AiError::Provider(format!("OpenRouter stream error: {msg}")));
        }
        // Check for finish_reason == "error"
        if let Some(choices) = value.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
                    if reason == "error" {
                        return Err(AiError::Provider(
                            "stream terminated with error finish reason".into(),
                        ));
                    }
                }
            }
        }
        if let Some(m) = value.get("model").and_then(|v| v.as_str()) {
            *model = m.to_string();
        }
        if value.get("usage").is_some() {
            *usage = Self::extract_usage(&value);
            let _ = tx
                .send(ProviderStreamEvent::UsageUpdated {
                    usage: usage.clone(),
                })
                .await;
        }
        if let Some(delta) = Self::stream_delta_text(&value) {
            if full_text.len().saturating_add(delta.len()) > MAX_STREAM_TEXT_BYTES {
                return Err(AiError::Provider("stream text exceeded byte limit".into()));
            }
            full_text.push_str(&delta);
            let _ = tx
                .send(ProviderStreamEvent::TextDelta { text: delta })
                .await;
        }
        Ok(false)
    }

    async fn consume_sse_chat_stream(
        &self,
        response: reqwest::Response,
        cancel: &CancellationToken,
        tx: &ProviderStreamTx,
    ) -> Result<(String, UsageMetadata, String), AiError> {
        let mut buf: Vec<u8> = Vec::new();
        let mut full_text = String::new();
        let mut usage = UsageMetadata::default();
        let mut model = self.model.clone();
        let mut events = 0usize;
        let mut stream = response.bytes_stream();

        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => {
                    let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                    return Err(AiError::Cancelled);
                }
                item = stream.next() => item,
            };
            match next {
                None => break,
                Some(Err(e)) => {
                    return Err(AiError::Http(redact_secrets(
                        &e.to_string(),
                        Some(&self.api_key),
                    )));
                }
                Some(Ok(chunk)) => {
                    if buf.len().saturating_add(chunk.len()) > MAX_STREAM_TEXT_BYTES {
                        return Err(AiError::Provider(format!(
                            "stream exceeded limit of {MAX_STREAM_TEXT_BYTES} bytes"
                        )));
                    }
                    buf.extend_from_slice(&chunk);
                    while let Some(idx) = buf.iter().position(|&b| b == b'\n') {
                        let line_bytes = buf.drain(..=idx).collect::<Vec<u8>>();
                        let line = String::from_utf8_lossy(&line_bytes);
                        let line = line.trim();
                        if line.is_empty() || line.starts_with(':') {
                            continue;
                        }
                        let Some(data) = line.strip_prefix("data:") else {
                            continue;
                        };
                        if self
                            .handle_sse_data(
                                data,
                                &mut full_text,
                                &mut usage,
                                &mut model,
                                &mut events,
                                tx,
                            )
                            .await?
                        {
                            return Ok((full_text, usage, model));
                        }
                    }
                }
            }
        }

        if !buf.is_empty() {
            let line = String::from_utf8_lossy(&buf);
            let line = line.trim();
            if !line.is_empty() && !line.starts_with(':') {
                if let Some(data) = line.strip_prefix("data:") {
                    let _ = self
                        .handle_sse_data(
                            data,
                            &mut full_text,
                            &mut usage,
                            &mut model,
                            &mut events,
                            tx,
                        )
                        .await?;
                }
            }
        }

        if full_text.trim().is_empty() {
            return Err(AiError::Provider(
                "OpenRouter stream completed without text content".into(),
            ));
        }
        Ok((full_text, usage, model))
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
        let client = self.request_client()?;
        let list_req = self
            .auth_headers(client.get(&list_url))
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

    async fn chat_stream(
        &self,
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
            return Err(AiError::Cancelled);
        }
        let messages = Self::build_messages(&request.system_prompt, &request.messages);
        let _ = SCHEMA_VERSION;
        let body = json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.4,
            "stream": true,
            "response_format": { "type": "json_object" }
        });
        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: self.provider_id().to_string(),
                model: self.model.clone(),
                live: true,
            })
            .await;

        let url = self.chat_url();
        let client = self.request_client()?;
        let req = self.auth_headers(client.post(&url)).json(&body);

        let response = tokio::select! {
            _ = request.cancel.cancelled() => {
                let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                return Err(AiError::Cancelled);
            }
            result = req.send() => {
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
        if !status.is_success() {
            let text = super::http_limits::read_response_text_bounded(
                response,
                &request.cancel,
                super::http_limits::MAX_PROVIDER_RESPONSE_BYTES,
                Some(&self.api_key),
            )
            .await
            .unwrap_or_default();
            let err_msg = redact_secrets(&text, Some(&self.api_key));
            let _ = tx
                .send(ProviderStreamEvent::ResponseFailed {
                    code: format!("HTTP_{}", status.as_u16()),
                    message: err_msg.clone(),
                })
                .await;
            return Err(AiError::Provider(format!(
                "OpenRouter HTTP {}: {}",
                status.as_u16(),
                err_msg
            )));
        }

        let (raw_text, usage, model) = self
            .consume_sse_chat_stream(response, &request.cancel, &tx)
            .await?;

        let _ = tx
            .send(ProviderStreamEvent::ResponseCompleted {
                response: AgentResponse {
                    raw_text: raw_text.clone(),
                    usage: usage.clone(),
                    model: model.clone(),
                    provider_id: self.provider_id().to_string(),
                },
                buffered: false,
            })
            .await;

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
