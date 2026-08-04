//! Shared provider-platform taxonomies.

use serde::{Deserialize, Serialize};

/// Wire protocol family used by an adapter. Not every OpenAI-shaped API is
/// "openai_chat_completions" — presets may still reuse that adapter when the
/// official API is documented as Chat Completions compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolFamily {
    OpenAiResponses,
    OpenAiChatCompletions,
    AnthropicMessages,
    GeminiGenerateContent,
    OllamaNativeChat,
    CoresideHostedGateway,
    MockDeterministic,
}

impl ProtocolFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai_responses",
            Self::OpenAiChatCompletions => "openai_chat_completions",
            Self::AnthropicMessages => "anthropic_messages",
            Self::GeminiGenerateContent => "gemini_generate_content",
            Self::OllamaNativeChat => "ollama_native_chat",
            Self::CoresideHostedGateway => "coreside_hosted_gateway",
            Self::MockDeterministic => "mock_deterministic",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "openai_responses" => Some(Self::OpenAiResponses),
            "openai_chat_completions" => Some(Self::OpenAiChatCompletions),
            "anthropic_messages" => Some(Self::AnthropicMessages),
            "gemini_generate_content" => Some(Self::GeminiGenerateContent),
            "ollama_native_chat" => Some(Self::OllamaNativeChat),
            "coreside_hosted_gateway" => Some(Self::CoresideHostedGateway),
            "mock_deterministic" => Some(Self::MockDeterministic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    ApiKeyBearer,
    ProviderApiKeyHeader,
    LocalAuthless,
    UserStaticToken,
    HostedCoresideSession,
}

