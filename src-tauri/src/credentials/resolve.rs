//! Resolve effective AI credentials: secure connection → .env fallback.

use crate::config::{
    self, AppConfig, DEFAULT_ANTHROPIC_BASE_URL, DEFAULT_ANTHROPIC_MODEL, DEFAULT_GEMINI_BASE_URL,
    DEFAULT_GEMINI_MODEL, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_MODEL,
    DEFAULT_OPENROUTER_BASE_URL, DEFAULT_OPENROUTER_MODEL,
};
use crate::credentials::{self, CredentialError};
use crate::db::{self, Database, ProviderConnection};

#[derive(Debug, Clone)]
pub struct ResolvedCredentials {
    pub provider: String,
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: String,
    pub source: String, // "connection" | "env" | "none"
    pub active_connection_id: Option<String>,
    pub env_path: Option<String>,
}

impl ResolvedCredentials {
    pub fn has_api_key(&self) -> bool {
        self.api_key
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn to_app_config(&self) -> AppConfig {
        AppConfig {
            provider: self.provider.clone(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            base_url: self.base_url.clone(),
            log_level: "info".into(),
            env_path: self.env_path.clone(),
        }
    }
}

pub fn defaults_for(provider: &str) -> (String, String) {
    if let Some(desc) = crate::ai::platform::descriptor_by_id(provider) {
        return (
            desc.default_model_hint.unwrap_or("").to_string(),
            desc.default_endpoint.unwrap_or("").to_string(),
        );
    }
    match provider {
        "openai" => (
            DEFAULT_OPENAI_MODEL.to_string(),
            DEFAULT_OPENAI_BASE_URL.to_string(),
        ),
        // Compatible endpoints require an explicit base URL on the connection.
        "compatible" => (DEFAULT_OPENAI_MODEL.to_string(), String::new()),
        "anthropic" | "claude" => (
            DEFAULT_ANTHROPIC_MODEL.to_string(),
            DEFAULT_ANTHROPIC_BASE_URL.to_string(),
        ),
        "openrouter" => (
            DEFAULT_OPENROUTER_MODEL.to_string(),
            DEFAULT_OPENROUTER_BASE_URL.to_string(),
        ),
        _ => (
            DEFAULT_GEMINI_MODEL.to_string(),
            DEFAULT_GEMINI_BASE_URL.to_string(),
        ),
    }
}

/// Build resolved credentials from a connection row + keyring secret lookup.
pub fn from_connection_with_secret(
    conn: &ProviderConnection,
    get_secret: impl FnOnce(&str) -> Result<String, CredentialError>,
) -> Result<ResolvedCredentials, String> {
    let auth_mode = conn
        .auth_mode
        .as_deref()
        .and_then(crate::ai::platform::AuthMode::parse)
        .or_else(|| {
            crate::ai::platform::descriptor_by_id(&conn.provider).map(|d| d.default_auth_mode)
        })
        .unwrap_or(crate::ai::platform::AuthMode::ApiKeyBearer);

    let (default_model, default_base) = defaults_for(&conn.provider);
    let model = conn
        .model_default
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(default_model);
    let base_url = conn
        .base_url
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(default_base);

    if !auth_mode.requires_secret() {
        return Ok(ResolvedCredentials {
            provider: conn.provider.clone(),
            api_key: None,
            model,
            base_url,
            source: "connection".into(),
            active_connection_id: Some(conn.id.clone()),
            env_path: None,
        });
    }

    let key = get_secret(&conn.keyring_account).map_err(|e| e.to_string())?;
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Credential not found".into());
    }
    Ok(ResolvedCredentials {
        provider: conn.provider.clone(),
        api_key: Some(key),
        model,
        base_url,
        source: "connection".into(),
        active_connection_id: Some(conn.id.clone()),
        env_path: None,
    })
}

fn from_connection(conn: &ProviderConnection) -> Result<ResolvedCredentials, String> {
    from_connection_with_secret(conn, credentials::get_secret)
}

/// The database half of credential resolution.
///
/// Read this while the database lock is held, release the lock, then call
/// [`resolve_from_sources`]. Keychain lookups are blocking OS calls (they can
/// prompt, or hang when no Secret Service is running) and must never run while
/// the single shared SQLite connection is locked.
#[derive(Debug, Clone, Default)]
pub struct CredentialSources {
    active: Option<ProviderConnection>,
    explicit: Option<ProviderConnection>,
    hosted_adapter_connected: bool,
}

pub fn read_credential_sources(db: &Database, connection_id: Option<&str>) -> CredentialSources {
    CredentialSources {
        active: db::get_active_provider_connection(db).ok().flatten(),
        explicit: connection_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|id| db::get_provider_connection(db, id).ok()),
        hosted_adapter_connected: credentials::adapter_connected(db),
    }
}

/// Look up one connection row for test-before-activate. Pair with
/// [`resolve_connection_secret`] after dropping the database lock.
pub fn read_connection(db: &Database, connection_id: &str) -> Result<ProviderConnection, String> {
    let id = connection_id.trim();
    if id.is_empty() {
        return Err("connectionId is required".into());
    }
    db::get_provider_connection(db, id).map_err(|e| e.to_string())
}

