//! AI layer: providers, prompts, and response parsing.

mod access_mode;
mod anthropic;
mod anthropic_family;
mod auto;
mod capability_registry;
mod errors;
mod gemini;
mod gemini_family;
mod hosted_provider;
mod http_limits;
mod mock;
pub mod ollama;
mod openai;
mod openai_family;
mod openrouter;
pub mod platform;
mod prompt_builder;
mod provider;
mod provider_send;
mod response_parser;
pub mod response_schema;
mod settings_change;
mod structured_user_input;
mod tool_loop;
pub mod ui_knowledge;

pub use access_mode::{resolve_access_presentation, AiAccessPresentation, DisclosurePolicy};

pub use anthropic::AnthropicProvider;
pub use auto::{chat_with_auto, peek_assistant_message};
pub use capability_registry::{
    hash_tool_results, seal_tool_result_envelope, tool_result_display_summary, AgentCapability,
    ToolCallRequest, ToolCallResult,
};
pub use errors::AiError;
pub use gemini::GeminiProvider;
pub use mock::MockAiProvider;
pub use ollama::{ollama_server_ready, OllamaProvider};
pub use openai::OpenAiProvider;
pub use openrouter::OpenRouterProvider;
pub use prompt_builder::{build_agent_prompt_with_references, PROMPT_VERSION};
#[allow(unused_imports)] // Stream types are the public foundation for Phase 7 wiring.
pub use provider::{
    AgentMessage, AgentRequest, AgentResponse, AiProvider, ProviderHealth, ProviderStreamEvent,
    ProviderStreamRx, ProviderStreamTx, UsageMetadata,
};
pub use provider_send::validate_provider_send;
pub use response_parser::{parse_agent_response, ParsedAgentResponse};
pub use response_schema::{
    layout_type_string, normalize_layout, validate_layout, ActionDefinition, ResponseType,
    SourceCitation, ToolAction, ToolChangePayload, ToolComponent, ToolDefinition,
    MAX_ACTIONS_PER_COMPONENT, VALID_LAYOUT_TYPES,
};
pub use settings_change::{
    is_allowed_setting_key, normalize_hex_or_none, normalize_setting_kv, parse_wallpaper_setting,
    SettingsChangePayload, WallpaperConfig,
};
#[allow(unused_imports)] // Public surface for StructuredUserInput (RC3.3 Phase 8).
pub use structured_user_input::{
    adopt_stored_structured_input, build_user_parts, flatten_parts_for_provider,
    has_trusted_structured_input, image_part_from_authorized_bytes, is_provider_image_mime,
    provider_text_summary, seal_from_ledger_payload, seal_local_user_submission,
    structured_metadata_value, structured_trust_from_text, text_contains_structured_marker,
    validate_provider_image_bytes, AgentContentPart, AgentRole, InstructionEligibility,
    StructuredUserInput, StructuredUserInputSubmission, TrustClass, MAX_PROVIDER_IMAGE_BYTES,
    MAX_PROVIDER_IMAGE_PIXELS,
};
pub use tool_loop::{project_context_for_prompt, ToolLoop, ToolLoopContext};

use crate::config::AppConfig;
use std::sync::Arc;

/// Create an AI provider from config. Uses mock when `AI_PROVIDER=mock`.
pub fn create_provider(config: &AppConfig) -> Result<Arc<dyn AiProvider>, AiError> {
    create_provider_with_model(config, None)
}

/// Create a provider from trusted access resolution (preferred for chat/runtime).
pub fn create_provider_for_access(
    access: &crate::credentials::ResolvedAiAccess,
    model_override: Option<&str>,
) -> Result<Arc<dyn AiProvider>, AiError> {
    create_provider_with_route(
        &access.credentials.to_app_config(),
        model_override,
        access.route,
        Some(access),
    )
}

/// Create a provider, optionally overriding the model id (`None` / `"auto"` → config default).
pub fn create_provider_with_model(
    config: &AppConfig,
    model_override: Option<&str>,
) -> Result<Arc<dyn AiProvider>, AiError> {
    let route = if config.provider.eq_ignore_ascii_case("mock") {
        crate::credentials::AiAccessRoute::Mock
    } else if config.has_api_key() {
        crate::credentials::AiAccessRoute::UserByok
    } else if crate::credentials::is_authless_local_provider(&config.provider) {
        crate::credentials::AiAccessRoute::LocalAuthless
    } else if config.provider == "coreside_hosted" {
        crate::credentials::AiAccessRoute::CoresideHosted
    } else {
        crate::credentials::AiAccessRoute::Unavailable
    };
    create_provider_with_route(config, model_override, route, None)
}

