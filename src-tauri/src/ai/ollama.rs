//! Ollama native API adapter (authless Local AI).
//!
//! Official local API: http://localhost:11434
//! - GET /api/tags — list models
//! - POST /api/chat — native chat (NDJSON when `stream: true`)
//!
//! Chat uses native `/api/chat` NDJSON streaming on the send path — not OpenAI-compat `/v1`.

use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use super::http_limits::{read_response_text_bounded, MAX_PROVIDER_RESPONSE_BYTES};
use super::provider::{
    AgentMessage, AgentRequest, AgentResponse, AiProvider, ProviderHealth, ProviderStreamEvent,
    ProviderStreamTx, UsageMetadata,
};
use super::response_schema::SCHEMA_VERSION;
use super::structured_user_input::{
    flatten_parts_for_provider, image_part_from_authorized_bytes, AgentContentPart, AgentRole,
};

const DEFAULT_OLLAMA_ORIGIN: &str = "http://127.0.0.1:11434";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_STREAM_EVENTS: usize = 50_000;
const MAX_STREAM_TEXT_BYTES: usize = MAX_PROVIDER_RESPONSE_BYTES;
const MAX_RAW_STREAM_BYTES: usize = MAX_PROVIDER_RESPONSE_BYTES.saturating_mul(2);
const MAX_LINE_BYTES: usize = 1_048_576;
const MAX_MALFORMED_LINES: usize = 8;

pub fn normalize_ollama_origin(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return DEFAULT_OLLAMA_ORIGIN.to_string();
    }
    if let Some(stripped) = trimmed.strip_suffix("/v1") {
        return stripped.trim_end_matches('/').to_string();
    }
    trimmed.to_string()
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct TagModel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ChatLine {
    #[serde(default)]
    message: Option<ChatMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
    /// Provider thinking/reasoning — captured but never rendered to the user.
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    images: Option<Vec<String>>,
    #[serde(default)]
    tool_calls: Option<Vec<Value>>,
}

/// Incremental NDJSON parser for Ollama `/api/chat` stream lines (testable without network).
#[derive(Debug)]
struct OllamaNdjsonParser {
    buf: Vec<u8>,
    full_text: String,
    model: String,
    events: usize,
    malformed: usize,
    total_raw_bytes: usize,
    produced_output: bool,
    completed: bool,
    saw_done: bool,
    usage: UsageMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OllamaNdjsonParseEvent {
    TextDelta(String),
}

#[derive(Debug, Clone, PartialEq)]
enum OllamaNdjsonFeedOutcome {
    Continue(Vec<OllamaNdjsonParseEvent>),
    Done {
        full_text: String,
        model: String,
        events: Vec<OllamaNdjsonParseEvent>,
        usage: UsageMetadata,
    },
}

impl OllamaNdjsonParser {
    fn new(default_model: String) -> Self {
        Self {
            buf: Vec::new(),
            full_text: String::new(),
            model: default_model,
            events: 0,
            malformed: 0,
            total_raw_bytes: 0,
            produced_output: false,
            completed: false,
            saw_done: false,
            usage: UsageMetadata::default(),
        }
    }

    fn feed_chunk(&mut self, chunk: &[u8]) -> Result<OllamaNdjsonFeedOutcome, AiError> {
        if self.completed {
            if !chunk.is_empty() {
                return Err(AiError::Parse(
                    "Ollama stream contained data after terminal done record".into(),
                ));
            }
            return Ok(OllamaNdjsonFeedOutcome::Continue(Vec::new()));
        }
        self.total_raw_bytes = self.total_raw_bytes.saturating_add(chunk.len());
        if self.total_raw_bytes > MAX_RAW_STREAM_BYTES {
            return Err(AiError::Provider(format!(
                "Ollama stream exceeded aggregate limit of {MAX_RAW_STREAM_BYTES} bytes"
            )));
        }
        if self.buf.len().saturating_add(chunk.len()) > MAX_RAW_STREAM_BYTES {
            return Err(AiError::Provider(format!(
                "Ollama stream buffer exceeded limit of {MAX_RAW_STREAM_BYTES} bytes"
            )));
        }
        self.buf.extend_from_slice(chunk);
        self.drain_complete_lines()
    }

