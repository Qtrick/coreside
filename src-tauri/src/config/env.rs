//! Environment-based configuration loader.
//!
//! Precedence:
//! 1. Aliases for the selected `AI_PROVIDER`
//!    (`GEMINI_*`, `OPENAI_*`, `ANTHROPIC_*` / `CLAUDE_*`, `OPENROUTER_*`)
//! 2. Provider-neutral `AI_*` values
//! 3. Built-in defaults for non-secret values only
//!
//! Gemini and OpenRouter adapters are implemented today. Keys for other
//! providers are accepted so `.env` can hold them ahead of future adapters.
//! OpenAI, Anthropic, and compatible endpoints are also supported at runtime.

use std::env;
use std::path::PathBuf;

use serde::Serialize;

pub const DEFAULT_PROVIDER: &str = "gemini";
pub const DEFAULT_LOG_LEVEL: &str = "info";

pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3.5-flash";
pub const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub const DEFAULT_OPENAI_MODEL: &str = "gpt-4.1-mini";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-4-5";
pub const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";

pub const DEFAULT_OPENROUTER_MODEL: &str = "google/gemini-2.5-flash";
pub const DEFAULT_OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Publishable Supabase client config for hosted AI (never service-role).
pub fn supabase_publishable_config() -> Option<(String, String)> {
    let url = read_env_any(&["SUPABASE_URL", "VITE_SUPABASE_URL"])?;
    let key = read_env_any(&[
        "SUPABASE_PUBLISHABLE_KEY",
        "SUPABASE_ANON_KEY",
        "VITE_SUPABASE_PUBLISHABLE_KEY",
    ])?;
    Some((url, key))
}

