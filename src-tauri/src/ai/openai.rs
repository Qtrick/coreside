//! OpenAI provider (chat completions API).

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::provider::{
    AgentMessage, AgentRequest, AgentResponse, AiProvider, ProviderHealth, ProviderStreamEvent,
    ProviderStreamTx, UsageMetadata,
};
use super::response_schema::SCHEMA_VERSION;
use crate::security::redact_secrets;
use futures_util::StreamExt;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_STREAM_EVENTS: usize = 50_000;
const MAX_STREAM_TEXT_BYTES: usize = super::http_limits::MAX_PROVIDER_RESPONSE_BYTES;

pub struct OpenAiProvider {
    api_key: String,
    model: String,
    base_url: String,
    provider_id: &'static str,
    display_name: &'static str,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self::with_identity(api_key, model, base_url, "openai", "OpenAI")
    }

    pub fn new_compatible(api_key: String, model: String, base_url: String) -> Self {
        Self::with_identity(api_key, model, base_url, "compatible", "OpenAI-compatible")
    }

    fn with_identity(
        api_key: String,
        model: String,
        base_url: String,
        provider_id: &'static str,
        display_name: &'static str,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            api_key,
            model,
            base_url: base_url.trim_end_matches('/').to_string(),
            provider_id,
            display_name,
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
        builder
            .bearer_auth(self.api_key.trim())
            .header("Content-Type", "application/json")
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

    async fn post_chat(&self, body: Value, cancel: CancellationToken) -> Result<Value, AiError> {
        let request = self
            .auth_headers(self.client.post(self.chat_url()))
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
                "{} HTTP {}: {}",
                self.display_name,
                status.as_u16(),
                redact_secrets(&text, Some(&self.api_key))
            )));
        }

        serde_json::from_str(&text).map_err(|e| {
            AiError::Parse(format!(
                "Invalid {} JSON: {} — {}",
                self.display_name,
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

    /// Extract assistant content delta from one OpenAI chat.completion.chunk JSON object.
    fn stream_delta_text(chunk: &Value) -> Option<String> {
        chunk
            .pointer("/choices/0/delta/content")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    /// Process one SSE `data:` payload. Returns `Ok(true)` when `[DONE]` ends the stream.
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
                    return Err(AiError::Http(redact_secrets(&e.to_string(), Some(&self.api_key))));
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
                            // Ignore non-data SSE fields (event:, id:, retry:).
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

        // Flush a final unterminated line (providers sometimes omit trailing \n before close).
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
        Ok((full_text, usage, model))
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn provider_id(&self) -> &str {
        self.provider_id
    }

    fn display_name(&self) -> &str {
        self.display_name
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        let list_req = self
            .auth_headers(self.client.get(self.models_url()))
            .timeout(Duration::from_secs(20));
        let list_result = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = list_req.send() => result,
        };
        if let Ok(resp) = list_result {
            if resp.status().is_success() {
                let text = super::http_limits::read_response_text_bounded(
                    resp,
                    &cancel,
                    super::http_limits::MAX_PROVIDER_RESPONSE_BYTES,
                    Some(&self.api_key),
                )
                .await
                .unwrap_or_default();
                let body: Value = serde_json::from_str(&text).unwrap_or(json!({}));
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
        let body = json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": "ping" }],
            "max_tokens": 8,
            "temperature": 0
        });
        let _ = self.post_chat(body, cancel).await?;
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
        // stream_options is OpenAI-specific; many compatible servers 400 on unknown fields.
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.4,
            "stream": true,
            "response_format": { "type": "json_object" }
        });
        if self.provider_id == "openai" {
            body["stream_options"] = json!({ "include_usage": true });
        }
        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: self.provider_id().to_string(),
                model: self.model.clone(),
                live: true,
            })
            .await;

        let http_req = self
            .auth_headers(self.client.post(self.chat_url()))
            .json(&body);
        let response = tokio::select! {
            _ = request.cancel.cancelled() => {
                let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                return Err(AiError::Cancelled);
            }
            result = http_req.send() => {
                result.map_err(|e| {
                    if e.is_timeout() {
                        AiError::Timeout
                    } else {
                        AiError::Http(redact_secrets(&e.to_string(), Some(&self.api_key)))
                    }
                })?
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let text = super::http_limits::read_response_text_bounded(
                response,
                &request.cancel,
                super::http_limits::MAX_PROVIDER_RESPONSE_BYTES,
                Some(&self.api_key),
            )
            .await
            .unwrap_or_default();
            let err = AiError::Provider(format!(
                "{} HTTP {}: {}",
                self.display_name,
                status.as_u16(),
                redact_secrets(&text, Some(&self.api_key))
            ));
            let _ = tx
                .send(ProviderStreamEvent::ResponseFailed {
                    code: err.code().to_string(),
                    message: err.to_string(),
                })
                .await;
            return Err(err);
        }

        let (raw_text, usage, model) = match self
            .consume_sse_chat_stream(response, &request.cancel, &tx)
            .await
        {
            Ok(v) => v,
            Err(err) => {
                if !matches!(err, AiError::Cancelled) {
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

        if raw_text.trim().is_empty() {
            let err = AiError::Parse("Empty streamed model text".into());
            let _ = tx
                .send(ProviderStreamEvent::ResponseFailed {
                    code: err.code().to_string(),
                    message: err.to_string(),
                })
                .await;
            return Err(err);
        }

        let response = AgentResponse {
            raw_text: raw_text.clone(),
            usage,
            model,
            provider_id: self.provider_id().to_string(),
        };
        let _ = tx
            .send(ProviderStreamEvent::TextCompleted {
                text: raw_text,
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

    #[test]
    fn extracts_openai_stream_delta_content() {
        let chunk = json!({
            "choices": [{ "delta": { "content": "Hel" } }]
        });
        assert_eq!(OpenAiProvider::stream_delta_text(&chunk).as_deref(), Some("Hel"));
        let empty = json!({ "choices": [{ "delta": {} }] });
        assert!(OpenAiProvider::stream_delta_text(&empty).is_none());
    }

    #[test]
    fn stream_delta_ignores_non_string_content() {
        let chunk = json!({
            "choices": [{ "delta": { "content": ["parts"] } }]
        });
        assert!(OpenAiProvider::stream_delta_text(&chunk).is_none());
    }
}
