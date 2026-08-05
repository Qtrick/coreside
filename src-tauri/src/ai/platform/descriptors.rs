//! Built-in provider descriptors / presets (RC3.5).
//!
//! Official endpoints and auth come from researched provider docs, not memory.
//! Capability flags here are vetted-preset confidence only until conformance
//! probes succeed for a connection/model.

use super::types::{
    AuthMode, CapabilityConfidence, CapabilityFlag, CapabilityProfile, EndpointClass,
    ModelDiscoveryStrategy, ProtocolFamily,
};
use serde::Serialize;

pub const PROVIDER_PRESET_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub protocol_family: ProtocolFamily,
    pub default_endpoint: Option<&'static str>,
    pub auth_modes: &'static [AuthMode],
    pub default_auth_mode: AuthMode,
    pub endpoint_class: EndpointClass,
    pub allows_endpoint_override: bool,
    pub model_discovery: ModelDiscoveryStrategy,
    pub default_model_hint: Option<&'static str>,
    pub docs_url: Option<&'static str>,
    pub capability_profile: CapabilityProfile,
    pub local: bool,
    pub experimental: bool,
    pub consumer_visible: bool,
    pub preset_version: &'static str,
}

fn chat_streaming_tools() -> CapabilityProfile {
    CapabilityProfile {
        flags: vec![
            CapabilityFlag::TextInput,
            CapabilityFlag::TextStreaming,
            CapabilityFlag::FunctionToolCalls,
            CapabilityFlag::NativeToolResults,
            CapabilityFlag::ModelListing,
            CapabilityFlag::UsageReporting,
            CapabilityFlag::Cancellation,
        ],
        confidence: CapabilityConfidence::VettedCoresidePreset,
    }
}

fn chat_streaming_tools_images_apps() -> CapabilityProfile {
    let mut p = chat_streaming_tools();
    p.flags.push(CapabilityFlag::ImageInput);
    p.flags.push(CapabilityFlag::StructuredJson);
    p.flags.push(CapabilityFlag::ProgressiveCoresideOperations);
    // Generated-app eligibility stays off until conformance proves reliable ops.
    p
}

fn local_chat() -> CapabilityProfile {
    CapabilityProfile {
        flags: vec![
            CapabilityFlag::TextInput,
            CapabilityFlag::TextStreaming,
            CapabilityFlag::ImageInput,
            CapabilityFlag::ModelListing,
            CapabilityFlag::Cancellation,
        ],
        confidence: CapabilityConfidence::VettedCoresidePreset,
    }
}