    /// Finish after EOF. Requires exactly one `done:true` terminal record.
    fn finish_stream(mut self) -> Result<(String, String, UsageMetadata), AiError> {
        if !self.buf.is_empty() {
            match self.drain_final_line()? {
                OllamaNdjsonFeedOutcome::Done {
                    full_text,
                    model,
                    usage,
                    ..
                } => return Ok((full_text, model, usage)),
                OllamaNdjsonFeedOutcome::Continue(_) => {}
            }
        }
        if !self.saw_done {
            return Err(AiError::Parse(
                "Ollama stream ended without a done:true terminal record".into(),
            ));
        }
        Ok((self.full_text, self.model, self.usage))
    }

    fn drain_final_line(&mut self) -> Result<OllamaNdjsonFeedOutcome, AiError> {
        if self.buf.is_empty() {
            return Ok(OllamaNdjsonFeedOutcome::Continue(Vec::new()));
        }
        if self.buf.len() > MAX_LINE_BYTES {
            return Err(AiError::Parse("Ollama NDJSON line exceeded size limit".into()));
        }
        let line_bytes = std::mem::take(&mut self.buf);
        let line = std::str::from_utf8(&line_bytes).map_err(|_| {
            AiError::Parse("Ollama stream contained invalid UTF-8".into())
        })?;
        let line = line.trim().trim_end_matches('\r');
        if line.is_empty() {
            return Ok(OllamaNdjsonFeedOutcome::Continue(Vec::new()));
        }
        match self.process_line(line)? {
            None => Ok(OllamaNdjsonFeedOutcome::Continue(Vec::new())),
            Some(OllamaNdjsonLineOutcome::Events(events)) => {
                Ok(OllamaNdjsonFeedOutcome::Continue(events))
            }
            Some(OllamaNdjsonLineOutcome::Done {
                full_text,
                model,
                events,
                usage,
            }) => {
                // Any leftover after a final line is already drained from buf.
                self.completed = true;
                Ok(OllamaNdjsonFeedOutcome::Done {
                    full_text,
                    model,
                    events,
                    usage,
                })
            }
        }
    }

    fn drain_complete_lines(&mut self) -> Result<OllamaNdjsonFeedOutcome, AiError> {
        let mut pending = Vec::new();
        while let Some(idx) = find_line_end(&self.buf) {
            let mut line_bytes = self.buf.drain(..=idx).collect::<Vec<u8>>();
            // Strip LF; also strip preceding CR for CRLF.
            if line_bytes.last() == Some(&b'\n') {
                line_bytes.pop();
            }
            if line_bytes.last() == Some(&b'\r') {
                line_bytes.pop();
            }
            if line_bytes.len() > MAX_LINE_BYTES {
                return Err(AiError::Parse("Ollama NDJSON line exceeded size limit".into()));
            }
            let line = std::str::from_utf8(&line_bytes).map_err(|_| {
                AiError::Parse("Ollama stream contained invalid UTF-8".into())
            })?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match self.process_line(line)? {
                None => {}
                Some(OllamaNdjsonLineOutcome::Events(events)) => pending.extend(events),
                Some(OllamaNdjsonLineOutcome::Done {
                    full_text,
                    model,
                    events,
                    usage,
                }) => {
                    pending.extend(events);
                    // Reject meaningful trailing bytes still in the buffer after done.
                    if self.buf.iter().any(|b| !b.is_ascii_whitespace()) {
                        return Err(AiError::Parse(
                            "Ollama stream contained data after terminal done record".into(),
                        ));
                    }
                    self.buf.clear();
                    self.completed = true;
                    return Ok(OllamaNdjsonFeedOutcome::Done {
                        full_text,
                        model,
                        events: pending,
                        usage,
                    });
                }
            }
        }
        if self.buf.len() > MAX_LINE_BYTES {
            return Err(AiError::Parse("Ollama NDJSON line exceeded size limit".into()));
        }
        Ok(OllamaNdjsonFeedOutcome::Continue(pending))
    }

    fn process_line(&mut self, line: &str) -> Result<Option<OllamaNdjsonLineOutcome>, AiError> {
        let parsed: ChatLine = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => {
                self.malformed = self.malformed.saturating_add(1);
                if self.produced_output {
                    return Err(AiError::Parse(
                        "Malformed Ollama NDJSON record after output started".into(),
                    ));
                }
                if self.malformed > MAX_MALFORMED_LINES {
                    return Err(AiError::Parse(
                        "Too many malformed Ollama NDJSON lines".into(),
                    ));
                }
                return Ok(None);
            }
        };

