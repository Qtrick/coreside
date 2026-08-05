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
use super::structured_user_input::{
    flatten_parts_for_provider, is_provider_image_mime, AgentContentPart,
};
use crate::security::redact_secrets;
use futures_util::StreamExt;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_STREAM_EVENTS: usize = 50_000;
const MAX_STREAM_TEXT_BYTES: usize = super::http_limits::MAX_PROVIDER_RESPONSE_BYTES;

/// Map a trusted Image part to an OpenAI `image_url` content part using a data URL
/// built from already-authorized bytes. Never accepts filesystem paths.
///
/// Returns `None` (with an honest skip reason) when bytes are missing, mime is
/// unsupported, or the payload is too heavy for the provider budget.
pub fn map_image_part_to_openai_content(part: &AgentContentPart) -> Result<Value, String> {
    let AgentContentPart::Image {
        attachment_id,
        mime_type,
        data_base64,
    } = part
    else {
        return Err("not an Image part".into());
    };
    if attachment_id.trim().is_empty() {
        return Err("image part missing attachment_id".into());
    }
    if !is_provider_image_mime(mime_type) {
        return Err(format!(
            "skip image {attachment_id}: unsupported mime {mime_type}"
        ));
    }
    let Some(b64) = data_base64.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Err(format!(
            "skip image {attachment_id}: no authorized bytes loaded (paths are never sent)"
        ));
    };
    // Reject anything that looks like a filesystem path slipped into the field.
    if b64.starts_with('/') || (b64.contains("://") && !b64.starts_with("data:")) {
        return Err(format!(
            "skip image {attachment_id}: refusing non-base64 / path-like payload"
        ));
    }
    // Approx decoded size from base64 length (4 chars → 3 bytes).
    let approx_bytes = (b64.len() / 4).saturating_mul(3);
    if approx_bytes > super::structured_user_input::MAX_PROVIDER_IMAGE_BYTES {
        return Err(format!(
            "skip image {attachment_id}: exceeds provider byte budget (~{approx_bytes} bytes)"
        ));
    }
    let mime = mime_type.trim().to_ascii_lowercase();
    Ok(json!({
        "type": "image_url",
        "image_url": {
            "url": format!("data:{mime};base64,{b64}")
        }
    }))
}

/// Build OpenAI message `content`: string when text-only; array when Image parts present.
pub fn openai_message_content(message: &AgentMessage) -> Value {
    let parts = if message.parts.is_empty() {
        return json!(message.content);
    } else {
        &message.parts
    };

    let has_image = parts
        .iter()
        .any(|p| matches!(p, AgentContentPart::Image { .. }));
    if !has_image {
        return json!(flatten_parts_for_provider(parts));
    }

    let mut content_parts: Vec<Value> = Vec::new();
    let text = flatten_parts_for_provider(
        &parts
            .iter()
            .filter(|p| !matches!(p, AgentContentPart::Image { .. }))
            .cloned()
            .collect::<Vec<_>>(),
    );
    if !text.trim().is_empty() {
        content_parts.push(json!({
            "type": "text",
            "text": text
        }));
    }
    for part in parts {
        if matches!(part, AgentContentPart::Image { .. }) {
            match map_image_part_to_openai_content(part) {
                Ok(v) => content_parts.push(v),
                Err(reason) => {
                    tracing::info!(target: "coreside::ai::openai", "{reason}");
                    content_parts.push(json!({
                        "type": "text",
                        "text": format!("[image not sent to model: {reason}]"),
                    }));
                }
            }
        }
    }
    if content_parts.is_empty() {
        return json!(message.provider_text());
    }
    // If every image was skipped and we only have text, keep a plain string.
    if content_parts.len() == 1
        && content_parts[0].get("type").and_then(|t| t.as_str()) == Some("text")
    {
        return content_parts[0]
            .get("text")
            .cloned()
            .unwrap_or_else(|| json!(message.provider_text()));
    }
    Value::Array(content_parts)
}