fn read_env_any(keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(value) = env::var(key) {
            let trimmed = strip_env_quotes(value.trim());
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: String,
    pub log_level: String,
    /// Absolute path of the `.env` that was loaded, if any.
    pub env_path: Option<String>,
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
    pub fn public_ai_status(&self, status: &str, message: Option<String>) -> PublicAiStatus {
        self.public_ai_status_with_source(status, message, "env", None)
    }

    pub fn public_ai_status_with_source(
        &self,
        status: &str,
        message: Option<String>,
        source: &str,
        active_connection_id: Option<String>,
    ) -> PublicAiStatus {
        self.public_ai_status_with_presentation(status, message, source, active_connection_id, None)
    }

    pub fn public_ai_status_with_presentation(
        &self,
        status: &str,
        message: Option<String>,
        source: &str,
        active_connection_id: Option<String>,
        presentation: Option<crate::ai::AiAccessPresentation>,
    ) -> PublicAiStatus {
        let disclosure = presentation.as_ref().map(|p| p.disclosure.clone());
        let show_provider = disclosure
            .as_ref()
            .map(|d| d.show_provider_identity)
            .unwrap_or(true);
        let show_model = disclosure
            .as_ref()
            .map(|d| d.show_model_identity)
            .unwrap_or(true);
        let show_cred = disclosure
            .as_ref()
            .map(|d| d.show_credential_source)
            .unwrap_or(true);
        let show_dev = disclosure
            .as_ref()
            .map(|d| d.allow_developer_details)
            .unwrap_or(false);

        PublicAiStatus {
            provider: if show_provider {
                self.provider.clone()
            } else {
                String::new()
            },
            model: if show_model {
                self.model.clone()
            } else {
                String::new()
            },
            key_detected: self.has_api_key(),
            status: status.to_string(),
            message: if show_cred || status != "ready" {
                message
            } else {
                None
            },
            base_url: if show_provider {
                self.base_url.clone()
            } else {
                String::new()
            },
            env_path: if show_dev {
                self.env_path.clone()
            } else {
                None
            },
            source: source.to_string(),
            active_connection_id,
            access_mode: presentation
                .as_ref()
                .map(|p| p.access_mode.as_str().to_string()),
            consumer_display_name: presentation
                .as_ref()
                .map(|p| p.consumer_display_name.clone()),
            user_facing_status: presentation.as_ref().map(|p| p.user_facing_status.clone()),
            disclosure,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_path: Option<String>,
    /// `connection` | `env` | `none`
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_connection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_facing_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disclosure: Option<crate::ai::DisclosurePolicy>,
}

/// Load `.env` only under the developer dotenv policy, then build config.
pub fn load_config() -> AppConfig {
    let env_path = load_dotenv_files();

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
        env_path: env_path.map(|p| p.display().to_string()),
    }
}

fn resolve_api_key(provider: &str) -> Option<String> {
    // Prefer the active provider's alias, then the neutral AI_API_KEY.
    // This lets several keys live in `.env` while AI_PROVIDER selects which one is used.
    let mut candidates: Vec<Option<String>> = Vec::new();
    match provider {
        "gemini" => candidates.push(env::var("GEMINI_API_KEY").ok()),
        "openai" | "compatible" => candidates.push(env::var("OPENAI_API_KEY").ok()),
        "anthropic" | "claude" => {
            candidates.push(env::var("ANTHROPIC_API_KEY").ok());
            candidates.push(env::var("CLAUDE_API_KEY").ok());
        }
        "openrouter" => candidates.push(env::var("OPENROUTER_API_KEY").ok()),
        "mock" => {}
        _ => {
            candidates.push(env::var("GEMINI_API_KEY").ok());
            candidates.push(env::var("OPENAI_API_KEY").ok());
            candidates.push(env::var("ANTHROPIC_API_KEY").ok());
            candidates.push(env::var("CLAUDE_API_KEY").ok());
            candidates.push(env::var("OPENROUTER_API_KEY").ok());
        }
    }
    candidates.push(env::var("AI_API_KEY").ok());
    first_nonempty(&candidates)
}

fn resolve_model(provider: &str) -> String {
    let mut candidates: Vec<Option<String>> = Vec::new();
    let default = match provider {
        "openai" | "compatible" => {
            candidates.push(env::var("OPENAI_MODEL").ok());
            candidates.push(env::var("AI_MODEL").ok());
            DEFAULT_OPENAI_MODEL
        }
        "anthropic" | "claude" => {
            candidates.push(env::var("ANTHROPIC_MODEL").ok());
            candidates.push(env::var("CLAUDE_MODEL").ok());
            candidates.push(env::var("AI_MODEL").ok());
            DEFAULT_ANTHROPIC_MODEL
        }
        "openrouter" => {
            candidates.push(env::var("OPENROUTER_MODEL").ok());
            // Only accept AI_MODEL when it looks like an OpenRouter id (vendor/model).
            if let Ok(m) = env::var("AI_MODEL") {
                if m.contains('/') {
                    candidates.push(Some(m));
                }
            }
            DEFAULT_OPENROUTER_MODEL
        }
        "mock" => {
            candidates.push(env::var("AI_MODEL").ok());
            candidates.push(env::var("GEMINI_MODEL").ok());
            "mock-fixture"
        }
        // gemini (default) and anything else
        _ => {
            candidates.push(env::var("GEMINI_MODEL").ok());
            candidates.push(env::var("AI_MODEL").ok());
            DEFAULT_GEMINI_MODEL
        }
    };
    first_nonempty(&candidates).unwrap_or_else(|| default.to_string())
}

fn resolve_base_url(provider: &str) -> String {
    let mut candidates: Vec<Option<String>> = Vec::new();
    let default = match provider {
        "openai" => {
            candidates.push(env::var("OPENAI_BASE_URL").ok());
            candidates.push(env::var("AI_BASE_URL").ok());
            DEFAULT_OPENAI_BASE_URL
        }
        "compatible" => {
            // Compatible must set AI_BASE_URL / OPENAI_BASE_URL; no silent vendor default.
            candidates.push(env::var("OPENAI_BASE_URL").ok());
            candidates.push(env::var("AI_BASE_URL").ok());
            ""
        }
        "anthropic" | "claude" => {
            candidates.push(env::var("ANTHROPIC_BASE_URL").ok());
            candidates.push(env::var("AI_BASE_URL").ok());
            DEFAULT_ANTHROPIC_BASE_URL
        }
        "openrouter" => {
            candidates.push(env::var("OPENROUTER_BASE_URL").ok());
            candidates.push(env::var("AI_BASE_URL").ok());
            DEFAULT_OPENROUTER_BASE_URL
        }
        "mock" => {
            candidates.push(env::var("AI_BASE_URL").ok());
            candidates.push(env::var("GEMINI_BASE_URL").ok());
            "mock://local"
        }
        _ => {
            candidates.push(env::var("GEMINI_BASE_URL").ok());
            candidates.push(env::var("AI_BASE_URL").ok());
            DEFAULT_GEMINI_BASE_URL
        }
    };
    first_nonempty(&candidates).unwrap_or_else(|| default.to_string())
}

fn strip_env_quotes(raw: &str) -> String {
    let s = raw.trim();
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        if (bytes[0] == b'"' && bytes[s.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[s.len() - 1] == b'\'')
        {
            return s[1..s.len() - 1].trim().to_string();
        }
    }
    s.to_string()
}

fn first_nonempty(candidates: &[Option<String>]) -> Option<String> {
    candidates
        .iter()
        .flatten()
        .map(|s| strip_env_quotes(s))
        .find(|s| !s.is_empty())
}

fn load_dotenv_files() -> Option<PathBuf> {
    // E2E orchestrator sets AI_PROVIDER=mock (and isolated DB). Repo `.env` must
    // not override those via from_path_override — that broke Journey 12 (openrouter
    // Auto streamed "Coreside cannot…" instead of the live stream probe).
    if std::env::var("CORESIDE_E2E")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        tracing::info!("dotenv loading skipped (CORESIDE_E2E=1)");
        return None;
    }

    // Production / packaged builds must not walk cwd/parents for `.env`.
    // Developer opt-in: debug build OR explicit CORESIDE_ALLOW_DOTENV=1.
    let allow = cfg!(debug_assertions)
        || std::env::var("CORESIDE_ALLOW_DOTENV")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
    if !allow {
        tracing::debug!("dotenv loading disabled (release build without CORESIDE_ALLOW_DOTENV)");
        return None;
    }

    let candidates = candidate_env_paths();
    for path in &candidates {
        if path.is_file() {
            // Override so Refresh Status picks up keys after the user saves `.env`.
            match dotenvy::from_path_override(path) {
                Ok(()) => {
                    tracing::info!(path = %path.display(), "loaded .env (developer override)");
                    return Some(path.clone());
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "failed to load .env");
                }
            }
        }
    }
    tracing::debug!(
        candidates = ?candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "no .env file found under developer dotenv policy"
    );
    // Do not call dotenvy::dotenv() — that searches cwd silently.
    None
}