/// Runtime descriptor table. Capability flags are Vec, so this is not `const`.
pub fn list_descriptors() -> Vec<ProviderDescriptor> {
    vec![
        ProviderDescriptor {
            id: "gemini",
            display_name: "Google Gemini",
            protocol_family: ProtocolFamily::GeminiGenerateContent,
            default_endpoint: Some("https://generativelanguage.googleapis.com/v1beta"),
            auth_modes: &[AuthMode::ProviderApiKeyHeader],
            default_auth_mode: AuthMode::ProviderApiKeyHeader,
            endpoint_class: EndpointClass::FixedTrustedRemote,
            allows_endpoint_override: false,
            model_discovery: ModelDiscoveryStrategy::OfficialModelList,
            default_model_hint: Some(crate::config::DEFAULT_GEMINI_MODEL),
            docs_url: Some("https://aistudio.google.com/apikey"),
            capability_profile: chat_streaming_tools_images_apps(),
            local: false,
            experimental: false,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        ProviderDescriptor {
            id: "openai",
            display_name: "OpenAI",
            protocol_family: ProtocolFamily::OpenAiChatCompletions,
            default_endpoint: Some(crate::config::DEFAULT_OPENAI_BASE_URL),
            auth_modes: &[AuthMode::ApiKeyBearer],
            default_auth_mode: AuthMode::ApiKeyBearer,
            endpoint_class: EndpointClass::FixedTrustedRemote,
            allows_endpoint_override: false,
            model_discovery: ModelDiscoveryStrategy::OfficialModelList,
            default_model_hint: Some(crate::config::DEFAULT_OPENAI_MODEL),
            docs_url: Some("https://platform.openai.com/api-keys"),
            capability_profile: chat_streaming_tools_images_apps(),
            local: false,
            experimental: false,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        ProviderDescriptor {
            id: "anthropic",
            display_name: "Anthropic",
            protocol_family: ProtocolFamily::AnthropicMessages,
            default_endpoint: Some(crate::config::DEFAULT_ANTHROPIC_BASE_URL),
            auth_modes: &[AuthMode::ProviderApiKeyHeader],
            default_auth_mode: AuthMode::ProviderApiKeyHeader,
            endpoint_class: EndpointClass::FixedTrustedRemote,
            allows_endpoint_override: false,
            model_discovery: ModelDiscoveryStrategy::ManualModelId,
            default_model_hint: Some(crate::config::DEFAULT_ANTHROPIC_MODEL),
            docs_url: Some("https://console.anthropic.com/settings/keys"),
            capability_profile: chat_streaming_tools_images_apps(),
            local: false,
            experimental: false,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        ProviderDescriptor {
            id: "openrouter",
            display_name: "OpenRouter",
            protocol_family: ProtocolFamily::OpenAiChatCompletions,
            default_endpoint: Some(crate::config::DEFAULT_OPENROUTER_BASE_URL),
            auth_modes: &[AuthMode::ApiKeyBearer],
            default_auth_mode: AuthMode::ApiKeyBearer,
            endpoint_class: EndpointClass::VettedRemoteProviderPreset,
            allows_endpoint_override: false,
            model_discovery: ModelDiscoveryStrategy::OfficialModelList,
            default_model_hint: Some(crate::config::DEFAULT_OPENROUTER_MODEL),
            docs_url: Some("https://openrouter.ai/keys"),
            capability_profile: chat_streaming_tools_images_apps(),
            local: false,
            experimental: false,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        // Official: https://platform.kimi.ai/docs/api/overview — base https://api.moonshot.ai/v1
        ProviderDescriptor {
            id: "kimi",
            display_name: "Kimi (Moonshot)",
            protocol_family: ProtocolFamily::OpenAiChatCompletions,
            default_endpoint: Some("https://api.moonshot.ai/v1"),
            auth_modes: &[AuthMode::ApiKeyBearer],
            default_auth_mode: AuthMode::ApiKeyBearer,
            endpoint_class: EndpointClass::VettedRemoteProviderPreset,
            allows_endpoint_override: false,
            model_discovery: ModelDiscoveryStrategy::OfficialModelList,
            default_model_hint: Some("kimi-k3"),
            docs_url: Some("https://platform.kimi.ai/docs/api/overview"),
            capability_profile: chat_streaming_tools_images_apps(),
            local: false,
            experimental: false,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        // Official: https://docs.mistral.ai — base https://api.mistral.ai/v1
        ProviderDescriptor {
            id: "mistral",
            display_name: "Mistral",
            protocol_family: ProtocolFamily::OpenAiChatCompletions,
            default_endpoint: Some("https://api.mistral.ai/v1"),
            auth_modes: &[AuthMode::ApiKeyBearer],
            default_auth_mode: AuthMode::ApiKeyBearer,
            endpoint_class: EndpointClass::VettedRemoteProviderPreset,
            allows_endpoint_override: false,
            model_discovery: ModelDiscoveryStrategy::OfficialModelList,
            default_model_hint: Some("mistral-large-latest"),
            docs_url: Some("https://docs.mistral.ai"),
            capability_profile: chat_streaming_tools_images_apps(),
            local: false,
            experimental: false,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        // Official local API: http://localhost:11434 — authless by default
        ProviderDescriptor {
            id: "ollama",
            display_name: "Ollama",
            protocol_family: ProtocolFamily::OllamaNativeChat,
            default_endpoint: Some("http://127.0.0.1:11434"),
            auth_modes: &[AuthMode::LocalAuthless],
            default_auth_mode: AuthMode::LocalAuthless,
            endpoint_class: EndpointClass::LoopbackLocal,
            allows_endpoint_override: true,
            model_discovery: ModelDiscoveryStrategy::LocalServerTags,
            default_model_hint: None,
            docs_url: Some("https://github.com/ollama/ollama/blob/main/docs/api.md"),
            capability_profile: local_chat(),
            local: true,
            experimental: false,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        // LM Studio OpenAI-compatible local server (default port 1234)
        ProviderDescriptor {
            id: "lmstudio",
            display_name: "LM Studio",
            protocol_family: ProtocolFamily::OpenAiChatCompletions,
            default_endpoint: Some("http://127.0.0.1:1234/v1"),
            auth_modes: &[AuthMode::LocalAuthless],
            default_auth_mode: AuthMode::LocalAuthless,
            endpoint_class: EndpointClass::LoopbackLocal,
            allows_endpoint_override: true,
            model_discovery: ModelDiscoveryStrategy::OfficialModelList,
            default_model_hint: None,
            docs_url: Some("https://lmstudio.ai/docs"),
            capability_profile: local_chat(),
            local: true,
            experimental: false,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        ProviderDescriptor {
            id: "vllm",
            display_name: "vLLM",
            protocol_family: ProtocolFamily::OpenAiChatCompletions,
            default_endpoint: Some("http://127.0.0.1:8000/v1"),
            auth_modes: &[AuthMode::LocalAuthless, AuthMode::ApiKeyBearer],
            default_auth_mode: AuthMode::LocalAuthless,
            endpoint_class: EndpointClass::LoopbackLocal,
            allows_endpoint_override: true,
            model_discovery: ModelDiscoveryStrategy::OfficialModelList,
            default_model_hint: None,
            docs_url: Some("https://docs.vllm.ai"),
            capability_profile: local_chat(),
            local: true,
            experimental: true,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        ProviderDescriptor {
            id: "llama_cpp",
            display_name: "llama.cpp",
            protocol_family: ProtocolFamily::OpenAiChatCompletions,
            default_endpoint: Some("http://127.0.0.1:8080/v1"),
            auth_modes: &[AuthMode::LocalAuthless],
            default_auth_mode: AuthMode::LocalAuthless,
            endpoint_class: EndpointClass::LoopbackLocal,
            allows_endpoint_override: true,
            model_discovery: ModelDiscoveryStrategy::OfficialModelList,
            default_model_hint: None,
            docs_url: Some("https://github.com/ggerganov/llama.cpp"),
            capability_profile: local_chat(),
            local: true,
            experimental: true,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        ProviderDescriptor {
            id: "compatible",
            display_name: "Custom compatible endpoint",
            protocol_family: ProtocolFamily::OpenAiChatCompletions,
            default_endpoint: None,
            auth_modes: &[AuthMode::ApiKeyBearer, AuthMode::UserStaticToken],
            default_auth_mode: AuthMode::ApiKeyBearer,
            endpoint_class: EndpointClass::UserConfiguredRemoteCompatible,
            allows_endpoint_override: true,
            model_discovery: ModelDiscoveryStrategy::ManualModelId,
            default_model_hint: Some("gpt-4.1-mini"),
            docs_url: None,
            capability_profile: CapabilityProfile {
                flags: vec![
                    CapabilityFlag::TextInput,
                    CapabilityFlag::TextStreaming,
                    CapabilityFlag::Cancellation,
                ],
                confidence: CapabilityConfidence::Unknown,
            },
            local: false,
            experimental: false,
            consumer_visible: true,
            preset_version: PROVIDER_PRESET_VERSION,
        },
        ProviderDescriptor {
            id: "mock",
            display_name: "Mock (deterministic)",
            protocol_family: ProtocolFamily::MockDeterministic,
            default_endpoint: None,
            auth_modes: &[AuthMode::LocalAuthless],
            default_auth_mode: AuthMode::LocalAuthless,
            endpoint_class: EndpointClass::LoopbackLocal,
            allows_endpoint_override: false,
            model_discovery: ModelDiscoveryStrategy::ManualModelId,
            default_model_hint: Some("mock"),
            docs_url: None,
            capability_profile: CapabilityProfile {
                flags: vec![
                    CapabilityFlag::TextInput,
                    CapabilityFlag::TextStreaming,
                    CapabilityFlag::StructuredJson,
                    CapabilityFlag::ProgressiveCoresideOperations,
                    CapabilityFlag::GeneratedAppEligible,
                    CapabilityFlag::Cancellation,
                ],
                confidence: CapabilityConfidence::SuccessfulConformanceProbe,
            },
            local: true,
            experimental: false,
            consumer_visible: false,
            preset_version: PROVIDER_PRESET_VERSION,
        },
    ]
}

pub fn list_consumer_descriptors() -> Vec<ProviderDescriptor> {
    list_descriptors()
        .into_iter()
        .filter(|d| d.consumer_visible)
        .collect()
}

pub fn descriptor_by_id(id: &str) -> Option<ProviderDescriptor> {
    let key = id.trim().to_lowercase();
    let key = match key.as_str() {
        "claude" => "anthropic".to_string(),
        "moonshot" => "kimi".to_string(),
        "llama.cpp" | "llamacpp" => "llama_cpp".to_string(),
        "lm-studio" | "lm_studio" => "lmstudio".to_string(),
        other => other.to_string(),
    };
    list_descriptors().into_iter().find(|d| d.id == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kimi_and_mistral_are_first_class_presets() {
        let kimi = descriptor_by_id("kimi").expect("kimi");
        assert_eq!(kimi.default_endpoint, Some("https://api.moonshot.ai/v1"));
        assert!(kimi.default_auth_mode.requires_secret());
        let mistral = descriptor_by_id("mistral").expect("mistral");
        assert_eq!(mistral.default_endpoint, Some("https://api.mistral.ai/v1"));
    }

    #[test]
    fn ollama_is_authless_loopback() {
        let ollama = descriptor_by_id("ollama").expect("ollama");
        assert_eq!(ollama.default_auth_mode, AuthMode::LocalAuthless);
        assert!(!ollama.default_auth_mode.requires_secret());
        assert_eq!(ollama.endpoint_class, EndpointClass::LoopbackLocal);
        assert_eq!(
            ollama.protocol_family,
            ProtocolFamily::OllamaNativeChat
        );
    }

    #[test]
    fn moonshot_alias_resolves_to_kimi() {
        let kimi = descriptor_by_id("moonshot").expect("moonshot alias");
        assert_eq!(kimi.id, "kimi");
        assert_eq!(kimi.default_endpoint, Some("https://api.moonshot.ai/v1"));
        assert!(kimi.default_auth_mode.requires_secret());
    }
}
