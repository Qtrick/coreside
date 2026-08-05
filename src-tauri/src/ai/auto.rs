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
                "google/gemini-2.5-flash",
                "meta-llama/llama-3.3-70b-instruct",
                "anthropic/claude-sonnet-4.5",
                "google/gemini-2.0-flash-001",
                "openrouter/auto",
            ] {
                push(&mut out, id);
            }
        }
        "gemini" => {
            for id in [
                "gemini-2.5-flash",
                "gemini-2.0-flash",
                "gemini-3.1-flash-lite",
                "gemini-3.5-flash",
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
    let idx = partial.find(KEY)?;
    let after = &partial[idx + KEY.len()..];
    let colon = after.find(':')?;
    let mut rest = after[colon + 1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    rest = &rest[1..];
    let mut out = String::new();
    let mut chars = rest.chars();
    let mut closed = false;
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(n) => out.push(n),
                None => break,
            }
        } else if c == '"' {
            closed = true;
            break;
        } else {
            out.push(c);
        }
    }
    if out.is_empty() && !closed {
        return None;
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