        if let Some(err) = parsed.error.as_ref().filter(|e| !e.trim().is_empty()) {
            return Err(AiError::Provider(format!("Ollama error: {err}")));
        }

        self.events += 1;
        if self.events > MAX_STREAM_EVENTS {
            return Err(AiError::Provider(format!(
                "Ollama stream exceeded {MAX_STREAM_EVENTS} events"
            )));
        }
        if let Some(m) = parsed.model.as_ref().filter(|s| !s.is_empty()) {
            self.model = m.clone();
        }
        if let Some(n) = parsed.prompt_eval_count {
            self.usage.prompt_tokens = u32::try_from(n).ok();
        }
        if let Some(n) = parsed.eval_count {
            self.usage.completion_tokens = u32::try_from(n).ok();
        }
        // thinking / images / tool_calls on stream: preserve protocol awareness without
        // rendering hidden reasoning. Tool calls in native stream are deferred to a
        // future capability slice; reject unexpected tool payloads fail-closed for now
        // when content is absent and tools are present (avoid silent discard).
        if parsed
            .message
            .as_ref()
            .and_then(|m| m.tool_calls.as_ref())
            .is_some_and(|t| !t.is_empty())
        {
            // ponytail: Ollama tool_calls in NDJSON are not yet mapped to Agent tool calls.
            // Fail closed rather than silently dropping a user-visible tool request.
            return Err(AiError::Provider(
                "Ollama tool_calls in stream are not supported on this Local AI path yet".into(),
            ));
        }

        let mut events = Vec::new();
        if let Some(content) = parsed
            .message
            .as_ref()
            .and_then(|m| m.content.as_ref())
            .filter(|s| !s.is_empty())
        {
            if self.full_text.len().saturating_add(content.len()) > MAX_STREAM_TEXT_BYTES {
                return Err(AiError::Provider(format!(
                    "Ollama stream text exceeded {MAX_STREAM_TEXT_BYTES} bytes"
                )));
            }
            self.full_text.push_str(content);
            self.produced_output = true;
            events.push(OllamaNdjsonParseEvent::TextDelta(content.clone()));
        }
        // Ignore thinking content intentionally (do not render chain-of-thought).
        let _ = parsed
            .message
            .as_ref()
            .and_then(|m| m.thinking.as_ref());
        let _ = parsed.done_reason;

        if parsed.done {
            if self.saw_done {
                return Err(AiError::Parse(
                    "Ollama stream contained duplicate done:true records".into(),
                ));
            }
            self.saw_done = true;
            return Ok(Some(OllamaNdjsonLineOutcome::Done {
                full_text: self.full_text.clone(),
                model: self.model.clone(),
                events,
                usage: self.usage.clone(),
            }));
        }
        if events.is_empty() {
            return Ok(None);
        }
        Ok(Some(OllamaNdjsonLineOutcome::Events(events)))
    }
}

