//! Authoritative AI access-mode resolution and consumer disclosure policy.
//! Frontend must not invent mode from scattered booleans.

use serde::Serialize;

/// Consumer-facing access modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAccessMode {
    CoresideHosted,
    UserByok,
    UserLocal,
    DeveloperEnvironment,
    Unavailable,
}

impl AiAccessMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CoresideHosted => "coreside_hosted",
            Self::UserByok => "user_byok",
            Self::UserLocal => "user_local",
            Self::DeveloperEnvironment => "developer_environment",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisclosurePolicy {
    pub show_provider_identity: bool,
    pub show_model_identity: bool,
    pub show_credential_source: bool,
    pub show_provider_catalog: bool,
    pub allow_model_selection: bool,
    pub allow_provider_management: bool,
    pub allow_connection_test: bool,
    pub allow_developer_details: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAccessPresentation {
    pub access_mode: AiAccessMode,
    pub available: bool,
    pub connection_state: String,
    pub consumer_display_name: String,
    pub user_facing_status: String,
    pub disclosure: DisclosurePolicy,
    /// Safe error category for consumer UI (never raw provider names).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_error_category: Option<String>,
}

/// Resolve access mode from trusted credential source + optional hosted adapter + developer mode.
///
/// - `source`: `connection` | `env` | `none` from `resolve_credentials`
/// - `provider`: resolved provider id (may be redacted later)
/// - `has_key`: whether a usable key exists
/// - `hosted_connected`: real hosted adapter entitlement (never fabricate)
/// - `developer_mode`: Settings developerMode (default false)
/// - `is_local_endpoint`: user-configured local inference (ollama etc.)
pub fn resolve_access_presentation(
    source: &str,
    provider: &str,
    has_key: bool,
    hosted_connected: bool,
    developer_mode: bool,
    is_local_endpoint: bool,
) -> AiAccessPresentation {
    let authless_local_active =
        source == "connection" && (is_local_endpoint || is_local_provider(provider));

    // Hosted takes precedence only when a real adapter reports connected and
    // explicit authless Local AI is not the active connection route.
    if hosted_connected && !authless_local_active {
        return AiAccessPresentation {
            access_mode: AiAccessMode::CoresideHosted,
            available: true,
            connection_state: "connected".into(),
            consumer_display_name: "Coreside AI".into(),
            user_facing_status: "Connected".into(),
            disclosure: DisclosurePolicy {
                show_provider_identity: false,
                show_model_identity: false,
                show_credential_source: false,
                show_provider_catalog: false,
                allow_model_selection: false,
                allow_provider_management: true,
                allow_connection_test: false,
                allow_developer_details: developer_mode,
            },
            safe_error_category: None,
        };
    }

    let local_connection =
        source == "connection" && (has_key || is_local_endpoint || is_local_provider(provider));

    if local_connection {
        if is_local_endpoint || is_local_provider(provider) || !has_key {
            return AiAccessPresentation {
                access_mode: AiAccessMode::UserLocal,
                available: true,
                connection_state: "connected".into(),
                consumer_display_name: "Local AI".into(),
                user_facing_status: "Connected".into(),
                disclosure: DisclosurePolicy {
                    show_provider_identity: true,
                    show_model_identity: true,
                    show_credential_source: true,
                    show_provider_catalog: true,
                    allow_model_selection: true,
                    allow_provider_management: true,
                    allow_connection_test: true,
                    allow_developer_details: developer_mode,
                },
                safe_error_category: None,
            };
        }
        return AiAccessPresentation {
            access_mode: AiAccessMode::UserByok,
            available: true,
            connection_state: "connected".into(),
            consumer_display_name: "AI Providers".into(),
            user_facing_status: "Connected".into(),
            disclosure: DisclosurePolicy {
                show_provider_identity: true,
                show_model_identity: true,
                show_credential_source: true,
                show_provider_catalog: true,
                allow_model_selection: true,
                allow_provider_management: true,
                allow_connection_test: true,
                allow_developer_details: developer_mode,
            },
            safe_error_category: None,
        };
    }

    if source == "env" && has_key {
        // Development / deployment credential — not user BYOK.
        // Consumer UI hides routing details unless Developer Mode is on.
        let disclosure = DisclosurePolicy {
            show_provider_identity: developer_mode,
            show_model_identity: developer_mode,
            show_credential_source: developer_mode,
            show_provider_catalog: developer_mode,
            allow_model_selection: true, // chat can still use preferredModel
            allow_provider_management: true,
            allow_connection_test: developer_mode,
            allow_developer_details: developer_mode,
        };
        // Never brand env/deployment credentials as "Coreside AI" (hosted-only).
        return AiAccessPresentation {
            access_mode: AiAccessMode::DeveloperEnvironment,
            available: true,
            connection_state: "connected".into(),
            consumer_display_name: "AI Access".into(),
            user_facing_status: if developer_mode {
                "Ready (development)".into()
            } else {
                "Ready".into()
            },
            disclosure,
            safe_error_category: None,
        };
    }

    if provider.eq_ignore_ascii_case("mock") {
        return AiAccessPresentation {
            access_mode: AiAccessMode::DeveloperEnvironment,
            available: true,
            connection_state: "connected".into(),
            consumer_display_name: "AI Access".into(),
            user_facing_status: "Ready".into(),
            disclosure: DisclosurePolicy {
                show_provider_identity: developer_mode,
                show_model_identity: developer_mode,
                show_credential_source: developer_mode,
                show_provider_catalog: false,
                allow_model_selection: false,
                allow_provider_management: true,
                allow_connection_test: developer_mode,
                allow_developer_details: developer_mode,
            },
            safe_error_category: None,
        };
    }

    AiAccessPresentation {
        access_mode: AiAccessMode::Unavailable,
        available: false,
        connection_state: "unavailable".into(),
        consumer_display_name: "AI Access".into(),
        user_facing_status: "Unavailable".into(),
        disclosure: DisclosurePolicy {
            show_provider_identity: false,
            show_model_identity: false,
            show_credential_source: false,
            show_provider_catalog: false,
            allow_model_selection: false,
            allow_provider_management: true,
            allow_connection_test: false,
            allow_developer_details: developer_mode,
        },
        safe_error_category: Some("no_ai_access".into()),
    }
}

fn is_local_provider(provider: &str) -> bool {
    matches!(
        provider.to_ascii_lowercase().as_str(),
        "ollama"
            | "lmstudio"
            | "vllm"
            | "llama_cpp"
            | "llama.cpp"
            | "local"
            | "openai-compatible-local"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_hides_provider_when_developer_mode_off() {
        let p = resolve_access_presentation("env", "openrouter", true, false, false, false);
        assert_eq!(p.access_mode, AiAccessMode::DeveloperEnvironment);
        assert!(!p.disclosure.show_provider_identity);
        assert!(!p.disclosure.show_model_identity);
        assert_eq!(p.consumer_display_name, "AI Access");
        assert_eq!(p.user_facing_status, "Ready");
    }

    #[test]
    fn byok_shows_provider() {
        let p = resolve_access_presentation("connection", "openrouter", true, false, false, false);
        assert_eq!(p.access_mode, AiAccessMode::UserByok);
        assert!(p.disclosure.show_provider_identity);
        assert!(p.disclosure.show_model_identity);
    }

    #[test]
    fn unavailable_when_no_key() {
        let p = resolve_access_presentation("none", "openrouter", false, false, false, false);
        assert_eq!(p.access_mode, AiAccessMode::Unavailable);
        assert!(!p.available);
        assert!(!p.disclosure.show_provider_identity);
    }

    #[test]
    fn hosted_hides_upstream() {
        let p = resolve_access_presentation("none", "openrouter", false, true, false, false);
        assert_eq!(p.access_mode, AiAccessMode::CoresideHosted);
        assert!(!p.disclosure.show_provider_identity);
        assert_eq!(p.consumer_display_name, "Coreside AI");
    }

    #[test]
    fn env_developer_mode_reveals() {
        let p = resolve_access_presentation("env", "openrouter", true, false, true, false);
        assert!(p.disclosure.show_provider_identity);
        assert!(p.disclosure.allow_developer_details);
    }

    #[test]
    fn authless_local_shows_without_key() {
        let p = resolve_access_presentation("connection", "ollama", false, true, false, true);
        assert_eq!(p.access_mode, AiAccessMode::UserLocal);
        assert!(p.available);
        assert_eq!(p.consumer_display_name, "Local AI");
    }

    #[test]
    fn authless_local_beats_hosted_presentation() {
        let p = resolve_access_presentation("connection", "ollama", false, false, false, true);
        assert_eq!(p.access_mode, AiAccessMode::UserLocal);
        assert_ne!(p.access_mode, AiAccessMode::CoresideHosted);
    }

    #[test]
    fn authless_ollama_provider_beats_hosted_without_endpoint_flag() {
        let p = resolve_access_presentation("connection", "ollama", false, true, false, false);
        assert_eq!(p.access_mode, AiAccessMode::UserLocal);
        assert_ne!(p.access_mode, AiAccessMode::CoresideHosted);
    }

    #[test]
    fn authless_local_beats_hosted_with_misleading_has_key_param() {
        let p = resolve_access_presentation("connection", "ollama", true, true, false, true);
        assert_eq!(p.access_mode, AiAccessMode::UserLocal);
        assert_ne!(p.access_mode, AiAccessMode::CoresideHosted);
    }

    #[test]
    fn local_providers_vllm_and_llama_cpp_resolve_user_local() {
        let vllm = resolve_access_presentation("connection", "vllm", false, false, false, false);
        assert_eq!(vllm.access_mode, AiAccessMode::UserLocal);
        assert_eq!(vllm.consumer_display_name, "Local AI");

        let llama =
            resolve_access_presentation("connection", "llama_cpp", false, false, false, false);
        assert_eq!(llama.access_mode, AiAccessMode::UserLocal);
        assert_eq!(llama.consumer_display_name, "Local AI");
    }
}
