//! Ollama native API helpers (authless Local AI).
//!
//! Official local API: http://localhost:11434
//! - GET /api/tags — list models
//! - POST /api/chat — native chat (NDJSON stream)
//!
//! Chat streaming still may route through OpenAI-compat `/v1` until the full
//! native stream adapter is wired into `AiProvider`. Health + discovery use
//! the native endpoints here.

use super::errors::AiError;
use super::http_limits::{read_response_text_bounded, MAX_PROVIDER_RESPONSE_BYTES};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

const DEFAULT_OLLAMA_ORIGIN: &str = "http://127.0.0.1:11434";

pub fn normalize_ollama_origin(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return DEFAULT_OLLAMA_ORIGIN.to_string();
    }
    // Strip accidental /v1 suffix from OpenAI-compat configs.
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

/// Bounded GET /api/tags — no Authorization header.
pub async fn list_ollama_models(
    base_url: &str,
    cancel: &CancellationToken,
) -> Result<Vec<String>, AiError> {
    let origin = normalize_ollama_origin(base_url);
    let url = format!("{origin}/api/tags");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
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
    let text = read_response_text_bounded(resp, cancel, MAX_PROVIDER_RESPONSE_BYTES, None).await?;
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
    list_ollama_models(base_url, cancel).await.map(|_| true)
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
}