fn find_line_end(buf: &[u8]) -> Option<usize> {
    buf.iter().position(|&b| b == b'\n')
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OllamaNdjsonLineOutcome {
    Events(Vec<OllamaNdjsonParseEvent>),
    Done {
        full_text: String,
        model: String,
        events: Vec<OllamaNdjsonParseEvent>,
        usage: UsageMetadata,
    },
}

pub struct OllamaProvider {
    origin: String,
    model: String,
    client: reqwest::Client,
}

struct OllamaImageCollect {
    base64: Vec<String>,
    skip_notes: Vec<String>,
}

impl OllamaProvider {
    pub fn new(origin: String, model: String) -> Self {
        // Fail closed: never fall back to Client::new() (default follows redirects).
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("reqwest Client");
        Self {
            origin: normalize_ollama_origin(&origin),
            model,
            client,
        }
    }

    fn chat_url(&self) -> String {
        format!("{}/api/chat", self.origin)
    }

    fn build_messages(system_prompt: &str, messages: &[AgentMessage]) -> Vec<Value> {
        let mut out = vec![json!({
            "role": "system",
            "content": system_prompt
        })];
        for m in messages {
            out.push(Self::ollama_message_json(m));
        }
        out
    }

    /// Ollama native message: text `content` + optional `images` (raw base64, not data URLs).
    fn ollama_message_json(message: &AgentMessage) -> Value {
        let role = match message.role {
            AgentRole::Assistant => "assistant",
            AgentRole::System => "system",
            AgentRole::User | AgentRole::ToolResult => "user",
        };
        if message.role == AgentRole::ToolResult {
            return json!({
                "role": role,
                "content": message.tool_result_upstream_content(),
            });
        }

        let text_parts: Vec<AgentContentPart> = message
            .parts
            .iter()
            .filter(|p| {
                !matches!(
                    p,
                    AgentContentPart::Image { .. } | AgentContentPart::ToolResultEnvelope { .. }
                )
            })
            .cloned()
            .collect();
        let mut content = if text_parts.is_empty() {
            message.content.clone()
        } else {
            flatten_parts_for_provider(&text_parts)
        };

        let image_collect = Self::collect_ollama_images(&message.parts);
        for note in &image_collect.skip_notes {
            if content.is_empty() {
                content = note.clone();
            } else {
                content = format!("{content}\n\n{note}");
            }
        }
        let images = image_collect.base64;
        if images.is_empty() {
            json!({
                "role": role,
                "content": content,
            })
        } else {
            json!({
                "role": role,
                "content": content,
                "images": images,
            })
        }
    }

    fn collect_ollama_images(parts: &[AgentContentPart]) -> OllamaImageCollect {
        let mut out = OllamaImageCollect {
            base64: Vec::new(),
            skip_notes: Vec::new(),
        };
        for part in parts {
            if let AgentContentPart::Image {
                attachment_id,
                mime_type,
                data_base64,
            } = part
            {
                if !super::structured_user_input::is_provider_image_mime(mime_type) {
                    let note = format!(
                        "[image not sent to model: skip image {attachment_id}: unsupported mime {mime_type}]"
                    );
                    tracing::info!(
                        target: "coreside::ai::ollama",
                        attachment_id = %attachment_id,
                        mime_type = %mime_type,
                        "skip ollama image: unsupported mime"
                    );
                    out.skip_notes.push(note);
                    continue;
                }
                let Some(b64) = data_base64.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty())
                else {
                    let note = format!(
                        "[image not sent to model: skip image {attachment_id}: no authorized bytes loaded]"
                    );
                    tracing::info!(
                        target: "coreside::ai::ollama",
                        attachment_id = %attachment_id,
                        "skip ollama image: no authorized bytes"
                    );
                    out.skip_notes.push(note);
                    continue;
                };
                if b64.starts_with('/') || (b64.contains("://") && !b64.starts_with("data:")) {
                    let note = format!(
                        "[image not sent to model: skip image {attachment_id}: refusing path-like payload]"
                    );
                    tracing::info!(
                        target: "coreside::ai::ollama",
                        attachment_id = %attachment_id,
                        "skip ollama image: refusing path-like payload"
                    );
                    out.skip_notes.push(note);
                    continue;
                }
                out.base64.push(b64.to_string());
            }
        }
        out
    }

    fn chat_body(&self, system_prompt: &str, messages: &[AgentMessage], stream: bool) -> Value {
        let _ = SCHEMA_VERSION;
        json!({
            "model": self.model,
            "messages": Self::build_messages(system_prompt, messages),
            "stream": stream,
            "format": "json",
            "options": {
                "temperature": 0.4
            }
        })
    }

    /// Bounded GET /api/tags — no Authorization header.
    pub async fn list_ollama_models(
        base_url: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<String>, AiError> {
        let origin = normalize_ollama_origin(base_url);
        let url = format!("{origin}/api/tags");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AiError::Http(e.to_string()))?;
        let resp = tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            result = client.get(&url).send() => result.map_err(|e| AiError::Http(e.to_string()))?,
        };
        if !resp.status().is_success() {
            return Err(AiError::Provider(format!(
                "Ollama /api/tags returned {}",
                resp.status()
            )));
        }
        let text = read_response_text_bounded(resp, cancel, MAX_PROVIDER_RESPONSE_BYTES, None)
            .await?;
        let parsed: TagsResponse =
            serde_json::from_str(&text).map_err(|e| AiError::Parse(e.to_string()))?;
        let mut names: Vec<String> = parsed.models.into_iter().map(|m| m.name).collect();
        names.sort();
        names.dedup();
        if names.len() > 512 {
            names.truncate(512);
        }
        Ok(names)
    }

    /// Native health: successful tags list (server up). Empty model list is still healthy.
    pub async fn ollama_server_ready(
        base_url: &str,
        cancel: &CancellationToken,
    ) -> Result<bool, AiError> {
        Self::list_ollama_models(base_url, cancel).await.map(|_| true)
    }

    async fn post_chat_non_stream(
        &self,
        request: &AgentRequest,
    ) -> Result<AgentResponse, AiError> {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }
        let body = self.chat_body(&request.system_prompt, &request.messages, false);
        let response = tokio::select! {
            _ = request.cancel.cancelled() => return Err(AiError::Cancelled),
            result = self.client.post(self.chat_url()).json(&body).send() => {
                result.map_err(|e| {
                    if e.is_timeout() {
                        AiError::Timeout
                    } else {
                        AiError::Http(e.to_string())
                    }
                })?
            }
        };
        if !response.status().is_success() {
            return Err(AiError::Provider(format!(
                "Ollama /api/chat returned {}",
                response.status()
            )));
        }
        let text = read_response_text_bounded(
            response,
            &request.cancel,
            MAX_PROVIDER_RESPONSE_BYTES,
            None,
        )
        .await?;
        let line: ChatLine =
            serde_json::from_str(&text).map_err(|e| AiError::Parse(e.to_string()))?;
        let raw_text = line
            .message
            .and_then(|m| m.content)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| AiError::Parse("Empty Ollama response".into()))?;
        let model = line.model.unwrap_or_else(|| self.model.clone());
        Ok(AgentResponse {
            raw_text,
            usage: UsageMetadata::default(),
            model,
            provider_id: "ollama".to_string(),
        })
    }

    async fn emit_ndjson_events(
        &self,
        events: &[OllamaNdjsonParseEvent],
        tx: &ProviderStreamTx,
    ) {
        for event in events {
            let OllamaNdjsonParseEvent::TextDelta(text) = event;
            let _ = tx
                .send(ProviderStreamEvent::TextDelta {
                    text: text.clone(),
                })
                .await;
        }
    }

    async fn consume_ndjson_chat_stream(
        &self,
        response: reqwest::Response,
        cancel: &CancellationToken,
        tx: &ProviderStreamTx,
    ) -> Result<(String, String, UsageMetadata), AiError> {
        let mut parser = OllamaNdjsonParser::new(self.model.clone());
        let mut stream = response.bytes_stream();
        let mut idle = tokio::time::interval(IDLE_TIMEOUT);
        idle.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        idle.tick().await; // skip immediate first tick

        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => {
                    drop(stream);
                    let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                    return Err(AiError::Cancelled);
                }
                _ = idle.tick() => {
                    drop(stream);
                    return Err(AiError::Timeout);
                }
                item = stream.next() => item,
            };
            match next {
                None => break,
                Some(Err(e)) => {
                    drop(stream);
                    return Err(AiError::Http(e.to_string()));
                }
                Some(Ok(chunk)) => {
                    idle.reset();
                    match parser.feed_chunk(&chunk)? {
                        OllamaNdjsonFeedOutcome::Continue(events) => {
                            self.emit_ndjson_events(&events, tx).await;
                        }
                        OllamaNdjsonFeedOutcome::Done {
                            full_text,
                            model,
                            events,
                            usage,
                        } => {
                            self.emit_ndjson_events(&events, tx).await;
                            // Drop the body so the connection is released after terminal success.
                            drop(stream);
                            return Ok((full_text, model, usage));
                        }
                    }
                }
            }
        }

        drop(stream);
        parser.finish_stream()
    }
}