fn candidate_env_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Parent of src-tauri (project root) via compile-time manifest dir — debug/dev only.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest_dir.parent() {
        paths.push(root.join(".env"));
    }
    paths.push(manifest_dir.join(".env"));

    // Optional: cwd only (not parent walk) when explicitly allowed.
    if let Ok(cwd) = env::current_dir() {
        paths.push(cwd.join(".env"));
    }

    // Deduplicate while preserving order.
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    paths
}

/// True when dotenv loading is compiled/active for this process.
pub fn dotenv_loading_allowed() -> bool {
    cfg!(debug_assertions)
        || std::env::var("CORESIDE_ALLOW_DOTENV")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_api_key_false_when_empty() {
        let cfg = AppConfig {
            provider: "gemini".into(),
            api_key: Some("  ".into()),
            model: DEFAULT_GEMINI_MODEL.into(),
            base_url: DEFAULT_GEMINI_BASE_URL.into(),
            log_level: "info".into(),
            env_path: None,
        };
        assert!(!cfg.has_api_key());
    }

    #[test]
    fn public_status_hides_key() {
        let cfg = AppConfig {
            provider: "gemini".into(),
            api_key: Some("secret-key".into()),
            model: DEFAULT_GEMINI_MODEL.into(),
            base_url: DEFAULT_GEMINI_BASE_URL.into(),
            log_level: "info".into(),
            env_path: Some("/tmp/.env".into()),
        };
        let status = cfg.public_ai_status("ready", None);
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("secret-key"));
        assert!(status.key_detected);
        // Without a presentation policy, env_path is omitted (not a consumer surface).
        assert!(status.env_path.is_none());
    }

    #[test]
    fn provider_defaults_are_distinct() {
        assert_ne!(DEFAULT_GEMINI_BASE_URL, DEFAULT_OPENAI_BASE_URL);
        assert_ne!(DEFAULT_OPENAI_BASE_URL, DEFAULT_ANTHROPIC_BASE_URL);
        assert_ne!(DEFAULT_OPENROUTER_BASE_URL, DEFAULT_OPENAI_BASE_URL);
    }

    #[test]
    fn strip_quotes_from_env_values() {
        assert_eq!(strip_env_quotes("\"abc\""), "abc");
        assert_eq!(strip_env_quotes("'abc'"), "abc");
        assert_eq!(strip_env_quotes("  abc  "), "abc");
    }

    #[test]
    fn dotenv_policy_is_debug_or_explicit() {
        let allowed = dotenv_loading_allowed();
        assert_eq!(
            allowed,
            cfg!(debug_assertions)
                || std::env::var("CORESIDE_ALLOW_DOTENV")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
        );
    }
}