/// Resolve a specific connection's secret. Must run with no database lock held.
pub fn resolve_connection_secret(conn: &ProviderConnection) -> Result<ResolvedCredentials, String> {
    from_connection(conn)
}

/// Resolve credentials for chat / status. Must run with no database lock held.
///
/// Precedence:
/// 1. Active secure connection (`is_active=1`)
/// 2. Explicit connection (when no active secret is available)
/// 3. Hosted adapter
/// 4. `.env`
/// 5. none
pub fn resolve_from_sources(sources: &CredentialSources) -> ResolvedCredentials {
    resolve_from_sources_with(sources, credentials::get_secret, config::load_config)
}

/// Convenience for callers that are not holding the database lock (tests, and
/// paths where the lock is already scoped away).
pub fn resolve_credentials(db: &Database, connection_id: Option<&str>) -> ResolvedCredentials {
    resolve_from_sources(&read_credential_sources(db, connection_id))
}

fn resolve_from_sources_with<F, E>(
    sources: &CredentialSources,
    get_secret: F,
    load_env: E,
) -> ResolvedCredentials
where
    F: Fn(&str) -> Result<String, CredentialError>,
    E: FnOnce() -> AppConfig,
{
    // 1. Active secure connection
    if let Some(conn) = sources.active.as_ref() {
        if let Ok(resolved) = from_connection_with_secret(conn, &get_secret) {
            return resolved;
        }
    }

    // 2. Explicit connection id (only when active is missing/unusable)
    if let Some(conn) = sources.explicit.as_ref() {
        if let Ok(resolved) = from_connection_with_secret(conn, &get_secret) {
            return resolved;
        }
    }

    // 3. Hosted session beats .env when the adapter is effective (parity with access_mode).
    if sources.hosted_adapter_connected {
        return ResolvedCredentials {
            provider: "coreside_hosted".into(),
            api_key: None,
            model: "auto".into(),
            base_url: String::new(),
            source: "none".into(),
            active_connection_id: None,
            env_path: None,
        };
    }

    // 4–5. .env fallback, else none
    let env = load_env();
    let source = if env.has_api_key() { "env" } else { "none" };
    ResolvedCredentials {
        provider: env.provider,
        api_key: env.api_key,
        model: env.model,
        base_url: env.base_url,
        source: source.into(),
        active_connection_id: None,
        env_path: env.env_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::account_for_connection;
    use crate::db::{self, Database, ProviderConnection};
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tempfile::tempdir;

    fn sample_conn(id: &str, provider: &str, active: bool) -> ProviderConnection {
        let now = "2026-01-01T00:00:00Z".to_string();
        ProviderConnection {
            id: id.to_string(),
            provider: provider.to_string(),
            label: format!("{provider} label"),
            base_url: None,
            model_default: None,
            keyring_account: account_for_connection(id),
            is_active: active,
            last_status: None,
            last_tested_at: None,
            created_at: now.clone(),
            updated_at: now,
            provider_descriptor_id: Some(provider.to_string()),
            protocol_family: None,
            auth_mode: None,
            endpoint_class: None,
            api_version: None,
            region: None,
            deployment: None,
            organization_id: None,
            project_id: None,
            capability_profile_json: None,
            capability_checked_at: None,
            model_catalog_checked_at: None,
            provider_preset_version: Some("1".into()),
            enabled: true,
        }
    }

    #[test]
    fn precedence_active_beats_explicit_and_env() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("cred.db")).unwrap();
        let active = sample_conn("active-1", "openai", false);
        let other = sample_conn("other-1", "anthropic", false);
        db::upsert_provider_connection(&mut db, &active).unwrap();
        db::upsert_provider_connection(&mut db, &other).unwrap();
        db::set_active_provider_connection(&mut db, "active-1").unwrap();

        let secrets = HashMap::from([
            (
                account_for_connection("active-1"),
                "sk-active-key".to_string(),
            ),
            (
                account_for_connection("other-1"),
                "sk-other-key".to_string(),
            ),
        ]);
        let env_cfg = AppConfig {
            provider: "gemini".into(),
            api_key: Some("env-key".into()),
            model: DEFAULT_GEMINI_MODEL.into(),
            base_url: DEFAULT_GEMINI_BASE_URL.into(),
            log_level: "info".into(),
            env_path: None,
        };

        let resolved = resolve_from_sources_with(
            &read_credential_sources(&db, Some("other-1")),
            |account| {
                secrets
                    .get(account)
                    .cloned()
                    .ok_or(CredentialError::NotFound)
            },
            || env_cfg.clone(),
        );

        assert_eq!(resolved.source, "connection");
        assert_eq!(resolved.active_connection_id.as_deref(), Some("active-1"));
        assert_eq!(resolved.api_key.as_deref(), Some("sk-active-key"));
        assert_eq!(resolved.provider, "openai");
    }

    #[test]
    fn precedence_explicit_when_no_active() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("cred2.db")).unwrap();
        let conn = sample_conn("only-1", "anthropic", false);
        db::upsert_provider_connection(&mut db, &conn).unwrap();

        let secrets = Mutex::new(HashMap::from([(
            account_for_connection("only-1"),
            "sk-ant-test".to_string(),
        )]));
        let env_cfg = AppConfig {
            provider: "gemini".into(),
            api_key: Some("env-key".into()),
            model: DEFAULT_GEMINI_MODEL.into(),
            base_url: DEFAULT_GEMINI_BASE_URL.into(),
            log_level: "info".into(),
            env_path: None,
        };

        let resolved = resolve_from_sources_with(
            &read_credential_sources(&db, Some("only-1")),
            |account| {
                secrets
                    .lock()
                    .unwrap()
                    .get(account)
                    .cloned()
                    .ok_or(CredentialError::NotFound)
            },
            || env_cfg.clone(),
        );

        assert_eq!(resolved.source, "connection");
        assert_eq!(resolved.active_connection_id.as_deref(), Some("only-1"));
        assert_eq!(resolved.api_key.as_deref(), Some("sk-ant-test"));
        assert_eq!(resolved.provider, "anthropic");
    }

    #[test]
    fn precedence_falls_back_to_env() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("cred3.db")).unwrap();
        let env_cfg = AppConfig {
            provider: "openrouter".into(),
            api_key: Some("or-env-key".into()),
            model: DEFAULT_OPENROUTER_MODEL.into(),
            base_url: DEFAULT_OPENROUTER_BASE_URL.into(),
            log_level: "info".into(),
            env_path: Some("/tmp/.env".into()),
        };

        let resolved = resolve_from_sources_with(
            &read_credential_sources(&db, None),
            |_| Err(CredentialError::NotFound),
            || env_cfg.clone(),
        );

        assert_eq!(resolved.source, "env");
        assert_eq!(resolved.api_key.as_deref(), Some("or-env-key"));
        assert_eq!(resolved.provider, "openrouter");
        assert!(resolved.active_connection_id.is_none());
    }

    #[test]
    fn precedence_none_when_no_sources() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("cred4.db")).unwrap();
        let env_cfg = AppConfig {
            provider: "gemini".into(),
            api_key: None,
            model: DEFAULT_GEMINI_MODEL.into(),
            base_url: DEFAULT_GEMINI_BASE_URL.into(),
            log_level: "info".into(),
            env_path: None,
        };

        let resolved = resolve_from_sources_with(
            &read_credential_sources(&db, Some("missing")),
            |_| Err(CredentialError::NotFound),
            || env_cfg.clone(),
        );

        assert_eq!(resolved.source, "none");
        assert!(!resolved.has_api_key());
    }

    #[test]
    fn compatible_defaults_require_explicit_base() {
        let (model, base) = defaults_for("compatible");
        assert_eq!(model, DEFAULT_OPENAI_MODEL);
        assert!(base.is_empty());
    }

    fn ollama_authless_conn(auth_mode: Option<&str>) -> ProviderConnection {
        ProviderConnection {
            id: "ollama-1".into(),
            provider: "ollama".into(),
            label: "Local Ollama".into(),
            base_url: Some("http://127.0.0.1:11434".into()),
            model_default: Some("llama3.2".into()),
            keyring_account: account_for_connection("ollama-1"),
            is_active: true,
            last_status: Some("connected".into()),
            last_tested_at: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            provider_descriptor_id: Some("ollama".into()),
            protocol_family: Some("ollama_native_chat".into()),
            auth_mode: auth_mode.map(str::to_string),
            endpoint_class: Some("loopback_local".into()),
            api_version: None,
            region: None,
            deployment: None,
            organization_id: None,
            project_id: None,
            capability_profile_json: None,
            capability_checked_at: None,
            model_catalog_checked_at: None,
            provider_preset_version: Some("1".into()),
            enabled: true,
        }
    }

    #[test]
    fn authless_local_connection_resolves_without_secret() {
        let conn = ollama_authless_conn(Some("local_authless"));
        let resolved = from_connection_with_secret(&conn, |_| Err(CredentialError::NotFound))
            .expect("authless resolve");
        assert!(!resolved.has_api_key());
        assert!(resolved.api_key.is_none());
        assert_eq!(resolved.provider, "ollama");
        assert_eq!(resolved.model, "llama3.2");
        assert_eq!(resolved.base_url, "http://127.0.0.1:11434");
        assert_eq!(resolved.source, "connection");
    }

    #[test]
    fn authless_never_invokes_secret_lookup() {
        let conn = ollama_authless_conn(Some("local_authless"));
        let resolved = from_connection_with_secret(&conn, |_| {
            panic!("authless connections must not read the keyring");
        })
        .expect("authless resolve");
        assert!(resolved.api_key.is_none());
    }

    #[test]
    fn authless_infers_mode_from_descriptor_when_auth_mode_missing() {
        let conn = ollama_authless_conn(None);
        let resolved = from_connection_with_secret(&conn, |_| {
            panic!("descriptor-derived authless must not read the keyring");
        })
        .expect("descriptor authless resolve");
        assert!(resolved.api_key.is_none());
        assert_eq!(resolved.provider, "ollama");
    }
}
