//! Auto model selection — try candidates and use the first that works.
//!
//! Important: avoid probe+chat double requests (burns Gemini/OpenRouter rate limits).
//! On 429, back off once before switching models.
//!
//! Production path consumes `AiProvider::chat_stream`. Live adapters (OpenAI) emit
//! progressive `TextDelta` events. Buffered adapters emit `ResponseStarted { live: false }`
//! with no fabricated deltas.
//!
//! Auto fallback policy (no answer splicing):
//! - Before the first meaningful user-visible delta: silent fallback to the next model.
//! - After a meaningful delta: do not switch models; surface the failure as interrupted.

use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use super::errors::AiError;
use super::provider::{
    AgentRequest, AgentResponse, AiProvider, ProviderStreamEvent, ProviderStreamTx,
};
use crate::credentials::ResolvedAiAccess;

/// Ordered Auto candidates for a provider (best first).
/// Diversify vendors so one provider's rate limit doesn't stall Auto.
pub fn auto_model_candidates(provider: &str, configured_default: &str) -> Vec<String> {
    let mut out = Vec::new();
    let push = |list: &mut Vec<String>, id: &str| {
        let id = id.trim();
        if id.is_empty() || id.eq_ignore_ascii_case("auto") {
            return;
        }
        if !list.iter().any(|x| x == id) {
            list.push(id.to_string());
        }
    };

    push(&mut out, configured_default);

    match provider {
        "openrouter" => {
            for id in [
                "openai/gpt-4o-mini",
                "google/gemini-3.8-flash",
                "google/gemini-2.5-flash",
                "meta-llama/llama-3.3-70b-instruct",
                "anthropic/claude-sonnet-4.5",
                "openrouter/auto",
            ] {
                push(&mut out, id);
            }
        }
        "gemini" => {
            for id in [
                "gemini-3.8-flash",
                "gemini-3.1-flash-lite",
                "gemini-2.5-flash",
                "gemini-2.5-pro",
            ] {
                push(&mut out, id);
            }
        }
        "openai" | "compatible" => {
            for id in ["gpt-4.1-mini", "gpt-4.1", "gpt-4o-mini"] {
                push(&mut out, id);
            }
        }
        "anthropic" | "claude" => {
            for id in ["claude-sonnet-4-5", "claude-haiku-4-5"] {
                push(&mut out, id);
            }
        }
        _ => {}
    }

    out
}

pub fn is_auto_preference(model: &str) -> bool {
    let t = model.trim();
    t.is_empty() || t.eq_ignore_ascii_case("auto")
}

fn is_rate_limited(err: &AiError) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("429")
        || msg.contains("rate limit")
        || msg.contains("rate-limit")
        || msg.contains("ratelimit")
        || msg.contains("quota")
        || msg.contains("resource_exhausted")
        || msg.contains("resource exhausted")
        || msg.contains("too many requests")
}

fn is_retryable_model_error(err: &AiError) -> bool {
    let msg = err.to_string().to_lowercase();
    is_rate_limited(err)
        || matches!(
            err,
            AiError::Provider(_) | AiError::Http(_) | AiError::Timeout | AiError::Parse(_)
        )
        || msg.contains("404")
        || msg.contains("model not found")
        || msg.contains("model_not_found")
        || msg.contains("invalid model")
        || msg.contains("no endpoints")
        || msg.contains("unavailable")
        || msg.contains("overloaded")
        || msg.contains("capacity")
}

pub struct ResolvedChat {
    pub response: AgentResponse,
    pub model_used: String,
    pub auto_mode: bool,
    pub attempts: usize,
    /// True when live provider TextDelta events were forwarded to the UI path.
    pub streamed_live: bool,
}

