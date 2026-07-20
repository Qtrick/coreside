//! AI layer: providers, prompts, and response parsing.

mod access_mode;
mod hosted_provider;
mod capability_registry;
mod tool_loop;
mod anthropic;
mod auto;
mod errors;
mod gemini;
mod mock;
mod openai;
mod openrouter;
mod prompt_builder;
mod provider;
mod response_parser;
mod response_schema;
mod settings_change;

pub use access_mode::{
    resolve_access_presentation, AiAccessPresentation, DisclosurePolicy,
};

pub use auto::chat_with_auto;
pub use errors::AiError;
pub use gemini::GeminiProvider;
pub use mock::MockAiProvider;
pub use openai::OpenAiProvider;
pub use anthropic::AnthropicProvider;
pub use openrouter::OpenRouterProvider;
pub use prompt_builder::{
    build_agent_prompt_with_references, PROMPT_VERSION,
};
pub use capability_registry::{
    AgentCapability, ToolCallRequest, ToolCallResult,
};
pub use tool_loop::{project_context_for_prompt, ToolLoop, ToolLoopContext};
pub use provider::{AgentMessage, AgentRequest, AiProvider, ProviderHealth};
pub use response_parser::{parse_agent_response, ParsedAgentResponse};
pub use response_schema::{
    layout_type_string, ResponseType, SourceCitation, ToolAction, ToolChangePayload,
    ToolComponent, ToolDefinition,
};
pub use settings_change::{
    is_allowed_setting_key, normalize_hex_or_none, normalize_setting_kv, parse_wallpaper_setting,
    SettingsChangePayload, WallpaperConfig,
};

use crate::config::AppConfig;
use std::sync::Arc;

/// Create an AI provider from config. Uses mock when `AI_PROVIDER=mock`.
pub fn create_provider(config: &AppConfig) -> Result<Arc<dyn AiProvider>, AiError> {
    create_provider_with_model(config, None)
}

/// Create a provider, optionally overriding the model id (`None` / `"auto"` → config default).
pub fn create_provider_with_model(
    config: &AppConfig,
    model_override: Option<&str>,
) -> Result<Arc<dyn AiProvider>, AiError> {
    let model = resolve_runtime_model(config, model_override);
    let provider = config.provider.to_lowercase();

    if provider == "mock" {
        return Ok(Arc::new(MockAiProvider::new()));
    }

    if !config.has_api_key() {
        if let Some(hosted) = hosted_provider::try_from_session()? {
            return Ok(hosted);
        }
    }

    let key = config
        .api_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| {
            AiError::NotConfigured(
                "AI API key not configured. Connect a provider in Settings or set a key in .env."
                    .into(),
            )
        })?;

    match provider.as_str() {
        "gemini" => Ok(Arc::new(GeminiProvider::new(
            key,
            model,
            config.base_url.clone(),
        ))),
        "openrouter" => Ok(Arc::new(OpenRouterProvider::new(
            key,
            model,
            config.base_url.clone(),
        ))),
        "openai" => Ok(Arc::new(OpenAiProvider::new(
            key,
            model,
            config.base_url.clone(),
        ))),
        "compatible" => {
            if config.base_url.trim().is_empty() {
                return Err(AiError::Validation(
                    "Custom OpenAI-compatible providers require a base URL".into(),
                ));
            }
            Ok(Arc::new(OpenAiProvider::new_compatible(
                key,
                model,
                config.base_url.clone(),
            )))
        }
        "anthropic" | "claude" => Ok(Arc::new(AnthropicProvider::new(
            key,
            model,
            config.base_url.clone(),
        ))),
        other => Err(AiError::Validation(format!(
            "Unsupported AI provider: {other}. Supported: gemini, openai, anthropic, openrouter, compatible, mock."
        ))),
    }
}