impl AuthMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ApiKeyBearer => "api_key_bearer",
            Self::ProviderApiKeyHeader => "provider_api_key_header",
            Self::LocalAuthless => "local_authless",
            Self::UserStaticToken => "user_static_token",
            Self::HostedCoresideSession => "hosted_coreside_session",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "api_key_bearer" => Some(Self::ApiKeyBearer),
            "provider_api_key_header" => Some(Self::ProviderApiKeyHeader),
            "local_authless" => Some(Self::LocalAuthless),
            "user_static_token" => Some(Self::UserStaticToken),
            "hosted_coreside_session" => Some(Self::HostedCoresideSession),
            _ => None,
        }
    }

    pub fn requires_secret(self) -> bool {
        !matches!(self, Self::LocalAuthless)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointClass {
    FixedTrustedRemote,
    VettedRemoteProviderPreset,
    UserConfiguredRemoteCompatible,
    LoopbackLocal,
    PrivateLanLocal,
    HostedCoresideGateway,
}

impl EndpointClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixedTrustedRemote => "fixed_trusted_remote",
            Self::VettedRemoteProviderPreset => "vetted_remote_provider_preset",
            Self::UserConfiguredRemoteCompatible => "user_configured_remote_compatible",
            Self::LoopbackLocal => "loopback_local",
            Self::PrivateLanLocal => "private_lan_local",
            Self::HostedCoresideGateway => "hosted_coreside_gateway",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "fixed_trusted_remote" => Some(Self::FixedTrustedRemote),
            "vetted_remote_provider_preset" => Some(Self::VettedRemoteProviderPreset),
            "user_configured_remote_compatible" => Some(Self::UserConfiguredRemoteCompatible),
            "loopback_local" => Some(Self::LoopbackLocal),
            "private_lan_local" => Some(Self::PrivateLanLocal),
            "hosted_coreside_gateway" => Some(Self::HostedCoresideGateway),
            _ => None,
        }
    }

    pub fn is_local(self) -> bool {
        matches!(self, Self::LoopbackLocal | Self::PrivateLanLocal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityConfidence {
    OfficialProviderMetadata,
    VettedCoresidePreset,
    SuccessfulConformanceProbe,
    ModelCatalogMetadata,
    UserOverride,
    Unknown,
}

impl CapabilityConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OfficialProviderMetadata => "official_provider_metadata",
            Self::VettedCoresidePreset => "vetted_coreside_preset",
            Self::SuccessfulConformanceProbe => "successful_conformance_probe",
            Self::ModelCatalogMetadata => "model_catalog_metadata",
            Self::UserOverride => "user_override",
            Self::Unknown => "unknown",
        }
    }

    /// Generated-app eligibility must not rely on low-confidence claims.
    pub fn allows_generated_apps(self) -> bool {
        matches!(
            self,
            Self::OfficialProviderMetadata
                | Self::VettedCoresidePreset
                | Self::SuccessfulConformanceProbe
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityFlag {
    TextInput,
    TextStreaming,
    StructuredJson,
    StrictJsonSchema,
    FunctionToolCalls,
    ParallelToolCalls,
    StreamingToolArguments,
    NativeToolResults,
    ImageInput,
    DocumentInput,
    ModelListing,
    UsageReporting,
    Cancellation,
    ProgressiveCoresideOperations,
    GeneratedAppEligible,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProfile {
    pub flags: Vec<CapabilityFlag>,
    pub confidence: CapabilityConfidence,
}

impl CapabilityProfile {
    pub fn empty_unknown() -> Self {
        Self {
            flags: vec![CapabilityFlag::TextInput],
            confidence: CapabilityConfidence::Unknown,
        }
    }

    pub fn supports(&self, flag: CapabilityFlag) -> bool {
        self.flags.contains(&flag)
    }

    /// Restrictive merge: intersection of flags; lowest confidence wins.
    pub fn intersect(&self, other: &Self) -> Self {
        let flags: Vec<_> = self
            .flags
            .iter()
            .copied()
            .filter(|f| other.flags.contains(f))
            .collect();
        let confidence = min_confidence(self.confidence, other.confidence);
        Self {
            flags,
            confidence,
        }
    }
}

fn min_confidence(a: CapabilityConfidence, b: CapabilityConfidence) -> CapabilityConfidence {
    use CapabilityConfidence::*;
    let rank = |c: CapabilityConfidence| match c {
        SuccessfulConformanceProbe => 5,
        OfficialProviderMetadata => 4,
        VettedCoresidePreset => 3,
        ModelCatalogMetadata => 2,
        UserOverride => 1,
        Unknown => 0,
    };
    if rank(a) <= rank(b) {
        a
    } else {
        b
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionHealth {
    Unconfigured,
    Checking,
    Ready,
    ReadyWithLimitations,
    AuthenticationFailed,
    EndpointUnavailable,
    ModelUnavailable,
    ProtocolMismatch,
    CapabilityMismatch,
    RateLimited,
    Disabled,
    NeedsAttention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelDiscoveryStrategy {
    OfficialModelList,
    LocalServerTags,
    HostedCatalog,
    VettedFallback,
    ManualModelId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authless_does_not_require_secret() {
        assert!(!AuthMode::LocalAuthless.requires_secret());
        assert!(AuthMode::ApiKeyBearer.requires_secret());
    }

    #[test]
    fn capability_intersect_is_restrictive() {
        let a = CapabilityProfile {
            flags: vec![
                CapabilityFlag::TextInput,
                CapabilityFlag::ImageInput,
                CapabilityFlag::GeneratedAppEligible,
            ],
            confidence: CapabilityConfidence::VettedCoresidePreset,
        };
        let b = CapabilityProfile {
            flags: vec![CapabilityFlag::TextInput, CapabilityFlag::ImageInput],
            confidence: CapabilityConfidence::Unknown,
        };
        let m = a.intersect(&b);
        assert!(m.supports(CapabilityFlag::TextInput));
        assert!(m.supports(CapabilityFlag::ImageInput));
        assert!(!m.supports(CapabilityFlag::GeneratedAppEligible));
        assert_eq!(m.confidence, CapabilityConfidence::Unknown);
        assert!(!m.confidence.allows_generated_apps());
    }

    #[test]
    fn capability_intersect_keeps_only_shared_flags() {
        let a = CapabilityProfile {
            flags: vec![CapabilityFlag::TextInput, CapabilityFlag::ImageInput],
            confidence: CapabilityConfidence::OfficialProviderMetadata,
        };
        let b = CapabilityProfile {
            flags: vec![CapabilityFlag::TextInput, CapabilityFlag::TextStreaming],
            confidence: CapabilityConfidence::VettedCoresidePreset,
        };
        let m = a.intersect(&b);
        assert!(m.supports(CapabilityFlag::TextInput));
        assert!(!m.supports(CapabilityFlag::ImageInput));
        assert!(!m.supports(CapabilityFlag::TextStreaming));
        assert_eq!(m.confidence, CapabilityConfidence::VettedCoresidePreset);
    }
}
