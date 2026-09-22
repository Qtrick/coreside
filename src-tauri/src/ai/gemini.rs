//! Google Gemini provider (generateContent + JSON schema).

use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::provider::{
    AgentRequest, AgentResponse, AiProvider, ProviderHealth, ProviderStreamEvent, ProviderStreamTx,
    UsageMetadata,
};
use super::response_schema::SCHEMA_VERSION;
use crate::security::redact_secrets;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_STREAM_EVENTS: usize = 50_000;
const MAX_STREAM_TEXT_BYTES: usize = super::http_limits::MAX_PROVIDER_RESPONSE_BYTES;

pub struct GeminiProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl GeminiProvider {
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

    fn generate_url(&self) -> String {
        format!("{}/models/{}:generateContent", self.base_url, self.model)
    }

    fn stream_generate_url(&self) -> String {
        format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.base_url, self.model
        )
    }

    fn list_models_url(&self) -> String {
        format!("{}/models", self.base_url)
    }

    fn request_client(&self) -> Result<reqwest::Client, AiError> {
        super::platform::validate_and_build_credential_client(
            &self.base_url,
            super::platform::EndpointClass::FixedTrustedRemote,
            false,
            false,
            REQUEST_TIMEOUT,
        )
        .map(|(_, c)| c)
        .map_err(|e| AiError::Validation(e.to_string()))
    }

    fn response_schema() -> Value {
        // Keep this conservative: Gemini rejects many OpenAPI features
        // (`nullable`, complex `anyOf`) which previously caused opaque 400s.
        json!({
            "type": "object",
            "properties": {
                "schemaVersion": { "type": "string" },
                "assistantMessage": { "type": "string" },
                "responseType": {
                    "type": "string",
                    "enum": ["message", "tool_change", "tool_use", "settings_change", "noop"]
                },
                "toolCalls": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "capability": { "type": "string" },
                            "arguments": { "type": "object" }
                        },
                        "required": ["capability"]
                    }
                },
                "citations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "title": { "type": "string" },
                            "url": { "type": "string" },
                            "displayDomain": { "type": "string" },
                            "snippet": { "type": "string" }
                        },
                        "required": ["id", "title", "url"]
                    }
                },
                "toolChange": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["create", "update", "replace"]
                        },
                        "targetToolId": { "type": "string" },
                        "changeSummary": { "type": "string" },
                        "tool": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "name": { "type": "string" },
                                "description": { "type": "string" },
                                "layout": {
                                    "type": "object",
                                    "properties": {
                                        "type": { "type": "string" }
                                    }
                                },
                                "components": {
                                    "type": "array",
                                    "items": { "type": "object" }
                                }
                            },
                            "required": ["id", "name", "components"]
                        }
                    },
                    "required": ["action", "changeSummary"]
                },
                "operations": {
                    "type": "array",
                    "description": "Canonical Runtime V2 operations (preferred for app changes). Each item is an operation object with id, type, target, and payload.",
                    "items": { "type": "object" }
                },
                "assistantMessages": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "turnId": { "type": "string" },
                "silent": { "type": "boolean" },
                "settingsChange": {
                    "type": "object",
                    "properties": {
                        "theme": {
                            "type": "string",
                            "enum": ["system", "light", "dark"]
                        },
                        "accentPrimary": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "accentSecondary": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "background": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "surface": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "surfaceMuted": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "border": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "textPrimary": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "textSecondary": {
                            "type": "object",
                            "properties": {
                                "light": { "type": "string" },
                                "dark": { "type": "string" }
                            }
                        },
                        "wallpaper": {
                            "type": "object",
                            "properties": {
                                "kind": {
                                    "type": "string",
                                    "enum": ["none", "matrix", "aurora", "particles", "rain", "pulse"]
                                },
                                "color": { "type": "string" },
                                "secondaryColor": { "type": "string" },
                                "speed": { "type": "number" },
                                "density": { "type": "number" },
                                "opacity": { "type": "number" }
                            },
                            "required": ["kind"]
                        },
                        "changeSummary": { "type": "string" }
                    }
                },
                "diagnostics": { "type": "object" }
            },
            "required": ["schemaVersion", "assistantMessage", "responseType"]
        })
    }

    fn build_contents(messages: &[super::provider::AgentMessage]) -> Vec<Value> {
        messages
            .iter()
            .map(super::gemini_family::gemini_content_entry)
            .collect()
    }

    async fn send_generate(
        &self,
        body: Value,
        cancel: CancellationToken,
    ) -> Result<Value, AiError> {
        let url = self.generate_url();
        let client = self.request_client()?;
        let request = client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body);

        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = request.send() => {
                result.map_err(|e| {
                    if e.is_timeout() {
                        AiError::Timeout
                    } else if e.is_connect() || e.is_request() {
                        AiError::Http(redact_secrets(&e.to_string(), Some(&self.api_key)))
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
                "Gemini HTTP {}: {}",
                status.as_u16(),
                redact_secrets(&text, Some(&self.api_key))
            )));
        }

        serde_json::from_str(&text).map_err(|e| {
            AiError::Parse(format!(
                "Invalid Gemini JSON: {} — {}",
                e,
                redact_secrets(
                    &text.chars().take(200).collect::<String>(),
                    Some(&self.api_key)
                )
            ))
        })
    }

    fn extract_text(payload: &Value) -> Result<String, AiError> {
        let candidates = payload
            .get("candidates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| AiError::Parse("Missing candidates".into()))?;

        let first = candidates
            .first()
            .ok_or_else(|| AiError::Parse("Empty candidates".into()))?;

        let parts = first
            .pointer("/content/parts")
            .and_then(|p| p.as_array())
            .ok_or_else(|| AiError::Parse("Missing content.parts".into()))?;

        let mut text = String::new();
        for part in parts {
            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                text.push_str(t);
            }
        }

        if text.is_empty() {
            return Err(AiError::Parse("Empty model text".into()));
        }
        Ok(text)
    }

    fn extract_usage(payload: &Value) -> UsageMetadata {
        let meta = payload.get("usageMetadata");
        UsageMetadata {
            prompt_tokens: meta
                .and_then(|m| m.get("promptTokenCount"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            completion_tokens: meta
                .and_then(|m| m.get("candidatesTokenCount"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            total_tokens: meta
                .and_then(|m| m.get("totalTokenCount"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        }
    }

    /// Concatenate `candidates[0].content.parts[].text` from one stream chunk.
    fn stream_chunk_text(chunk: &Value) -> Option<String> {
        let parts = chunk.pointer("/candidates/0/content/parts")?.as_array()?;
        let mut text = String::new();
        for part in parts {
            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                text.push_str(t);
            }
        }
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// Emit only the new suffix when Gemini sends cumulative text; otherwise treat as delta.
    fn stream_text_delta(chunk: &Value, already: &str) -> Option<String> {
        let extracted = Self::stream_chunk_text(chunk)?;
        if extracted.starts_with(already) && extracted.len() >= already.len() {
            let suffix = &extracted[already.len()..];
            if suffix.is_empty() {
                None
            } else {
                Some(suffix.to_string())
            }
        } else {
            Some(extracted)
        }
    }

    fn safety_failure(chunk: &Value) -> Option<String> {
        if let Some(reason) = chunk
            .pointer("/promptFeedback/blockReason")
            .and_then(|v| v.as_str())
        {
            return Some(format!("Gemini blocked prompt: {reason}"));
        }
        if let Some(err) = chunk.get("error") {
            let message = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Gemini stream error");
            return Some(message.to_string());
        }
        let finish = chunk
            .pointer("/candidates/0/finishReason")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match finish {
            "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" => {
                Some(format!("Gemini terminated: {finish}"))
            }
            _ => None,
        }
    }

    fn build_chat_body(system_prompt: &str, messages: &[super::provider::AgentMessage]) -> Value {
        let contents = Self::build_contents(messages);
        json!({
            "systemInstruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": contents,
            "generationConfig": {
                "temperature": 0.4,
                "responseMimeType": "application/json",
                "responseSchema": Self::response_schema()
            }
        })
    }

    /// Process one SSE `data:` payload. Returns `Ok(true)` when a terminal finishReason ends the stream.
    async fn handle_sse_data(
        &self,
        data: &str,
        full_text: &mut String,
        usage: &mut UsageMetadata,
        events: &mut usize,
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
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            return Ok(false);
        };
        if let Some(msg) = Self::safety_failure(&value) {
            return Err(AiError::Provider(redact_secrets(&msg, Some(&self.api_key))));
        }
        if value.get("usageMetadata").is_some() {
            *usage = Self::extract_usage(&value);
            let _ = tx
                .send(ProviderStreamEvent::UsageUpdated {
                    usage: usage.clone(),
                })
                .await;
        }
        if let Some(delta) = Self::stream_text_delta(&value, full_text) {
            if full_text.len().saturating_add(delta.len()) > MAX_STREAM_TEXT_BYTES {
                return Err(AiError::Provider("stream text exceeded byte limit".into()));
            }
            full_text.push_str(&delta);
            let _ = tx
                .send(ProviderStreamEvent::TextDelta { text: delta })
                .await;
        }
        let finish = value
            .pointer("/candidates/0/finishReason")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        // Terminal success finishes end the stream; empty finishReason continues.
        Ok(matches!(finish, "STOP" | "MAX_TOKENS"))
    }

    async fn consume_sse_chat_stream(
        &self,
        response: reqwest::Response,
        cancel: &CancellationToken,
        tx: &ProviderStreamTx,
    ) -> Result<(String, UsageMetadata), AiError> {
        let mut buf: Vec<u8> = Vec::new();
        let mut full_text = String::new();
        let mut usage = UsageMetadata::default();
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
                            .handle_sse_data(data, &mut full_text, &mut usage, &mut events, tx)
                            .await?
                        {
                            return Ok((full_text, usage));
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
                        .handle_sse_data(data, &mut full_text, &mut usage, &mut events, tx)
                        .await?;
                }
            }
        }
        Ok((full_text, usage))
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    fn provider_id(&self) -> &str {
        "gemini"
    }

    fn display_name(&self) -> &str {
        "Google Gemini"
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        // Prefer listing models; fall back to a minimal generate.
        let list_url = self.list_models_url();
        let client = self.request_client()?;
        let list_req = client
            .get(&list_url)
            .header("x-goog-api-key", &self.api_key)
            .timeout(Duration::from_secs(20));

        let list_result = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = list_req.send() => result,
        };

        if let Ok(resp) = list_result {
            if resp.status().is_success() {
                let body: Value = resp.json().await.unwrap_or(json!({}));
                let models = body
                    .get("models")
                    .and_then(|m| m.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                m.get("name")
                                    .and_then(|n| n.as_str())
                                    .map(|s| s.trim_start_matches("models/").to_string())
                            })
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

        // Minimal generate fallback
        let body = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": "ping" }]
            }],
            "generationConfig": {
                "maxOutputTokens": 8
            }
        });

        let _ = self.send_generate(body, cancel).await?;
        Ok(ProviderHealth {
            ok: true,
            message: "generateContent reachable".into(),
            models: vec![self.model.clone()],
        })
    }

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        let contents = Self::build_contents(&request.messages);
        if contents.is_empty() {
            return Err(AiError::Validation("No messages to send".into()));
        }

        let mut body = json!({
            "systemInstruction": {
                "parts": [{ "text": request.system_prompt }]
            },
            "contents": contents,
            "generationConfig": {
                "temperature": 0.4,
                "responseMimeType": "application/json",
                "responseSchema": Self::response_schema()
            }
        });

        let _ = SCHEMA_VERSION;

        let payload = match self
            .send_generate(body.clone(), request.cancel.clone())
            .await
        {
            Ok(payload) => payload,
            Err(err) => {
                let msg = err.to_string().to_lowercase();
                let looks_like_schema = msg.contains("schema")
                    || msg.contains("invalid argument")
                    || msg.contains("400");
                if !looks_like_schema {
                    return Err(err);
                }
                tracing::warn!(
                    error = %redact_secrets(&err.to_string(), Some(&self.api_key)),
                    "Gemini structured schema rejected; retrying with JSON mime only"
                );
                if let Some(gen) = body.get_mut("generationConfig") {
                    if let Some(obj) = gen.as_object_mut() {
                        obj.remove("responseSchema");
                    }
                }
                self.send_generate(body, request.cancel.clone()).await?
            }
        };
        let raw_text = Self::extract_text(&payload)?;
        let usage = Self::extract_usage(&payload);

        Ok(AgentResponse {
            raw_text,
            usage,
            model: self.model.clone(),
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

        let contents = Self::build_contents(&request.messages);
        if contents.is_empty() {
            let err = AiError::Validation("No messages to send".into());
            let _ = tx
                .send(ProviderStreamEvent::ResponseFailed {
                    code: err.code().to_string(),
                    message: err.to_string(),
                })
                .await;
            return Err(err);
        }

        let _ = SCHEMA_VERSION;
        let mut body = Self::build_chat_body(&request.system_prompt, &request.messages);

        let _ = tx
            .send(ProviderStreamEvent::ResponseStarted {
                provider_id: self.provider_id().to_string(),
                model: self.model.clone(),
                live: true,
            })
            .await;

        let url = self.stream_generate_url();
        let client = self.request_client()?;
        let response = {
            let http_req = client
                .post(&url)
                .header("x-goog-api-key", &self.api_key)
                .header("Content-Type", "application/json")
                .json(&body);
            let first = tokio::select! {
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
            if first.status().is_success() {
                first
            } else {
                let status = first.status();
                let text = super::http_limits::read_response_text_bounded(
                    first,
                    &request.cancel,
                    super::http_limits::MAX_PROVIDER_RESPONSE_BYTES,
                    Some(&self.api_key),
                )
                .await
                .unwrap_or_default();
                let msg = text.to_lowercase();
                let looks_like_schema = msg.contains("schema")
                    || msg.contains("invalid argument")
                    || status.as_u16() == 400;
                if !looks_like_schema {
                    let err = AiError::Provider(format!(
                        "Gemini HTTP {}: {}",
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
                tracing::warn!(
                    error = %redact_secrets(&text, Some(&self.api_key)),
                    "Gemini stream schema rejected; retrying with JSON mime only"
                );
                if let Some(gen) = body.get_mut("generationConfig") {
                    if let Some(obj) = gen.as_object_mut() {
                        obj.remove("responseSchema");
                    }
                }
                let retry_req = client
                    .post(&url)
                    .header("x-goog-api-key", &self.api_key)
                    .header("Content-Type", "application/json")
                    .json(&body);
                tokio::select! {
                    _ = request.cancel.cancelled() => {
                        let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                        return Err(AiError::Cancelled);
                    }
                    result = retry_req.send() => {
                        result.map_err(|e| {
                            if e.is_timeout() {
                                AiError::Timeout
                            } else {
                                AiError::Http(redact_secrets(&e.to_string(), Some(&self.api_key)))
                            }
                        })?
                    }
                }
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
                "Gemini HTTP {}: {}",
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

        let (raw_text, usage) = match self
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
            model: self.model.clone(),
            provider_id: self.provider_id().to_string(),
        };
        let _ = tx
            .send(ProviderStreamEvent::TextCompleted { text: raw_text })
            .await;
        let _ = tx
            .send(ProviderStreamEvent::ResponseCompleted {
                response: response.clone(),
                buffered: false,
            })
            .await;
        Ok(response)
    }

    async fn probe(&self, cancel: CancellationToken) -> Result<(), AiError> {
        let body = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": "ping" }]
            }],
            "generationConfig": {
                "maxOutputTokens": 8
            }
        });
        let _ = self.send_generate(body, cancel).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_gemini_stream_chunk_text() {
        let chunk = json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": "Hel" }, { "text": "lo" }]
                }
            }]
        });
        assert_eq!(
            GeminiProvider::stream_chunk_text(&chunk).as_deref(),
            Some("Hello")
        );
    }

    #[test]
    fn stream_text_delta_handles_incremental_parts() {
        let chunk = json!({
            "candidates": [{
                "content": { "parts": [{ "text": " world" }] }
            }]
        });
        assert_eq!(
            GeminiProvider::stream_text_delta(&chunk, "Hello").as_deref(),
            Some(" world")
        );
    }

    #[test]
    fn stream_text_delta_handles_cumulative_text() {
        let chunk = json!({
            "candidates": [{
                "content": { "parts": [{ "text": "Hello world" }] }
            }]
        });
        assert_eq!(
            GeminiProvider::stream_text_delta(&chunk, "Hello").as_deref(),
            Some(" world")
        );
        assert!(GeminiProvider::stream_text_delta(&chunk, "Hello world").is_none());
    }

    #[test]
    fn safety_failure_detects_block_reason() {
        let chunk = json!({
            "promptFeedback": { "blockReason": "SAFETY" }
        });
        assert!(GeminiProvider::safety_failure(&chunk)
            .unwrap()
            .contains("SAFETY"));
        let term = json!({
            "candidates": [{ "finishReason": "SAFETY" }]
        });
        assert!(GeminiProvider::safety_failure(&term)
            .unwrap()
            .contains("SAFETY"));
        let ok = json!({
            "candidates": [{
                "content": { "parts": [{ "text": "ok" }] },
                "finishReason": "STOP"
            }]
        });
        assert!(GeminiProvider::safety_failure(&ok).is_none());
    }
}
