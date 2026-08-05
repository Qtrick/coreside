//! Anthropic Messages API provider.

use std::collections::HashMap;
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
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_STREAM_EVENTS: usize = 50_000;
const MAX_STREAM_TEXT_BYTES: usize = super::http_limits::MAX_PROVIDER_RESPONSE_BYTES;

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

struct ActiveToolCall {
    call_id: String,
    name: String,
    arguments: String,
}

impl AnthropicProvider {
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
        let api_messages: Vec<Value> = messages
            .iter()
            .map(super::anthropic_family::anthropic_message_json)
            .collect();
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
        Self::usage_from_value(payload.get("usage"))
    }

    fn usage_from_value(usage: Option<&Value>) -> UsageMetadata {
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

    /// Extract `text_delta` text from a content_block_delta event JSON object.
    fn stream_text_delta(event: &Value) -> Option<String> {
        if event.get("type").and_then(|t| t.as_str()) != Some("content_block_delta") {
            return None;
        }
        let delta = event.get("delta")?;
        if delta.get("type").and_then(|t| t.as_str()) != Some("text_delta") {
            return None;
        }
        delta
            .get("text")
            .and_then(|t| t.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    /// Process one SSE `data:` payload. Returns `Ok(true)` when the stream should end.
    async fn handle_sse_data(
        &self,
        data: &str,
        full_text: &mut String,
        usage: &mut UsageMetadata,
        model: &mut String,
        events: &mut usize,
        active_tools: &mut HashMap<usize, ActiveToolCall>,
        tx: &ProviderStreamTx,
    ) -> Result<bool, AiError> {
        let data = data.trim();
        if data.is_empty() {
            return Ok(false);
        }
        *events += 1;
        if *events > MAX_STREAM_EVENTS {
            return Err(AiError::Provider("stream event count exceeded".into()));
        }
        // Malformed JSON: skip the event (do not abort the whole stream).
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            return Ok(false);
        };
        let event_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match event_type {
            "ping" => Ok(false),
            "error" => {
                let message = value
                    .pointer("/error/message")
                    .and_then(|m| m.as_str())
                    .or_else(|| value.get("message").and_then(|m| m.as_str()))
                    .unwrap_or("Anthropic stream error");
                Err(AiError::Provider(redact_secrets(
                    message,
                    Some(&self.api_key),
                )))
            }
            "message_start" => {
                if let Some(m) = value.pointer("/message/model").and_then(|v| v.as_str()) {
                    *model = m.to_string();
                }
                if value.pointer("/message/usage").is_some() {
                    *usage = Self::usage_from_value(value.pointer("/message/usage"));
                    let _ = tx
                        .send(ProviderStreamEvent::UsageUpdated {
                            usage: usage.clone(),
                        })
                        .await;
                }
                Ok(false)
            }
            "content_block_start" => {
                let index = value
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let block = value.get("content_block");
                let block_type = block
                    .and_then(|b| b.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if block_type == "tool_use" {
                    let call_id = block
                        .and_then(|b| b.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .and_then(|b| b.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if !call_id.is_empty() && !name.is_empty() {
                        active_tools.insert(
                            index,
                            ActiveToolCall {
                                call_id: call_id.clone(),
                                name: name.clone(),
                                arguments: String::new(),
                            },
                        );
                        let _ = tx
                            .send(ProviderStreamEvent::ToolCallStarted { call_id, name })
                            .await;
                    }
                }
                // text blocks: do not invent TextDelta from empty start text
                Ok(false)
            }
            "content_block_delta" => {
                if let Some(delta) = Self::stream_text_delta(&value) {
                    if full_text.len().saturating_add(delta.len()) > MAX_STREAM_TEXT_BYTES {
                        return Err(AiError::Provider("stream text exceeded byte limit".into()));
                    }
                    full_text.push_str(&delta);
                    let _ = tx
                        .send(ProviderStreamEvent::TextDelta { text: delta })
                        .await;
                    return Ok(false);
                }
                // tool_use input_json_delta — never flatten into assistant text
                let delta = value.get("delta");
                if delta.and_then(|d| d.get("type")).and_then(|t| t.as_str())
                    == Some("input_json_delta")
                {
                    let index = value
                        .get("index")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as usize)
                        .unwrap_or(0);
                    if let Some(partial) = delta
                        .and_then(|d| d.get("partial_json"))
                        .and_then(|v| v.as_str())
                    {
                        if let Some(tool) = active_tools.get_mut(&index) {
                            tool.arguments.push_str(partial);
                            let _ = tx
                                .send(ProviderStreamEvent::ToolCallArgumentsDelta {
                                    call_id: tool.call_id.clone(),
                                    arguments_delta: partial.to_string(),
                                })
                                .await;
                        }
                    }
                }
                Ok(false)
            }
            "content_block_stop" => {
                let index = value
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(0);
                if let Some(tool) = active_tools.remove(&index) {
                    let _ = tx
                        .send(ProviderStreamEvent::ToolCallCompleted {
                            call_id: tool.call_id,
                            name: tool.name,
                            arguments: tool.arguments,
                        })
                        .await;
                }
                Ok(false)
            }
            "message_delta" => {
                if value.get("usage").is_some() {
                    let next = Self::usage_from_value(value.get("usage"));
                    // message_delta usage typically only has output_tokens; preserve input.
                    if next.prompt_tokens.is_some() {
                        usage.prompt_tokens = next.prompt_tokens;
                    }
                    if next.completion_tokens.is_some() {
                        usage.completion_tokens = next.completion_tokens;
                    }
                    usage.total_tokens = match (usage.prompt_tokens, usage.completion_tokens) {
                        (Some(a), Some(b)) => Some(a + b),
                        _ => next.total_tokens.or(usage.total_tokens),
                    };
                    let _ = tx
                        .send(ProviderStreamEvent::UsageUpdated {
                            usage: usage.clone(),
                        })
                        .await;
                }
                Ok(false)
            }
            "message_stop" => Ok(true),
            _ => Ok(false),
        }
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
        let mut active_tools: HashMap<usize, ActiveToolCall> = HashMap::new();
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
                        // Ignore event:/id:/retry: — route from payload `type` when present.
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
                                &mut active_tools,
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
                            &mut active_tools,
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
            &[AgentMessage::text(crate::ai::AgentRole::User, "ping")],
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

    async fn chat_stream(
        &self,
        request: AgentRequest,
        tx: ProviderStreamTx,
    ) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
            return Err(AiError::Cancelled);
        }
        let _ = SCHEMA_VERSION;
        let mut body = Self::build_body(&request.system_prompt, &request.messages, &self.model);
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".into(), json!(true));
        }

        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: self.provider_id().to_string(),
                model: self.model.clone(),
                live: true,
            })
            .await;

        let http_req = self
            .auth_headers(self.client.post(self.messages_url()))
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
                "Anthropic HTTP {}: {}",
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
            // Prefer honest failure over fake typing when no live deltas arrived.
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
    fn extracts_anthropic_text_delta() {
        let event = json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": { "type": "text_delta", "text": "Hel" }
        });
        assert_eq!(
            AnthropicProvider::stream_text_delta(&event).as_deref(),
            Some("Hel")
        );
    }

    #[test]
    fn text_delta_ignores_tool_json_delta() {
        let event = json!({
            "type": "content_block_delta",
            "index": 1,
            "delta": { "type": "input_json_delta", "partial_json": "{\"x\":" }
        });
        assert!(AnthropicProvider::stream_text_delta(&event).is_none());
    }

    #[test]
    fn text_delta_ignores_non_delta_events() {
        let event = json!({
            "type": "message_start",
            "message": { "model": "claude-sonnet-4-5" }
        });
        assert!(AnthropicProvider::stream_text_delta(&event).is_none());
        let empty = json!({
            "type": "content_block_delta",
            "delta": { "type": "text_delta", "text": "" }
        });
        assert!(AnthropicProvider::stream_text_delta(&empty).is_none());
    }
}