/// Module-level entry for credential health checks and status probes.
pub async fn ollama_server_ready(
    base_url: &str,
    cancel: &CancellationToken,
) -> Result<bool, AiError> {
    OllamaProvider::ollama_server_ready(base_url, cancel).await
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn provider_id(&self) -> &str {
        "ollama"
    }

    fn display_name(&self) -> &str {
        "Ollama"
    }

    async fn health_check(&self, cancel: CancellationToken) -> Result<ProviderHealth, AiError> {
        let models = OllamaProvider::list_ollama_models(&self.origin, &cancel).await?;
        Ok(ProviderHealth {
            ok: true,
            message: format!("Ollama reachable ({} models)", models.len()),
            models,
        })
    }

    async fn chat(&self, request: AgentRequest) -> Result<AgentResponse, AiError> {
        self.post_chat_non_stream(&request).await
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
                model: self.model.clone(),
                live: true,
            })
            .await;

        let body = self.chat_body(&request.system_prompt, &request.messages, true);
        let response = tokio::select! {
            _ = request.cancel.cancelled() => {
                let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                return Err(AiError::Cancelled);
            }
            result = self.client.post(self.chat_url()).json(&body).send() => {
                result.map_err(|e| {
                    if e.is_timeout() {
                        AiError::Timeout
                    } else {
                        AiError::Http(e.to_string())
                    }
                })?
            }
        };

        if !response.status().is_success() {
            let err = AiError::Provider(format!(
                "Ollama /api/chat returned {}",
                response.status()
            ));
            let _ = tx
                .send(ProviderStreamEvent::ResponseFailed {
                    code: err.code().to_string(),
                    message: err.to_string(),
                })
                .await;
            return Err(err);
        }

        let (full_text, model, usage) = match self
            .consume_ndjson_chat_stream(response, &request.cancel, &tx)
            .await
        {
            Ok(v) => v,
            Err(err) => {
                if matches!(err, AiError::Cancelled) {
                    return Err(err);
                }
                let _ = tx
                    .send(ProviderStreamEvent::ResponseFailed {
                        code: err.code().to_string(),
                        message: err.to_string(),
                    })
                    .await;
                return Err(err);
            }
        };

        let response = AgentResponse {
            raw_text: full_text.clone(),
            usage,
            model,
            provider_id: self.provider_id().to_string(),
        };
        let _ = tx
            .send(ProviderStreamEvent::TextCompleted {
                text: full_text,
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
    fn strips_v1_suffix() {
        assert_eq!(
            normalize_ollama_origin("http://127.0.0.1:11434/v1"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            normalize_ollama_origin("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(normalize_ollama_origin(""), DEFAULT_OLLAMA_ORIGIN);
    }

    #[test]
    fn chat_line_deserializes_partial() {
        let line = r#"{"message":{"role":"assistant","content":"{\"assistant"},"done":false}"#;
        let parsed: ChatLine = serde_json::from_str(line).expect("parse");
        assert!(!parsed.done);
        assert_eq!(
            parsed.message.as_ref().and_then(|m| m.content.as_deref()),
            Some("{\"assistant")
        );
    }

    #[test]
    fn malformed_line_policy_allows_skip() {
        let garbage = "not json";
        assert!(serde_json::from_str::<ChatLine>(garbage).is_err());
        let good = r#"{"message":{"content":"ok"},"done":true}"#;
        assert!(serde_json::from_str::<ChatLine>(good).is_ok());
    }

    fn line(content: &str, done: bool) -> String {
        format!(
            r#"{{"message":{{"content":"{content}"}},"done":{done}}}"#
        )
    }

    #[test]
    fn ndjson_parser_accumulates_text_across_chunk_boundaries() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        let chunk1 = format!("{}\n", line("hel", false));
        let chunk2 = format!("{}\n", line("lo", true));

        let out1 = parser.feed_chunk(chunk1.as_bytes()).expect("chunk1");
        assert_eq!(
            out1,
            OllamaNdjsonFeedOutcome::Continue(vec![OllamaNdjsonParseEvent::TextDelta(
                "hel".into()
            )])
        );

        let out2 = parser.feed_chunk(chunk2.as_bytes()).expect("chunk2");
        match out2 {
            OllamaNdjsonFeedOutcome::Done {
                full_text, model, events, ..
            } => {
                assert_eq!(full_text, "hello");
                assert_eq!(model, "llama3.2");
                assert_eq!(
                    events,
                    vec![OllamaNdjsonParseEvent::TextDelta("lo".into())]
                );
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn ndjson_parser_skips_malformed_preamble_until_done() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        let payload = format!(
            "not-json\n\n{}\n",
            line("recovered", true)
        );
        let out = parser.feed_chunk(payload.as_bytes()).expect("feed");
        match out {
            OllamaNdjsonFeedOutcome::Done { full_text, .. } => {
                assert_eq!(full_text, "recovered");
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn ndjson_parser_rejects_malformed_after_output_started() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        let payload = format!("{}\nbad-json\n", line("hi", false));
        let err = parser.feed_chunk(payload.as_bytes()).expect_err("after output");
        assert!(matches!(err, AiError::Parse(_)));
        assert!(err.to_string().contains("after output"));
    }

    #[test]
    fn ndjson_parser_rejects_alternating_malformed_after_start() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        parser
            .feed_chunk(format!("{}\n", line("a", false)).as_bytes())
            .unwrap();
        let err = parser
            .feed_chunk(b"not-json\n")
            .expect_err("alternating");
        assert!(err.to_string().contains("after output"));
    }

    #[test]
    fn ndjson_parser_rejects_too_many_malformed_lines() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        let garbage = "not-json\n".repeat(MAX_MALFORMED_LINES + 1);
        let err = parser
            .feed_chunk(garbage.as_bytes())
            .expect_err("malformed limit");
        assert!(matches!(err, AiError::Parse(_)));
        assert!(err.to_string().contains("malformed"));
    }

    #[test]
    fn ndjson_parser_processes_trailing_line_without_newline() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        let partial = line("tail", true);
        let out = parser
            .feed_chunk(partial.as_bytes())
            .expect("partial without newline");
        assert_eq!(out, OllamaNdjsonFeedOutcome::Continue(Vec::new()));

        let (text, model, _) = parser.finish_stream().expect("finish");
        assert_eq!(text, "tail");
        assert_eq!(model, "llama3.2");
    }

    #[test]
    fn ndjson_parser_eof_without_done_is_error_even_with_text() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        parser
            .feed_chunk(format!("{}\n", line("partial", false)).as_bytes())
            .unwrap();
        let err = parser.finish_stream().expect_err("missing done");
        assert!(err.to_string().contains("done:true"));
    }

    #[test]
    fn ndjson_parser_empty_stream_is_parse_error() {
        let parser = OllamaNdjsonParser::new("llama3.2".into());
        let err = parser.finish_stream().expect_err("empty stream");
        assert!(matches!(err, AiError::Parse(_)));
    }

    #[test]
    fn ndjson_parser_rejects_invalid_utf8() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        let err = parser.feed_chunk(b"\xff\n").expect_err("utf8");
        assert!(err.to_string().contains("UTF-8"));
    }

    #[test]
    fn ndjson_parser_supports_crlf() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        let payload = format!("{}\r\n", line("ok", true));
        match parser.feed_chunk(payload.as_bytes()).unwrap() {
            OllamaNdjsonFeedOutcome::Done { full_text, .. } => assert_eq!(full_text, "ok"),
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn ndjson_parser_rejects_trailing_after_done() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        let payload = format!("{}\n{}\n", line("a", true), line("b", false));
        let err = parser.feed_chunk(payload.as_bytes()).expect_err("trailing");
        assert!(err.to_string().contains("after terminal"));
    }

    #[test]
    fn ndjson_parser_handles_native_error_object() {
        let mut parser = OllamaNdjsonParser::new("llama3.2".into());
        let err = parser
            .feed_chunk(b"{\"error\":\"model not found\"}\n")
            .expect_err("error line");
        assert!(err.to_string().contains("Ollama error"));
    }

    #[test]
    fn ollama_message_includes_images_array_not_placeholder_text() {
        let png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE,
            0xD4, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let image = image_part_from_authorized_bytes(
            "att-1",
            "image/png",
            png,
        )
        .unwrap();
        let msg = AgentMessage::with_parts(
            AgentRole::User,
            "see image",
            vec![
                AgentContentPart::Text {
                    text: "see image".into(),
                },
                image,
            ],
        );
        let mapped = OllamaProvider::ollama_message_json(&msg);
        assert!(mapped.get("images").and_then(|v| v.as_array()).is_some());
        let content = mapped["content"].as_str().unwrap_or("");
        assert!(!content.contains("[image attachmentId="));
    }

    #[test]
    fn ollama_tool_result_uses_json_envelope_not_markdown_wrapper() {
        use crate::ai::capability_registry::{
            seal_tool_result_envelope, ToolCallResult,
        };
        use serde_json::json;
        let results = vec![ToolCallResult {
            capability: "web_search".into(),
            ok: true,
            output: json!({ "hits": 1 }),
            error: None,
            pending_approval: None,
        }];
        let msg = AgentMessage::with_parts(
            AgentRole::ToolResult,
            "Tool results (1)",
            vec![seal_tool_result_envelope(&results)],
        );
        let mapped = OllamaProvider::ollama_message_json(&msg);
        let content = mapped["content"].as_str().expect("content");
        assert!(content.contains("untrusted_tool_output"));
        assert!(!content.contains("[UNTRUSTED_TOOL_RESULT"));
    }
}
