//! Environment-based configuration loader.
//!
//! Precedence:
//! 1. Provider-neutral `AI_*` values
//! 2. Aliases for the selected `AI_PROVIDER` (`GEMINI_*`, `OPENAI_*`, `ANTHROPIC_*` / `CLAUDE_*`)
//! 3. Built-in defaults for non-secret values only
//!
//! Only the Gemini adapter is implemented today. Keys for other providers are
//! accepted so `.env` can hold them ahead of future adapters.

use std::env;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub const DEFAULT_PROVIDER: &str = "gemini";
pub const DEFAULT_LOG_LEVEL: &str = "info";

pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3.5-flash";
pub const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub const DEFAULT_OPENAI_MODEL: &str = "gpt-4.1-mini";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-4-5";
pub const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";

/// Back-compat alias used by existing tests and docs.
pub const DEFAULT_MODEL: &str = DEFAULT_GEMINI_MODEL;
/// Back-compat alias used by existing tests and docs.
pub const DEFAULT_BASE_URL: &str = DEFAULT_GEMINI_BASE_URL;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: String,
    pub log_level: String,
}

impl AppConfig {
    /// Never expose the API key to the frontend — bool only.
    pub fn has_api_key(&self) -> bool {
        self.api_key
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }

    /// Safe snapshot for IPC (no secrets).
    pub fn public_ai_status(
        &self,
        status: &str,
        message: Option<String>,
    ) -> PublicAiStatus {
        PublicAiStatus {
            provider: self.provider.clone(),
            model: self.model.clone(),
            key_detected: self.has_api_key(),
            status: status.to_string(),
            message,
            base_url: self.base_url.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAiStatus {
    pub provider: String,
    pub model: String,
    pub key_detected: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub base_url: String,
}

/// Load `.env` from project root (walk up from cwd, or parent of `CARGO_MANIFEST_DIR`), then build config.
pub fn load_config() -> AppConfig {
    load_dotenv_files();

    let provider = first_nonempty(&[env::var("AI_PROVIDER").ok()])
        .unwrap_or_else(|| DEFAULT_PROVIDER.to_string())
        .to_lowercase();

    let api_key = resolve_api_key(&provider);
    let model = resolve_model(&provider);
    let base_url = resolve_base_url(&provider);

    let log_level = first_nonempty(&[
        env::var("CORESIDE_LOG_LEVEL").ok(),
        env::var("RUST_LOG").ok(),
    ])
    .unwrap_or_else(|| DEFAULT_LOG_LEVEL.to_string());

    AppConfig {
        provider,
        api_key,
        model,
        base_url,
        log_level,
    }
}

fn resolve_api_key(provider: &str) -> Option<String> {
    let mut candidates = vec![env::var("AI_API_KEY").ok()];
    match provider {
        "gemini" => candidates.push(env::var("GEMINI_API_KEY").ok()),
        "openai" => candidates.push(env::var("OPENAI_API_KEY").ok()),
        "anthropic" | "claude" => {
            candidates.push(env::var("ANTHROPIC_API_KEY").ok());
            candidates.push(env::var("CLAUDE_API_KEY").ok());
        }
        "mock" => {}
        // Unknown provider: still accept common aliases so keys are not lost.
        _ => {
            candidates.push(env::var("GEMINI_API_KEY").ok());
            candidates.push(env::var("OPENAI_API_KEY").ok());
            candidates.push(env::var("ANTHROPIC_API_KEY").ok());
            candidates.push(env::var("CLAUDE_API_KEY").ok());
        }
    }
    first_nonempty(&candidates)
}

fn resolve_model(provider: &str) -> String {
    let mut candidates = vec![env::var("AI_MODEL").ok()];
    let default = match provider {
        "openai" => {
            candidates.push(env::var("OPENAI_MODEL").ok());
            DEFAULT_OPENAI_MODEL
        }
        "anthropic" | "claude" => {
            candidates.push(env::var("ANTHROPIC_MODEL").ok());
            candidates.push(env::var("CLAUDE_MODEL").ok());
            DEFAULT_ANTHROPIC_MODEL
        }
        "mock" => {
            candidates.push(env::var("GEMINI_MODEL").ok());
            "mock-fixture"
        }
        // gemini (default) and anything else
        _ => {
            candidates.push(env::var("GEMINI_MODEL").ok());
            DEFAULT_GEMINI_MODEL
        }
    };
    first_nonempty(&candidates).unwrap_or_else(|| default.to_string())
}

fn resolve_base_url(provider: &str) -> String {
    let mut candidates = vec![env::var("AI_BASE_URL").ok()];
    let default = match provider {
        "openai" => {
            candidates.push(env::var("OPENAI_BASE_URL").ok());
            DEFAULT_OPENAI_BASE_URL
        }
        "anthropic" | "claude" => {
            candidates.push(env::var("ANTHROPIC_BASE_URL").ok());
            DEFAULT_ANTHROPIC_BASE_URL
        }
        "mock" => {
            candidates.push(env::var("GEMINI_BASE_URL").ok());
            "mock://local"
        }
        _ => {
            candidates.push(env::var("GEMINI_BASE_URL").ok());
            DEFAULT_GEMINI_BASE_URL
        }
    };
    first_nonempty(&candidates).unwrap_or_else(|| default.to_string())
}

fn first_nonempty(candidates: &[Option<String>]) -> Option<String> {
    candidates
        .iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
}

fn load_dotenv_files() {
    for path in candidate_env_paths() {
        if path.is_file() {
            match dotenvy::from_path(&path) {
                Ok(()) => {
                    tracing::debug!(path = %path.display(), "loaded .env");
                    return;
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "failed to load .env");
                }
            }
        }
    }
    // Fall back to default dotenv search (cwd).
    let _ = dotenvy::dotenv();
}

fn candidate_env_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Parent of src-tauri (project root) via compile-time manifest dir.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest_dir.parent() {
        paths.push(root.join(".env"));
    }
    paths.push(manifest_dir.join(".env"));

    // Walk up from current working directory.
    if let Ok(cwd) = env::current_dir() {
        let mut dir: Option<&Path> = Some(cwd.as_path());
        while let Some(d) = dir {
            paths.push(d.join(".env"));
            dir = d.parent();
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_api_key_false_when_empty() {
        let cfg = AppConfig {
            provider: "gemini".into(),
            api_key: Some("  ".into()),
            model: DEFAULT_MODEL.into(),
            base_url: DEFAULT_BASE_URL.into(),
            log_level: "info".into(),
        };
        assert!(!cfg.has_api_key());
    }

    #[test]
    fn public_status_hides_key() {
        let cfg = AppConfig {
            provider: "gemini".into(),
            api_key: Some("secret-key".into()),
            model: DEFAULT_MODEL.into(),
            base_url: DEFAULT_BASE_URL.into(),
            log_level: "info".into(),
        };
        let status = cfg.public_ai_status("ready", None);
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("secret-key"));
        assert!(status.key_detected);
    }

    #[test]
    fn provider_defaults_are_distinct() {
        assert_ne!(DEFAULT_GEMINI_BASE_URL, DEFAULT_OPENAI_BASE_URL);
        assert_ne!(DEFAULT_OPENAI_BASE_URL, DEFAULT_ANTHROPIC_BASE_URL);
    }
}