/// Best-effort peek at `"assistantMessage"` while JSON is still incomplete.
/// Returns progressive visible text for the chat bubble without fabricating content.
pub fn peek_assistant_message(partial: &str) -> Option<String> {
    const KEY: &str = "\"assistantMessage\"";

    // Scan for top-level `"assistantMessage"` at depth 1 in the JSON structure,
    // ensuring it is neither inside a preceding string literal nor a nested object.
    let mut in_str = false;
    let mut esc = false;
    let mut depth: usize = 0;
    let mut target_idx = None;

    let bytes = partial.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if esc {
            esc = false;
            i += 1;
            continue;
        }
        let b = bytes[i];
        if in_str {
            if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }

        match b {
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b'"' => {
                if depth == 1 && partial[i..].starts_with(KEY) {
                    let after = &partial[i + KEY.len()..];
                    let trimmed = after.trim_start();
                    if trimmed.starts_with(':') {
                        target_idx = Some(i);
                        break;
                    }
                }
                in_str = true;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let idx = target_idx?;
    let after = &partial[idx + KEY.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let rest = rest.strip_prefix('"')?;

    let quote_pos = rest.find('"');
    let backslash_pos = rest.find('\\');

    // Fast-path: when there are no backslashes before the closing quote (or if unclosed and
    // no backslashes in rest), the slice contains zero escape sequences.
    // We avoid character-by-character loops and allocate exactly slice.len() bytes.
    let unescaped_slice = match (quote_pos, backslash_pos) {
        (Some(q), None) => Some(&rest[..q]),
        (Some(q), Some(b)) if q < b => Some(&rest[..q]),
        (None, None) => Some(rest),
        _ => None,
    };

    if let Some(slice) = unescaped_slice {
        if slice.is_empty() {
            return None;
        }
        return Some(slice.to_owned());
    }

    // Escaped slow path: find where the assistant message string terminates by scanning
    // for an unescaped quote. Bytes '"' and '\\' are ASCII and never part of UTF-8 multi-byte codes.
    let bytes = rest.as_bytes();
    let mut i = 0;
    let mut end = rest.len();
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
        } else if bytes[i] == b'"' {
            end = i;
            break;
        } else {
            i += 1;
        }
    }

    let slice = &rest[..end.min(rest.len())];
    let slice_bytes = slice.as_bytes();
    let mut out = String::with_capacity(slice.len());
    let mut i = 0;

    while i < slice_bytes.len() {
        if slice_bytes[i] == b'\\' {
            let Some(&next) = slice_bytes.get(i + 1) else {
                // Incomplete escape at stream chunk boundary (trailing backslash).
                // Break cleanly without corrupting previous valid decoded characters.
                break;
            };
            match next {
                b'n' => {
                    out.push('\n');
                    i += 2;
                }
                b'r' => {
                    out.push('\r');
                    i += 2;
                }
                b't' => {
                    out.push('\t');
                    i += 2;
                }
                b'b' => {
                    out.push('\x08');
                    i += 2;
                }
                b'f' => {
                    out.push('\x0C');
                    i += 2;
                }
                b'"' => {
                    out.push('"');
                    i += 2;
                }
                b'\\' => {
                    out.push('\\');
                    i += 2;
                }
                b'/' => {
                    out.push('/');
                    i += 2;
                }
                b'u' => {
                    if i + 6 > slice_bytes.len() {
                        // Incomplete \uXXXX at stream boundary. Stop here; next chunk completes it.
                        break;
                    }
                    let Ok(hex_str) = std::str::from_utf8(&slice_bytes[i + 2..i + 6]) else {
                        break;
                    };
                    let Ok(code_unit) = u16::from_str_radix(hex_str, 16) else {
                        break;
                    };

                    if (0xD800..=0xDBFF).contains(&code_unit) {
                        // High surrogate: check for following low surrogate (\uDC00..\uDFFF).
                        if i + 12 > slice_bytes.len() {
                            // Incomplete surrogate pair at chunk boundary. Await full sequence.
                            break;
                        }
                        if slice_bytes[i + 6] != b'\\' || slice_bytes[i + 7] != b'u' {
                            break;
                        }
                        let Ok(low_hex_str) = std::str::from_utf8(&slice_bytes[i + 8..i + 12])
                        else {
                            break;
                        };
                        let Ok(low_code_unit) = u16::from_str_radix(low_hex_str, 16) else {
                            break;
                        };
                        if !(0xDC00..=0xDFFF).contains(&low_code_unit) {
                            break;
                        }
                        let code_point = 0x10000
                            + (((code_unit as u32) - 0xD800) << 10)
                            + ((low_code_unit as u32) - 0xDC00);
                        if let Some(ch) = char::from_u32(code_point) {
                            out.push(ch);
                        }
                        i += 12;
                    } else if (0xDC00..=0xDFFF).contains(&code_unit) {
                        // Unpaired low surrogate: invalid.
                        break;
                    } else {
                        // Standard BMP code point.
                        if let Some(ch) = char::from_u32(code_unit as u32) {
                            out.push(ch);
                        }
                        i += 6;
                    }
                }
                other => {
                    if other < 128 {
                        out.push(other as char);
                        i += 2;
                    } else {
                        let ch = slice[i + 1..].chars().next().unwrap();
                        out.push(ch);
                        i += 1 + ch.len_utf8();
                    }
                }
            }
        } else if slice_bytes[i] < 128 {
            out.push(slice_bytes[i] as char);
            i += 1;
        } else {
            let ch = slice[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }

    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// Run chat_stream with optional Auto fallback across candidate models.
pub async fn chat_with_auto(
    access: &ResolvedAiAccess,
    model_preference: &str,
    request: AgentRequest,
    mut on_action: impl FnMut(&str),
    mut on_stream: impl FnMut(ProviderStreamEvent),
) -> Result<ResolvedChat, AiError> {
    let config = access.credentials.to_app_config();
    let auto_mode = is_auto_preference(model_preference);
    let candidates = if auto_mode {
        auto_model_candidates(&config.provider, &config.model)
    } else {
        vec![model_preference.trim().to_string()]
    };

    if candidates.is_empty() {
        return Err(AiError::Validation("No models available for Auto".into()));
    }

    let mut attempts = 0usize;
    let mut last_err: Option<AiError> = None;
    let base_idempotency_key = request
        .idempotency_key
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    if !auto_mode {
        on_action("Connecting to the model");
    }

    for model in candidates.iter() {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        attempts += 1;
        let provider = build_provider(access, model)?;

        let attempt = AgentRequest {
            system_prompt: request.system_prompt.clone(),
            messages: request.messages.clone(),
            cancel: request.cancel.clone(),
            idempotency_key: Some(if auto_mode {
                format!("{base_idempotency_key}:{model}")
            } else {
                base_idempotency_key.clone()
            }),
            max_output_tokens: request.max_output_tokens,
        };

        on_action("Generating a response");
        match chat_stream_with_rate_limit_retry(provider.as_ref(), &attempt, &mut on_stream).await {
            Ok((response, streamed_live)) => {
                on_action("Reading the response");
                return Ok(ResolvedChat {
                    model_used: response.model.clone(),
                    response,
                    auto_mode,
                    attempts,
                    streamed_live,
                });
            }
            Err(StreamAttemptError::Failed {
                err,
                meaningful_delta,
            }) => {
                if meaningful_delta {
                    // Never splice Model B into Model A's visible answer.
                    return Err(err);
                }
                if auto_mode && is_retryable_model_error(&err) {
                    on_action("Trying another model");
                    last_err = Some(err);
                    continue;
                }
                return Err(err);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| AiError::Provider("Auto could not find a working model".into())))
}

#[derive(Debug)]
enum StreamAttemptError {
    Failed {
        err: AiError,
        meaningful_delta: bool,
    },
}

async fn chat_stream_with_rate_limit_retry(
    provider: &dyn AiProvider,
    request: &AgentRequest,
    on_stream: &mut impl FnMut(ProviderStreamEvent),
) -> Result<(AgentResponse, bool), StreamAttemptError> {
    let idempotency_key = request
        .idempotency_key
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    match run_chat_stream(provider, request, &idempotency_key, on_stream).await {
        Ok(ok) => Ok(ok),
        Err(StreamAttemptError::Failed {
            err,
            meaningful_delta,
        }) if is_rate_limited(&err) && !meaningful_delta => {
            tokio::select! {
                _ = request.cancel.cancelled() => {
                    return Err(StreamAttemptError::Failed {
                        err: AiError::Cancelled,
                        meaningful_delta: false,
                    });
                }
                _ = tokio::time::sleep(Duration::from_millis(1200)) => {}
            }
            run_chat_stream(provider, request, &idempotency_key, on_stream).await
        }
        Err(other) => Err(other),
    }
}

fn note_stream_event(
    ev: &ProviderStreamEvent,
    streamed_live: &mut bool,
    meaningful_delta: &mut bool,
    response: &mut Option<AgentResponse>,
) {
    match ev {
        ProviderStreamEvent::ResponseStarted { live, .. } => {
            if *live {
                *streamed_live = true;
            }
        }
        ProviderStreamEvent::TextDelta { text } => {
            if !text.is_empty() {
                *meaningful_delta = true;
                *streamed_live = true;
            }
        }
        ProviderStreamEvent::ResponseCompleted {
            response: r,
            buffered,
        } => {
            if !*buffered {
                *streamed_live = true;
            }
            *response = Some(r.clone());
        }
        _ => {}
    }
}

async fn run_chat_stream(
    provider: &dyn AiProvider,
    request: &AgentRequest,
    idempotency_key: &str,
    on_stream: &mut impl FnMut(ProviderStreamEvent),
) -> Result<(AgentResponse, bool), StreamAttemptError> {
    let (tx, mut rx): (ProviderStreamTx, _) = tokio::sync::mpsc::channel(64);
    let attempt = AgentRequest {
        system_prompt: request.system_prompt.clone(),
        messages: request.messages.clone(),
        cancel: request.cancel.clone(),
        idempotency_key: Some(idempotency_key.to_string()),
        max_output_tokens: request.max_output_tokens,
    };

    let provider_fut = provider.chat_stream(attempt, tx);
    tokio::pin!(provider_fut);

    let mut streamed_live = false;
    let mut meaningful_delta = false;
    let mut response: Option<AgentResponse> = None;

    loop {
        tokio::select! {
            biased;
            event = rx.recv() => {
                match event {
                    Some(ev) => {
                        note_stream_event(
                            &ev,
                            &mut streamed_live,
                            &mut meaningful_delta,
                            &mut response,
                        );
                        on_stream(ev);
                    }
                    None => {
                        // Channel closed — wait for provider result.
                        break;
                    }
                }
            }
            result = &mut provider_fut => {
                // Drain remaining events before returning.
                while let Ok(ev) = rx.try_recv() {
                    note_stream_event(
                        &ev,
                        &mut streamed_live,
                        &mut meaningful_delta,
                        &mut response,
                    );
                    on_stream(ev);
                }
                match result {
                    Ok(r) => {
                        return Ok((response.unwrap_or(r), streamed_live));
                    }
                    Err(err) => {
                        return Err(StreamAttemptError::Failed {
                            err,
                            meaningful_delta,
                        });
                    }
                }
            }
        }
    }

    // Receiver ended before provider future; still await it.
    match provider_fut.await {
        Ok(r) => Ok((response.unwrap_or(r), streamed_live)),
        Err(err) => Err(StreamAttemptError::Failed {
            err,
            meaningful_delta,
        }),
    }
}

fn build_provider(access: &ResolvedAiAccess, model: &str) -> Result<Arc<dyn AiProvider>, AiError> {
    crate::ai::create_provider_for_access(access, Some(model))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::{AgentMessage, ProviderStreamTx};
    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn auto_candidates_dedupe_and_prefer_default() {
        let list = auto_model_candidates("openrouter", "openai/gpt-4o-mini");
        assert_eq!(list[0], "openai/gpt-4o-mini");
        assert!(list.iter().any(|m| m == "google/gemini-2.5-flash"));
        assert_eq!(
            list.iter().filter(|m| *m == "openai/gpt-4o-mini").count(),
            1
        );
    }

    #[test]
    fn auto_pref_detection() {
        assert!(is_auto_preference("auto"));
        assert!(is_auto_preference(""));
        assert!(!is_auto_preference("google/gemini-2.5-flash"));
    }

    #[test]
    fn rate_limit_detection_avoids_generate_false_positive() {
        let generate_err =
            AiError::Provider("Gemini HTTP 400: generateContent failed: INVALID_ARGUMENT".into());
        assert!(!is_rate_limited(&generate_err));
        assert!(is_retryable_model_error(&generate_err));

        let limited = AiError::Provider("Gemini HTTP 429: rate limit exceeded".into());
        assert!(is_rate_limited(&limited));
        assert!(is_retryable_model_error(&limited));

        let not_found = AiError::Provider("model not found: gemini-nope".into());
        assert!(is_retryable_model_error(&not_found));

        let cancelled = AiError::Cancelled;
        assert!(!is_retryable_model_error(&cancelled));
    }

    #[test]
    fn peek_assistant_message_partial_json() {
        let partial = r#"{"schemaVersion":"1","assistantMessage":"Hello wor"#;
        assert_eq!(
            peek_assistant_message(partial).as_deref(),
            Some("Hello wor")
        );
        let complete = r#"{"assistantMessage":"Done","responseType":"message"}"#;
        assert_eq!(peek_assistant_message(complete).as_deref(), Some("Done"));
        assert!(peek_assistant_message(r#"{"schemaVersion":"1"}"#).is_none());
    }

    #[test]
    fn peek_assistant_message_escapes_and_special_characters() {
        // Escaped quotes and newlines
        let escaped = r#"{"assistantMessage":"Line 1\nLine 2\t\"quoted\"\r\nDone"}"#;
        assert_eq!(
            peek_assistant_message(escaped).as_deref(),
            Some("Line 1\nLine 2\t\"quoted\"\r\nDone")
        );

        // Incomplete escape at stream boundary
        let cut_escape = r#"{"assistantMessage":"Incomplete escape: \"#;
        assert_eq!(
            peek_assistant_message(cut_escape).as_deref(),
            Some("Incomplete escape: ")
        );

        // Escaped backslashes
        let backslash = r#"{"assistantMessage":"Path C:\\Users\\Desktop"}"#;
        assert_eq!(
            peek_assistant_message(backslash).as_deref(),
            Some("Path C:\\Users\\Desktop")
        );

        // Unicode and emojis
        let unicode = r#"{"assistantMessage":"Hello 🌍 🚀 世界"}"#;
        assert_eq!(
            peek_assistant_message(unicode).as_deref(),
            Some("Hello 🌍 🚀 世界")
        );

        // Downstream backslashes in subsequent JSON fields do not defeat the fast path
        let downstream_escapes = r#"{"assistantMessage":"Clean message","payload":{"code":"fn main() {\n    println!(\"hello\");\n}"}}"#;
        assert_eq!(
            peek_assistant_message(downstream_escapes).as_deref(),
            Some("Clean message")
        );
    }

    #[test]
    fn peek_assistant_message_invalid_or_non_string_formats() {
        // Tolerates arbitrary whitespace before and after colon
        let padded = r#"{"assistantMessage"   :   "Padded text"}"#;
        assert_eq!(
            peek_assistant_message(padded).as_deref(),
            Some("Padded text")
        );

        // Non-string values must return None
        assert!(peek_assistant_message(r#"{"assistantMessage": 12345}"#).is_none());
        assert!(peek_assistant_message(r#"{"assistantMessage": true}"#).is_none());
        assert!(peek_assistant_message(r#"{"assistantMessage": {}}"#).is_none());

        // Empty message returns None (does not show blank bubble)
        assert!(peek_assistant_message(r#"{"assistantMessage":""}"#).is_none());

        // Substring key does not match
        assert!(peek_assistant_message(r#"{"not_assistantMessage":"text"}"#).is_none());
    }

    #[test]
    fn peek_assistant_message_unicode_and_adversarial_suite() {
        // Unicode escapes
        assert_eq!(
            peek_assistant_message(r#"{"assistantMessage":"\u263A"}"#).as_deref(),
            Some("☺")
        );
        assert_eq!(
            peek_assistant_message(r#"{"assistantMessage":"\u4F60\u597D"}"#).as_deref(),
            Some("你好")
        );
        // Surrogate pair
        assert_eq!(
            peek_assistant_message(r#"{"assistantMessage":"\uD83D\uDE00"}"#).as_deref(),
            Some("😀")
        );

        // Incomplete escapes at stream boundary
        assert_eq!(
            peek_assistant_message(r#"{"assistantMessage":"Hello \u26"#).as_deref(),
            Some("Hello ")
        );
        assert_eq!(
            peek_assistant_message(r#"{"assistantMessage":"Hello \u"#).as_deref(),
            Some("Hello ")
        );
        assert_eq!(
            peek_assistant_message(r#"{"assistantMessage":"Hello \""#).as_deref(),
            Some("Hello \"")
        );
        assert_eq!(
            peek_assistant_message(r#"{"assistantMessage":"Hello \uD83D"#).as_deref(),
            Some("Hello ")
        );

        // Top-level key security
        let nested_security = r#"{"payload":{"sub":"has \"assistantMessage\": \"secret\""},"assistantMessage":"Real message"}"#;
        assert_eq!(
            peek_assistant_message(nested_security).as_deref(),
            Some("Real message")
        );
    }

    #[test]
    fn peek_assistant_message_large_payload_slice_scan_performance() {
        // 50KB payload with repeated peek operations simulating live SSE stream chunks
        let large_body =
            "The model is generating continuous structured data with rich content. ".repeat(700);
        let partial_json = format!(r#"{{"schemaVersion":"1","assistantMessage":"{large_body}"#);

        for _ in 0..100 {
            let peeked = peek_assistant_message(&partial_json);
            assert!(peeked.is_some());
            assert_eq!(peeked.unwrap().len(), large_body.len());
        }
    }

    struct DelayedLiveProvider;

    #[async_trait]
    impl AiProvider for DelayedLiveProvider {
        fn provider_id(&self) -> &str {
            "mock-delayed"
        }
        fn display_name(&self) -> &str {
            "Delayed Live Mock"
        }
        async fn health_check(
            &self,
            _cancel: CancellationToken,
        ) -> Result<crate::ai::provider::ProviderHealth, AiError> {
            Ok(crate::ai::provider::ProviderHealth {
                ok: true,
                message: "ok".into(),
                models: vec![],
            })
        }
        async fn chat(&self, _request: AgentRequest) -> Result<AgentResponse, AiError> {
            Err(AiError::Provider("use chat_stream".into()))
        }
        async fn chat_stream(
            &self,
            request: AgentRequest,
            tx: ProviderStreamTx,
        ) -> Result<AgentResponse, AiError> {
            let _ = tx
                .send(ProviderStreamEvent::ResponseStarted {
                    provider_id: self.provider_id().to_string(),
                    model: "mock-delayed".into(),
                    live: true,
                })
                .await;
            let _ = tx
                .send(ProviderStreamEvent::TextDelta {
                    text: r#"{"assistantMessage":"First"#.into(),
                })
                .await;
            // Delay completion so callers can observe the first delta earlier.
            tokio::select! {
                _ = request.cancel.cancelled() => {
                    let _ = tx.send(ProviderStreamEvent::ResponseCancelled).await;
                    return Err(AiError::Cancelled);
                }
                _ = tokio::time::sleep(Duration::from_millis(80)) => {}
            }
            let raw = r#"{"schemaVersion":"1","assistantMessage":"First delta arrived","responseType":"message"}"#;
            let response = AgentResponse {
                raw_text: raw.into(),
                usage: Default::default(),
                model: "mock-delayed".into(),
                provider_id: self.provider_id().to_string(),
            };
            let _ = tx
                .send(ProviderStreamEvent::TextCompleted { text: raw.into() })
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

    #[tokio::test]
    async fn true_streaming_emits_delta_before_completion() {
        let provider = DelayedLiveProvider;
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let cancel = CancellationToken::new();
        let request = AgentRequest {
            system_prompt: "t".into(),
            messages: vec![AgentMessage::text(crate::ai::AgentRole::User, "hi")],
            cancel: cancel.clone(),
            idempotency_key: Some("k1".into()),
            max_output_tokens: None,
        };

        let started = std::time::Instant::now();
        let mut first_delta_at = None;
        let join = tokio::spawn(async move { provider.chat_stream(request, tx).await });

        while let Some(ev) = rx.recv().await {
            if matches!(ev, ProviderStreamEvent::TextDelta { .. }) && first_delta_at.is_none() {
                first_delta_at = Some(started.elapsed());
            }
            if matches!(ev, ProviderStreamEvent::ResponseCompleted { .. }) {
                break;
            }
        }
        let _ = join.await.unwrap().unwrap();
        let first = first_delta_at.expect("expected TextDelta");
        assert!(
            first < Duration::from_millis(50),
            "first delta should arrive before delayed completion, got {first:?}"
        );
    }

    struct FailAfterDeltaProvider;

    #[async_trait]
    impl AiProvider for FailAfterDeltaProvider {
        fn provider_id(&self) -> &str {
            "mock-fail-after-delta"
        }
        fn display_name(&self) -> &str {
            "Fail After Delta"
        }
        async fn health_check(
            &self,
            _cancel: CancellationToken,
        ) -> Result<crate::ai::provider::ProviderHealth, AiError> {
            Ok(crate::ai::provider::ProviderHealth {
                ok: true,
                message: "ok".into(),
                models: vec![],
            })
        }
        async fn chat(&self, _request: AgentRequest) -> Result<AgentResponse, AiError> {
            Err(AiError::Provider("use chat_stream".into()))
        }
        async fn chat_stream(
            &self,
            _request: AgentRequest,
            tx: ProviderStreamTx,
        ) -> Result<AgentResponse, AiError> {
            let _ = tx
                .send(ProviderStreamEvent::ResponseStarted {
                    provider_id: self.provider_id().to_string(),
                    model: "mock-fail".into(),
                    live: true,
                })
                .await;
            let _ = tx
                .send(ProviderStreamEvent::TextDelta {
                    text: r#"{"assistantMessage":"Partial"#.into(),
                })
                .await;
            let err = AiError::Provider("simulated mid-stream failure".into());
            let _ = tx
                .send(ProviderStreamEvent::ResponseFailed {
                    code: err.code().to_string(),
                    message: err.to_string(),
                })
                .await;
            Err(err)
        }
    }

    #[tokio::test]
    async fn failure_after_text_delta_marks_meaningful() {
        let provider = FailAfterDeltaProvider;
        let cancel = CancellationToken::new();
        let request = AgentRequest {
            system_prompt: "t".into(),
            messages: vec![AgentMessage::text(crate::ai::AgentRole::User, "hi")],
            cancel,
            idempotency_key: Some("k-fail".into()),
            max_output_tokens: None,
        };
        let mut saw_delta = false;
        let result = run_chat_stream(&provider, &request, "k-fail", &mut |ev| {
            if matches!(ev, ProviderStreamEvent::TextDelta { .. }) {
                saw_delta = true;
            }
        })
        .await;
        assert!(saw_delta, "expected TextDelta before failure");
        match result {
            Err(StreamAttemptError::Failed {
                meaningful_delta: true,
                ..
            }) => {}
            other => panic!("expected meaningful_delta failure, got {other:?}"),
        }
    }

    struct SilentFailProvider;

    #[async_trait]
    impl AiProvider for SilentFailProvider {
        fn provider_id(&self) -> &str {
            "mock-silent-fail"
        }
        fn display_name(&self) -> &str {
            "Silent Fail"
        }
        async fn health_check(
            &self,
            _cancel: CancellationToken,
        ) -> Result<crate::ai::provider::ProviderHealth, AiError> {
            Ok(crate::ai::provider::ProviderHealth {
                ok: true,
                message: "ok".into(),
                models: vec![],
            })
        }
        async fn chat(&self, _request: AgentRequest) -> Result<AgentResponse, AiError> {
            Err(AiError::Provider("use chat_stream".into()))
        }
        async fn chat_stream(
            &self,
            _request: AgentRequest,
            tx: ProviderStreamTx,
        ) -> Result<AgentResponse, AiError> {
            let _ = tx
                .send(ProviderStreamEvent::ResponseStarted {
                    provider_id: self.provider_id().to_string(),
                    model: "mock-silent".into(),
                    live: true,
                })
                .await;
            // No TextDelta — failure before user-visible content.
            Err(AiError::Provider("unavailable".into()))
        }
    }

    fn ollama_authless_access() -> crate::credentials::ResolvedAiAccess {
        crate::credentials::ResolvedAiAccess {
            credentials: crate::credentials::ResolvedCredentials {
                provider: "ollama".into(),
                api_key: None,
                model: "llama3.2".into(),
                base_url: "http://127.0.0.1:11434".into(),
                source: "connection".into(),
                active_connection_id: Some("ollama-1".into()),
                env_path: None,
                missing_connection_secret: false,
            },
            route: crate::credentials::AiAccessRoute::LocalAuthless,
        }
    }

    #[test]
    fn build_provider_preserves_local_route_across_model_retries() {
        let access = ollama_authless_access();
        let first = build_provider(&access, "llama3.2").expect("first model");
        let second = build_provider(&access, "mistral").expect("retry model");
        assert_eq!(first.provider_id(), "ollama");
        assert_eq!(second.provider_id(), "ollama");
        assert_ne!(first.provider_id(), "coreside_hosted");
        assert_ne!(second.provider_id(), "coreside_hosted");
    }

    #[tokio::test]
    async fn failure_before_text_delta_is_not_meaningful() {
        let provider = SilentFailProvider;
        let cancel = CancellationToken::new();
        let request = AgentRequest {
            system_prompt: "t".into(),
            messages: vec![AgentMessage::text(crate::ai::AgentRole::User, "hi")],
            cancel,
            idempotency_key: Some("k-silent".into()),
            max_output_tokens: None,
        };
        let result = run_chat_stream(&provider, &request, "k-silent", &mut |_| {}).await;
        match result {
            Err(StreamAttemptError::Failed {
                meaningful_delta: false,
                ..
            }) => {}
            other => panic!("expected non-meaningful failure, got {other:?}"),
        }
    }
}
