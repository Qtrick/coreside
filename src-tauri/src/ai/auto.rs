//! Auto model selection — try candidates and use the first that works.
//!
//! Important: avoid probe+chat double requests (burns Gemini/OpenRouter rate limits).
//! On 429, back off once before switching models.

use std::sync::Arc;
use std::time::Duration;

use super::errors::AiError;
use super::provider::{AgentRequest, AgentResponse, AiProvider};
use crate::config::AppConfig;

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
                // Prefer non-Gemini first when default is Gemini-heavy, to avoid shared rate walls.
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
    // Avoid bare "rate" — it false-positives on "generate" / "generateContent".
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
}

/// Run chat with optional Auto fallback across candidate models.
pub async fn chat_with_auto(
    config: &AppConfig,
    model_preference: &str,
    request: AgentRequest,
    mut on_action: impl FnMut(&str),
) -> Result<ResolvedChat, AiError> {
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

    if !auto_mode {
        on_action("Connecting to the model");
    }

    for model in candidates.iter() {
        if request.cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }

        attempts += 1;
        let provider = build_provider(config, model)?;

        // Single chat attempt (no separate probe — probes double rate-limit usage).
        on_action("Generating a response");
        match chat_with_rate_limit_retry(provider.as_ref(), &request).await {
            Ok(response) => {
                on_action("Reading the response");
                return Ok(ResolvedChat {
                    model_used: response.model.clone(),
                    response,
                    auto_mode,
                    attempts,
                });
            }
            Err(err) => {
                if auto_mode && is_retryable_model_error(&err) {
                    last_err = Some(err);
                    continue;
                }
                return Err(err);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| AiError::Provider("Auto could not find a working model".into())))
}

async fn chat_with_rate_limit_retry(
    provider: &dyn AiProvider,
    request: &AgentRequest,
) -> Result<AgentResponse, AiError> {
    let first = provider
        .chat(AgentRequest {
            system_prompt: request.system_prompt.clone(),
            messages: request.messages.clone(),
            cancel: request.cancel.clone(),
        })
        .await;

    match first {
        Ok(response) => Ok(response),
        Err(err) if is_rate_limited(&err) => {
            // One quiet backoff, then a single retry on the same model.
            tokio::select! {
                _ = request.cancel.cancelled() => return Err(AiError::Cancelled),
                _ = tokio::time::sleep(Duration::from_millis(1200)) => {}
            }
            provider
                .chat(AgentRequest {
                    system_prompt: request.system_prompt.clone(),
                    messages: request.messages.clone(),
                    cancel: request.cancel.clone(),
                })
                .await
        }
        Err(err) => Err(err),
    }
}

fn build_provider(config: &AppConfig, model: &str) -> Result<Arc<dyn AiProvider>, AiError> {
    // Delegate to the shared factory so openai/anthropic/compatible stay wired.
    crate::ai::create_provider_with_model(config, Some(model))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Provider errors remain retryable for Auto fallback — just not via the rate-limit path.
        assert!(is_retryable_model_error(&generate_err));

        let limited = AiError::Provider("Gemini HTTP 429: rate limit exceeded".into());
        assert!(is_rate_limited(&limited));
        assert!(is_retryable_model_error(&limited));

        let not_found = AiError::Provider("model not found: gemini-nope".into());
        assert!(is_retryable_model_error(&not_found));

        let cancelled = AiError::Cancelled;
        assert!(!is_retryable_model_error(&cancelled));
    }
}