fn create_provider_with_route(
    config: &AppConfig,
    model_override: Option<&str>,
    route: crate::credentials::AiAccessRoute,
    access: Option<&crate::credentials::ResolvedAiAccess>,
) -> Result<Arc<dyn AiProvider>, AiError> {
    let model = resolve_runtime_model(config, model_override);
    let provider = config.provider.to_lowercase();

    if route == crate::credentials::AiAccessRoute::Mock || provider == "mock" {
        return Ok(Arc::new(MockAiProvider::new()));
    }

    match route {
        crate::credentials::AiAccessRoute::LocalAuthless => {
            create_local_or_compatible_provider(config, &provider, &model, "")
        }
        crate::credentials::AiAccessRoute::CoresideHosted => hosted_provider::try_from_session()?
            .ok_or_else(|| {
                AiError::NotConfigured(
                    "Coreside AI session is not available. Sign in to use hosted AI.".into(),
                )
            }),
        crate::credentials::AiAccessRoute::UserByok
        | crate::credentials::AiAccessRoute::DeveloperEnv => {
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
            create_local_or_compatible_provider(config, &provider, &model, &key)
        }
        crate::credentials::AiAccessRoute::Unavailable => {
            // Resolved access that is Unavailable must never fall through to Hosted
            // (or any other trust class). Hosted requires AiAccessRoute::CoresideHosted.
            if access.is_some() {
                let connection_scoped = access
                    .map(|a| {
                        a.credentials.source == "connection"
                            || a.credentials.active_connection_id.is_some()
                            || a.credentials.missing_connection_secret
                    })
                    .unwrap_or(false);
                return Err(AiError::NotConfigured(if connection_scoped {
                    "Your active AI connection is not ready. Re-enter its API key in Settings."
                        .into()
                } else {
                    "AI access is unavailable for the selected route.".into()
                }));
            }
            // Legacy create_provider_with_model (no ResolvedAiAccess): authless Local only.
            if !config.has_api_key() {
                if crate::credentials::is_authless_local_provider(&provider) {
                    return create_local_or_compatible_provider(config, &provider, &model, "");
                }
                if let Some(desc) = platform::descriptor_by_id(&provider) {
                    if !desc.default_auth_mode.requires_secret() && desc.local {
                        return create_local_or_compatible_provider(config, &provider, &model, "");
                    }
                }
            }
            Err(AiError::NotConfigured(
                "AI API key not configured. Connect a provider in Settings or set a key in .env."
                    .into(),
            ))
        }
        crate::credentials::AiAccessRoute::Mock => Ok(Arc::new(MockAiProvider::new())),
    }
}

fn create_local_or_compatible_provider(
    config: &AppConfig,
    provider: &str,
    model: &str,
    key: &str,
) -> Result<Arc<dyn AiProvider>, AiError> {
    let Some(desc) = platform::descriptor_by_id(provider) else {
        return Err(AiError::Validation(format!(
            "Unsupported AI provider: {provider}. Choose a connection from AI connections."
        )));
    };
    create_provider_for_descriptor(config, &desc, model, key)
}