fn resolve_runtime_model(config: &AppConfig, model_override: Option<&str>) -> String {
    match model_override.map(str::trim).filter(|s| !s.is_empty()) {
        Some(id) if !id.eq_ignore_ascii_case("auto") => id.to_string(),
        _ => config.model.clone(),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalog {
    pub provider: String,
    /// Persisted preference: `"auto"` or a concrete model id.
    pub selected: String,
    /// What Auto resolves to from `.env` / defaults. Hidden in Auto UI label.
    pub auto_resolves_to: String,
    pub options: Vec<ModelOption>,
}

/// Curated model catalog for the chat picker (Auto + specific models).
pub fn model_catalog(config: &AppConfig, selected: &str) -> ModelCatalog {
    let mut options = vec![ModelOption {
        id: "auto".into(),
        label: "Auto".into(),
        description: Some("Picks the best available model automatically".into()),
    }];

    match config.provider.to_lowercase().as_str() {
        "openai" | "compatible" => {
            options.extend([
                ModelOption {
                    id: "gpt-4.1-mini".into(),
                    label: "GPT-4.1 Mini".into(),
                    description: Some("Fast and affordable".into()),
                },
                ModelOption {
                    id: "gpt-4.1".into(),
                    label: "GPT-4.1".into(),
                    description: Some("Strong general model".into()),
                },
            ]);
        }
        "anthropic" | "claude" => {
            options.extend([
                ModelOption {
                    id: "claude-sonnet-4-5".into(),
                    label: "Claude Sonnet 4.5".into(),
                    description: Some("Balanced Anthropic model".into()),
                },
                ModelOption {
                    id: "claude-haiku-4-5".into(),
                    label: "Claude Haiku 4.5".into(),
                    description: Some("Fast and inexpensive".into()),
                },
            ]);
        }
        "openrouter" => {
            options.extend([
                ModelOption {
                    id: "google/gemini-2.5-flash".into(),
                    label: "Gemini 2.5 Flash".into(),
                    description: Some("Fast via OpenRouter".into()),
                },
                ModelOption {
                    id: "openai/gpt-4o-mini".into(),
                    label: "GPT-4o Mini".into(),
                    description: Some("Affordable OpenAI via OpenRouter".into()),
                },
                ModelOption {
                    id: "anthropic/claude-sonnet-4.5".into(),
                    label: "Claude Sonnet 4.5".into(),
                    description: Some("Via OpenRouter".into()),
                },
                ModelOption {
                    id: "meta-llama/llama-3.3-70b-instruct".into(),
                    label: "Llama 3.3 70B".into(),
                    description: Some("Open weights via OpenRouter".into()),
                },
            ]);
        }
        "mock" => {
            options.push(ModelOption {
                id: "mock-fixture".into(),
                label: "Mock fixture".into(),
                description: Some("Deterministic offline responses".into()),
            });
        }
        _ => {
            options.extend([
                ModelOption {
                    id: "gemini-3.5-flash".into(),
                    label: "Gemini 3.5 Flash".into(),
                    description: Some("Recommended — fast agentic Flash".into()),
                },
                ModelOption {
                    id: "gemini-3.1-flash-lite".into(),
                    label: "Gemini 3.1 Flash-Lite".into(),
                    description: Some("Lowest latency / cost".into()),
                },
                ModelOption {
                    id: "gemini-2.5-flash".into(),
                    label: "Gemini 2.5 Flash".into(),
                    description: Some("Previous Flash generation".into()),
                },
                ModelOption {
                    id: "gemini-2.5-pro".into(),
                    label: "Gemini 2.5 Pro".into(),
                    description: Some("Higher capability, slower".into()),
                },
            ]);
        }
    }

    let selected = {
        let trimmed = selected.trim();
        if trimmed.is_empty() {
            "auto".to_string()
        } else {
            trimmed.to_string()
        }
    };

    ModelCatalog {
        provider: config.provider.clone(),
        selected,
        auto_resolves_to: config.model.clone(),
        options,
    }
}

/// Catalog stub when disclosure forbids provider model listing (no upstream slugs).
pub fn auto_only_catalog(selected: &str) -> ModelCatalog {
    let selected = {
        let trimmed = selected.trim();
        if trimmed.is_empty() {
            "auto".to_string()
        } else {
            trimmed.to_string()
        }
    };
    ModelCatalog {
        provider: String::new(),
        selected,
        auto_resolves_to: "auto".into(),
        options: vec![ModelOption {
            id: "auto".into(),
            label: "Auto".into(),
            description: Some("Picks the best available model automatically".into()),
        }],
    }
}