pub struct OpenAiProvider {
    api_key: String,
    model: String,
    base_url: String,
    provider_id: String,
    display_name: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self::with_identity(api_key, model, base_url, "openai", "OpenAI")
    }

    pub fn new_compatible(api_key: String, model: String, base_url: String) -> Self {
        Self::with_identity(
            api_key,
            model,
            base_url,
            "compatible",
            "OpenAI-compatible",
        )
    }

    pub fn with_identity(
        api_key: String,
        model: String,
        base_url: String,
        provider_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        let provider_id = provider_id.into();
        let base_url = base_url.trim_end_matches('/').to_string();
        let class = if provider_id == "compatible" {
            super::platform::EndpointClass::UserConfiguredRemoteCompatible
        } else {
            super::platform::EndpointClass::FixedTrustedRemote
        };
        let is_override = provider_id == "compatible";
        // Send-authoritative pin at client construction; rebuilt again at request time.
        let client = super::platform::validate_and_build_credential_client(
            &base_url,
            class,
            is_override,
            is_override,
            REQUEST_TIMEOUT,
        )
        .map(|(_, c)| c)
        .unwrap_or_else(|_| {
            // Construction must not panic; send path revalidates and fail-closes.
            reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("reqwest Client")
        });
        Self {
            api_key,
            model,
            base_url,
            provider_id,
            display_name: display_name.into(),
            client,
        }
    }

    fn request_client(&self) -> Result<reqwest::Client, AiError> {
        let class = if self.provider_id == "compatible" {
            super::platform::EndpointClass::UserConfiguredRemoteCompatible
        } else {
            super::platform::EndpointClass::FixedTrustedRemote
        };
        let is_override = self.provider_id == "compatible";
        super::platform::validate_and_build_credential_client(
            &self.base_url,
            class,
            is_override,
            is_override,
            REQUEST_TIMEOUT,
        )
        .map(|(_, c)| c)
        .map_err(|e| AiError::Validation(e.to_string()))
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
        super::openai_family::build_openai_chat_messages(system_prompt, messages)
    }

    async fn post_chat(&self, body: Value, cancel: CancellationToken) -> Result<Value, AiError> {
        let client = self.request_client()?;
        let request = self
            .auth_headers(client.post(self.chat_url()))
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
        self.provider_id.as_str()
    }

    fn display_name(&self) -> &str {
        self.display_name.as_str()
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        let client = self.request_client()?;
        let list_req = self
            .auth_headers(client.get(self.models_url()))
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

        let client = self.request_client()?;
        let http_req = self
            .auth_headers(client.post(self.chat_url()))
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
    use crate::ai::structured_user_input::{image_part_from_authorized_bytes, AgentRole};

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

    #[test]
    fn maps_image_part_to_openai_data_url_not_filesystem_path() {
        let png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE,
            0xD4, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let part = image_part_from_authorized_bytes("att-img-1", "image/png", png).unwrap();
        let mapped = map_image_part_to_openai_content(&part).expect("mapped");
        assert_eq!(mapped["type"], "image_url");
        let url = mapped["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        assert!(!url.contains("/Users/"));
        assert!(!url.contains("file://"));
        assert!(!url.contains("coreside-asset"));

        let missing = AgentContentPart::Image {
            attachment_id: "att-2".into(),
            mime_type: "image/png".into(),
            data_base64: None,
        };
        let err = map_image_part_to_openai_content(&missing).unwrap_err();
        assert!(err.contains("no authorized bytes"));

        let msg = AgentMessage::with_parts(
            AgentRole::User,
            "see image",
            vec![
                AgentContentPart::Text {
                    text: "see image".into(),
                },
                part,
            ],
        );
        let content = openai_message_content(&msg);
        assert!(content.is_array());
        let arr = content.as_array().unwrap();
        assert!(arr.iter().any(|p| p["type"] == "text"));
        assert!(arr.iter().any(|p| p["type"] == "image_url"));
    }
}