/// Protocol-family dispatch (RC3.6). Adapters are chosen from descriptor metadata
/// (`protocol_family`, `endpoint_class`, preset id) — not ad-hoc provider string fallthrough.
fn create_provider_for_descriptor(
    config: &AppConfig,
    desc: &platform::ProviderDescriptor,
    model: &str,
    key: &str,
) -> Result<Arc<dyn AiProvider>, AiError> {
    match desc.protocol_family {
        platform::ProtocolFamily::OllamaNativeChat => {
            let origin = if config.base_url.trim().is_empty() {
                ollama::normalize_ollama_origin("")
            } else {
                ollama::normalize_ollama_origin(&config.base_url)
            };
            Ok(Arc::new(OllamaProvider::new(origin, model.to_string())))
        }
        platform::ProtocolFamily::GeminiGenerateContent => Ok(Arc::new(GeminiProvider::new(
            key.to_string(),
            model.to_string(),
            config.base_url.clone(),
        ))),
        platform::ProtocolFamily::AnthropicMessages => Ok(Arc::new(AnthropicProvider::new(
            key.to_string(),
            model.to_string(),
            config.base_url.clone(),
        ))),
        platform::ProtocolFamily::MockDeterministic => Ok(Arc::new(MockAiProvider::new())),
        platform::ProtocolFamily::OpenAiChatCompletions
        | platform::ProtocolFamily::OpenAiResponses => {
            if desc.id == "openrouter" {
                return Ok(Arc::new(OpenRouterProvider::new(
                    key.to_string(),
                    model.to_string(),
                    config.base_url.clone(),
                )));
            }
            if desc.endpoint_class == platform::EndpointClass::UserConfiguredRemoteCompatible {
                if config.base_url.trim().is_empty() {
                    return Err(AiError::Validation(
                        "Custom OpenAI-compatible providers require a base URL".into(),
                    ));
                }
                return Ok(Arc::new(OpenAiProvider::new_compatible(
                    key.to_string(),
                    model.to_string(),
                    config.base_url.clone(),
                )));
            }
            Ok(Arc::new(OpenAiProvider::with_identity(
                key.to_string(),
                model.to_string(),
                config.base_url.clone(),
                desc.id,
                desc.display_name,
            )))
        }
        platform::ProtocolFamily::CoresideHostedGateway => Err(AiError::Validation(
            "Coreside hosted gateway is resolved via hosted session, not BYOK factory".into(),
        )),
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

/// Curated model options per provider preset. Unknown providers do not fall through to Gemini.
fn extend_model_options_for_provider(options: &mut Vec<ModelOption>, provider: &str) {
    let key = provider.trim().to_lowercase();
    let key = match key.as_str() {
        "claude" => "anthropic",
        "moonshot" => "kimi",
        other => other,
    };
    match key {
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
        "anthropic" => {
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
                    id: "google/gemini-3.8-flash".into(),
                    label: "Gemini 3.8 Flash".into(),
                    description: Some("Recommended — state-of-the-art Flash GA via OpenRouter".into()),
                },
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
        "kimi" => {
            options.extend([
                ModelOption {
                    id: "kimi-k3".into(),
                    label: "Kimi K3".into(),
                    description: Some("Moonshot flagship".into()),
                },
                ModelOption {
                    id: "kimi-k2.5".into(),
                    label: "Kimi K2.5".into(),
                    description: Some("Prior generation".into()),
                },
            ]);
        }
        "mistral" => {
            options.extend([
                ModelOption {
                    id: "mistral-large-latest".into(),
                    label: "Mistral Large".into(),
                    description: Some("Highest capability".into()),
                },
                ModelOption {
                    id: "mistral-small-latest".into(),
                    label: "Mistral Small".into(),
                    description: Some("Fast and affordable".into()),
                },
            ]);
        }
        "gemini" => {
            options.extend([
                ModelOption {
                    id: "gemini-3.8-flash".into(),
                    label: "Gemini 3.8 Flash".into(),
                    description: Some("Recommended — state-of-the-art Flash GA".into()),
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
        "mock" => {
            options.push(ModelOption {
                id: "mock-fixture".into(),
                label: "Mock fixture".into(),
                description: Some("Deterministic offline responses".into()),
            });
        }
        "ollama" | "lmstudio" | "vllm" | "llama_cpp" => {
            if let Some(desc) = platform::descriptor_by_id(key) {
                if let Some(hint) = desc.default_model_hint {
                    options.push(ModelOption {
                        id: hint.to_string(),
                        label: hint.to_string(),
                        description: Some("Local server model id".into()),
                    });
                }
            }
        }
        _ => {
            if let Some(desc) = platform::descriptor_by_id(key) {
                if let Some(hint) = desc.default_model_hint.filter(|h| !h.is_empty()) {
                    options.push(ModelOption {
                        id: hint.to_string(),
                        label: hint.to_string(),
                        description: Some(format!("Default for {}", desc.display_name)),
                    });
                }
            }
        }
    }
}

/// Curated model catalog for the chat picker (Auto + specific models).
pub fn model_catalog(config: &AppConfig, selected: &str) -> ModelCatalog {
    let mut options = vec![ModelOption {
        id: "auto".into(),
        label: "Auto".into(),
        description: Some("Picks the best available model automatically".into()),
    }];

    extend_model_options_for_provider(&mut options, &config.provider);

    // Unknown provider without a descriptor: never inject Gemini defaults.
    if options.len() == 1
        && platform::descriptor_by_id(&config.provider).is_none()
        && !config.model.trim().is_empty()
        && !config.model.eq_ignore_ascii_case("auto")
    {
        options.push(ModelOption {
            id: config.model.clone(),
            label: config.model.clone(),
            description: Some("Configured model".into()),
        });
    }

    let selected = {
        let trimmed = selected.trim();
        if trimmed.is_empty() {
            "auto".to_string()
        } else if trimmed == "auto" {
            "auto".to_string()
        } else if options.iter().any(|o| o.id == trimmed) {
            trimmed.to_string()
        } else {
            // Stale model preference from a previous provider — fall back to auto
            // rather than silently sending an invalid model ID to the wrong provider.
            "auto".to_string()
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

#[cfg(test)]
mod provider_factory_tests {
    use super::*;
    use crate::config::{
        AppConfig, DEFAULT_ANTHROPIC_BASE_URL, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENROUTER_BASE_URL,
    };
    use crate::credentials::{AiAccessRoute, ResolvedAiAccess, ResolvedCredentials};

    fn ollama_authless_config() -> AppConfig {
        AppConfig {
            provider: "ollama".into(),
            api_key: None,
            model: "llama3.2".into(),
            base_url: "http://127.0.0.1:11434".into(),
            log_level: "info".into(),
            env_path: None,
        }
    }

    fn ollama_authless_access() -> ResolvedAiAccess {
        ResolvedAiAccess {
            credentials: ResolvedCredentials {
                provider: "ollama".into(),
                api_key: None,
                model: "llama3.2".into(),
                base_url: "http://127.0.0.1:11434".into(),
                source: "connection".into(),
                active_connection_id: Some("ollama-1".into()),
                env_path: None,
                missing_connection_secret: false,
            },
            route: AiAccessRoute::LocalAuthless,
        }
    }

    #[test]
    fn create_provider_with_model_ollama_authless_uses_native_adapter() {
        let provider = create_provider_with_model(&ollama_authless_config(), None)
            .expect("ollama authless provider");
        assert_eq!(provider.provider_id(), "ollama");
        assert_eq!(provider.display_name(), "Ollama");
        assert_ne!(provider.provider_id(), "coreside_hosted");
    }

    #[test]
    fn protocol_family_dispatch_ollama_native() {
        let config = ollama_authless_config();
        let desc = platform::descriptor_by_id("ollama").expect("ollama descriptor");
        let provider = create_provider_for_descriptor(&config, &desc, "llama3.2", "")
            .expect("ollama via protocol family");
        assert_eq!(provider.provider_id(), "ollama");
    }

    #[test]
    fn protocol_family_dispatch_gemini_generate_content() {
        let config = AppConfig {
            provider: "gemini".into(),
            api_key: Some("test-gemini-key".into()),
            model: "gemini-2.5-flash".into(),
            base_url: String::new(),
            log_level: "info".into(),
            env_path: None,
        };
        let desc = platform::descriptor_by_id("gemini").expect("gemini descriptor");
        let provider =
            create_provider_for_descriptor(&config, &desc, "gemini-2.5-flash", "test-gemini-key")
                .expect("gemini via protocol family");
        assert_eq!(provider.provider_id(), "gemini");
        assert_eq!(provider.display_name(), "Google Gemini");
    }

    #[test]
    fn protocol_family_dispatch_openai_chat_completions() {
        let config = AppConfig {
            provider: "openai".into(),
            api_key: Some("sk-test".into()),
            model: "gpt-4.1-mini".into(),
            base_url: DEFAULT_OPENAI_BASE_URL.to_string(),
            log_level: "info".into(),
            env_path: None,
        };
        let desc = platform::descriptor_by_id("openai").expect("openai descriptor");
        let provider = create_provider_for_descriptor(&config, &desc, "gpt-4.1-mini", "sk-test")
            .expect("openai");
        assert_eq!(provider.provider_id(), "openai");
    }

    #[test]
    fn protocol_family_dispatch_anthropic_messages() {
        let config = AppConfig {
            provider: "anthropic".into(),
            api_key: Some("sk-ant".into()),
            model: "claude-sonnet-4-5".into(),
            base_url: DEFAULT_ANTHROPIC_BASE_URL.to_string(),
            log_level: "info".into(),
            env_path: None,
        };
        let desc = platform::descriptor_by_id("anthropic").expect("anthropic descriptor");
        let provider =
            create_provider_for_descriptor(&config, &desc, "claude-sonnet-4-5", "sk-ant")
                .expect("anthropic");
        assert_eq!(provider.provider_id(), "anthropic");
    }

    #[test]
    fn protocol_family_dispatch_openrouter_uses_dedicated_adapter() {
        let config = AppConfig {
            provider: "openrouter".into(),
            api_key: Some("or-test".into()),
            model: "google/gemini-2.5-flash".into(),
            base_url: DEFAULT_OPENROUTER_BASE_URL.to_string(),
            log_level: "info".into(),
            env_path: None,
        };
        let desc = platform::descriptor_by_id("openrouter").expect("openrouter descriptor");
        let provider =
            create_provider_for_descriptor(&config, &desc, "google/gemini-2.5-flash", "or-test")
                .expect("openrouter");
        assert_eq!(provider.provider_id(), "openrouter");
    }

    #[test]
    fn protocol_family_dispatch_mistral_uses_openai_adapter() {
        let config = AppConfig {
            provider: "mistral".into(),
            api_key: Some("m-test".into()),
            model: "mistral-large-latest".into(),
            base_url: "https://api.mistral.ai/v1".into(),
            log_level: "info".into(),
            env_path: None,
        };
        let desc = platform::descriptor_by_id("mistral").expect("mistral descriptor");
        let provider =
            create_provider_for_descriptor(&config, &desc, "mistral-large-latest", "m-test")
                .expect("mistral");
        assert_eq!(provider.provider_id(), "mistral");
    }

    #[test]
    fn protocol_family_dispatch_kimi_uses_openai_adapter() {
        let config = AppConfig {
            provider: "kimi".into(),
            api_key: Some("k-test".into()),
            model: "kimi-k3".into(),
            base_url: "https://api.moonshot.ai/v1".into(),
            log_level: "info".into(),
            env_path: None,
        };
        let desc = platform::descriptor_by_id("kimi").expect("kimi descriptor");
        let provider =
            create_provider_for_descriptor(&config, &desc, "kimi-k3", "k-test").expect("kimi");
        assert_eq!(provider.provider_id(), "kimi");
    }

    #[test]
    fn model_catalog_mistral_and_ollama_avoid_gemini_slugs() {
        let mistral = model_catalog(
            &AppConfig {
                provider: "mistral".into(),
                api_key: Some("m".into()),
                model: "mistral-large-latest".into(),
                base_url: String::new(),
                log_level: "info".into(),
                env_path: None,
            },
            "auto",
        );
        assert!(mistral
            .options
            .iter()
            .any(|o| o.id == "mistral-large-latest"));
        assert!(!mistral.options.iter().any(|o| o.id.contains("gemini")));

        let ollama = model_catalog(
            &AppConfig {
                provider: "ollama".into(),
                api_key: None,
                model: "llama3.2".into(),
                base_url: "http://127.0.0.1:11434".into(),
                log_level: "info".into(),
                env_path: None,
            },
            "auto",
        );
        assert_eq!(ollama.options.len(), 1);
        assert!(ollama.options.iter().any(|o| o.id == "auto"));
        assert!(!ollama.options.iter().any(|o| o.id.contains("gemini")));
    }

    #[test]
    fn model_catalog_kimi_does_not_use_gemini_defaults() {
        let config = AppConfig {
            provider: "kimi".into(),
            api_key: Some("k".into()),
            model: "kimi-k3".into(),
            base_url: String::new(),
            log_level: "info".into(),
            env_path: None,
        };
        let catalog = model_catalog(&config, "auto");
        assert!(catalog.options.iter().any(|o| o.id == "kimi-k3"));
        assert!(!catalog.options.iter().any(|o| o.id == "gemini-3.5-flash"));
    }

    #[test]
    fn model_catalog_unknown_provider_has_no_gemini_slugs() {
        let config = AppConfig {
            provider: "not-a-provider".into(),
            api_key: None,
            model: "custom-model".into(),
            base_url: String::new(),
            log_level: "info".into(),
            env_path: None,
        };
        let catalog = model_catalog(&config, "auto");
        assert_eq!(catalog.options.len(), 2);
        assert!(catalog.options.iter().any(|o| o.id == "auto"));
        assert!(catalog.options.iter().any(|o| o.id == "custom-model"));
        assert!(!catalog.options.iter().any(|o| o.id.contains("gemini")));
    }

    #[test]
    fn unknown_provider_without_descriptor_returns_validation() {
        let config = ollama_authless_config();
        let result =
            create_local_or_compatible_provider(&config, "not-a-real-provider", "some-model", "");
        match result {
            Err(AiError::Validation(msg)) => {
                assert!(msg.contains("Unsupported AI provider"));
                assert!(msg.contains("not-a-real-provider"));
            }
            other => panic!("expected validation error, got {:?}", other.err()),
        }
    }

    #[test]
    fn create_provider_for_access_local_authless_never_hosted() {
        let provider = create_provider_for_access(&ollama_authless_access(), None)
            .expect("local authless provider");
        assert_eq!(provider.provider_id(), "ollama");
        assert_ne!(provider.provider_id(), "coreside_hosted");
    }

    #[test]
    fn create_provider_for_access_compatible_authless_never_hosted() {
        let access = ResolvedAiAccess {
            credentials: ResolvedCredentials {
                provider: "compatible".into(),
                api_key: None,
                model: "gpt-4o-mini".into(),
                base_url: "http://127.0.0.1:8080/v1".into(),
                source: "connection".into(),
                active_connection_id: Some("compat-1".into()),
                env_path: None,
                missing_connection_secret: false,
            },
            route: AiAccessRoute::LocalAuthless,
        };
        let provider = create_provider_for_access(&access, None).expect("compatible local");
        assert_eq!(provider.provider_id(), "compatible");
        assert_ne!(provider.provider_id(), "coreside_hosted");
    }

    #[test]
    fn hosted_route_failure_does_not_fall_back_to_local() {
        let access = ResolvedAiAccess {
            credentials: ResolvedCredentials {
                provider: "coreside_hosted".into(),
                api_key: None,
                model: "auto".into(),
                base_url: String::new(),
                source: "none".into(),
                active_connection_id: None,
                env_path: None,
                missing_connection_secret: false,
            },
            route: AiAccessRoute::CoresideHosted,
        };
        let result = create_provider_for_access(&access, None);
        match result {
            Err(err) => assert_eq!(err.code(), "not_configured"),
            Ok(provider) => panic!(
                "hosted route must error when session is unavailable, got {:?}",
                provider.provider_id()
            ),
        }
    }

    #[test]
    fn unavailable_connection_route_never_falls_back_to_hosted() {
        let access = ResolvedAiAccess {
            credentials: ResolvedCredentials {
                provider: "openai".into(),
                api_key: None,
                model: "gpt-4.1-mini".into(),
                base_url: String::new(),
                source: "connection".into(),
                active_connection_id: Some("byok-1".into()),
                env_path: None,
                missing_connection_secret: true,
            },
            route: AiAccessRoute::Unavailable,
        };
        let result = create_provider_for_access(&access, None);
        match result {
            Err(err) => {
                assert_eq!(err.code(), "not_configured");
                assert!(err.to_string().contains("active AI connection"));
            }
            Ok(provider) => panic!(
                "broken BYOK must not fall back to hosted, got {:?}",
                provider.provider_id()
            ),
        }
    }

    #[test]
    fn unavailable_without_connection_source_never_falls_back_to_hosted() {
        let access = ResolvedAiAccess {
            credentials: ResolvedCredentials {
                provider: "openai".into(),
                api_key: None,
                model: "gpt-4.1-mini".into(),
                base_url: String::new(),
                source: "none".into(),
                active_connection_id: None,
                env_path: None,
                missing_connection_secret: false,
            },
            route: AiAccessRoute::Unavailable,
        };
        let result = create_provider_for_access(&access, None);
        match result {
            Err(err) => assert_eq!(err.code(), "not_configured"),
            Ok(provider) => panic!(
                "Unavailable must not fall back to hosted, got {:?}",
                provider.provider_id()
            ),
        }
    }
}
