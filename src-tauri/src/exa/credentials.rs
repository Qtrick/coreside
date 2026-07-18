//! Resolve Exa API key: keyring `coreside.search.exa` then env `EXA_API_KEY`.
//! Never store the key in SQLite.

use std::env;

use super::errors::ExaError;
use crate::search::{get_search_secret, search_keyring_account, set_search_secret, delete_search_secret};

pub const EXA_PROVIDER: &str = "exa";
pub const EXA_ENV_KEY: &str = "EXA_API_KEY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExaCredentialSource {
    Keyring,
    Env,
    None,
}

impl ExaCredentialSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::Env => "env",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedExaCredentials {
    pub api_key: Option<String>,
    pub source: ExaCredentialSource,
}

impl ResolvedExaCredentials {
    pub fn has_key(&self) -> bool {
        self.api_key
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }
}

pub fn exa_keyring_account() -> String {
    search_keyring_account(EXA_PROVIDER)
}

/// Precedence: OS keyring → `EXA_API_KEY` → none.
pub fn resolve_exa_credentials() -> ResolvedExaCredentials {
    resolve_exa_credentials_with(
        || get_search_secret(&exa_keyring_account()).ok(),
        || env::var(EXA_ENV_KEY).ok(),
    )
}

pub fn resolve_exa_credentials_with<K, E>(get_keyring: K, get_env: E) -> ResolvedExaCredentials
where
    K: FnOnce() -> Option<String>,
    E: FnOnce() -> Option<String>,
{
    if let Some(key) = get_keyring() {
        let trimmed = key.trim().to_string();
        if !trimmed.is_empty() {
            return ResolvedExaCredentials {
                api_key: Some(trimmed),
                source: ExaCredentialSource::Keyring,
            };
        }
    }
    if let Some(key) = get_env() {
        let trimmed = key.trim().to_string();
        if !trimmed.is_empty() {
            return ResolvedExaCredentials {
                api_key: Some(trimmed),
                source: ExaCredentialSource::Env,
            };
        }
    }
    ResolvedExaCredentials {
        api_key: None,
        source: ExaCredentialSource::None,
    }
}

pub fn require_exa_api_key() -> Result<(String, ExaCredentialSource), ExaError> {
    let resolved = resolve_exa_credentials();
    match resolved.api_key {
        Some(key) if !key.is_empty() => Ok((key, resolved.source)),
        _ => Err(ExaError::NotConfigured(
            "Configure Exa (Settings) or set EXA_API_KEY for open-web search".into(),
        )),
    }
}

pub fn store_exa_api_key(secret: &str) -> Result<(), ExaError> {
    set_search_secret(&exa_keyring_account(), secret).map_err(|e| ExaError::Credential(e.to_string()))
}

pub fn delete_exa_api_key() -> Result<(), ExaError> {
    delete_search_secret(&exa_keyring_account()).map_err(|e| ExaError::Credential(e.to_string()))
}

pub fn has_exa_key() -> bool {
    resolve_exa_credentials().has_key()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_beats_env() {
        let resolved = resolve_exa_credentials_with(
            || Some(" keyring-key ".into()),
            || Some("env-key".into()),
        );
        assert_eq!(resolved.source, ExaCredentialSource::Keyring);
        assert_eq!(resolved.api_key.as_deref(), Some("keyring-key"));
    }

    #[test]
    fn env_fallback() {
        let resolved = resolve_exa_credentials_with(|| None, || Some("env-only".into()));
        assert_eq!(resolved.source, ExaCredentialSource::Env);
        assert!(resolved.has_key());
    }

    #[test]
    fn none_when_empty() {
        let resolved = resolve_exa_credentials_with(|| Some("  ".into()), || None);
        assert_eq!(resolved.source, ExaCredentialSource::None);
        assert!(!resolved.has_key());
    }

    #[test]
    fn account_id() {
        assert_eq!(exa_keyring_account(), "coreside.search.exa");
    }
}
