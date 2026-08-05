//! Resolve effective AI credentials: secure connection → .env fallback.

use crate::ai::platform::{self, AuthMode};
use crate::config::{
    self, AppConfig, DEFAULT_ANTHROPIC_BASE_URL, DEFAULT_ANTHROPIC_MODEL, DEFAULT_GEMINI_BASE_URL,
    DEFAULT_GEMINI_MODEL, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_MODEL,
    DEFAULT_OPENROUTER_BASE_URL, DEFAULT_OPENROUTER_MODEL,
};
use crate::credentials::{self, CredentialError};
use crate::db::{self, Database, ProviderConnection};

/// Trusted runtime route for AI access. Prevents silent Hosted fallthrough when
/// explicit authless Local AI is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiAccessRoute {
    LocalAuthless,
    UserByok,
    CoresideHosted,
    DeveloperEnv,
    Mock,
    Unavailable,
}

#[derive(Clone)]
pub struct ResolvedAiAccess {
    pub credentials: ResolvedCredentials,
    pub route: AiAccessRoute,
}

impl std::fmt::Debug for ResolvedAiAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedAiAccess")
            .field("credentials", &self.credentials)
            .field("route", &self.route)
            .finish()
    }
}

#[derive(Clone)]
pub struct ResolvedCredentials {
    pub provider: String,
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: String,
    pub source: String, // "connection" | "env" | "none"
    pub active_connection_id: Option<String>,
    pub env_path: Option<String>,
    /// Active BYOK row exists but the keyring secret is missing/invalid.
    pub missing_connection_secret: bool,
}

impl std::fmt::Debug for ResolvedCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedCredentials")
            .field("provider", &self.provider)
            .field(
                "api_key",
                &self
                    .api_key
                    .as_ref()
                    .map(|_| crate::security::REDACTED_SECRET),
            )
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("source", &self.source)
            .field("active_connection_id", &self.active_connection_id)
            .field("env_path", &self.env_path)
            .field("missing_connection_secret", &self.missing_connection_secret)
            .finish()
    }
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

    pub fn access_route(&self) -> AiAccessRoute {
        route_for_credentials(self)
    }

    pub fn is_authless_local(&self) -> bool {
        self.access_route() == AiAccessRoute::LocalAuthless
    }
}

pub fn connection_auth_mode(conn: &ProviderConnection) -> AuthMode {
    conn.auth_mode
        .as_deref()
        .and_then(AuthMode::parse)
        .or_else(|| platform::descriptor_by_id(&conn.provider).map(|d| d.default_auth_mode))
        .unwrap_or(AuthMode::ApiKeyBearer)
}

pub fn is_authless_local_connection(conn: &ProviderConnection) -> bool {
    connection_auth_mode(conn) == AuthMode::LocalAuthless
}

pub fn is_authless_local_provider(provider: &str) -> bool {
    platform::descriptor_by_id(provider)
        .map(|d| d.local && !d.default_auth_mode.requires_secret())
        .unwrap_or(false)
}

/// Provider presets that only support authless local access (e.g. Ollama).
pub fn connection_is_mandatory_authless(conn: &ProviderConnection) -> bool {
    platform::descriptor_by_id(&conn.provider)
        .map(|d| {
            !d.auth_modes.is_empty()
                && d.auth_modes
                    .iter()
                    .all(|mode| *mode == AuthMode::LocalAuthless)
        })
        .unwrap_or(false)
}

fn route_for_credentials(creds: &ResolvedCredentials) -> AiAccessRoute {
    if creds.provider.eq_ignore_ascii_case("mock") {
        return AiAccessRoute::Mock;
    }
    if creds.source == "connection" {
        if creds.missing_connection_secret {
            return AiAccessRoute::Unavailable;
        }
        if creds.has_api_key() {
            return AiAccessRoute::UserByok;
        }
        // Connection resolved without a secret (Ollama, loopback compatible, …).
        return AiAccessRoute::LocalAuthless;
    }
    if creds.provider == "coreside_hosted" {
        return AiAccessRoute::CoresideHosted;
    }
    if creds.source == "env" && creds.has_api_key() {
        return AiAccessRoute::DeveloperEnv;
    }
    AiAccessRoute::Unavailable
}

pub fn resolve_ai_access(sources: &CredentialSources) -> ResolvedAiAccess {
    let credentials = resolve_from_sources(sources);
    let route = route_for_credentials(&credentials);
    ResolvedAiAccess { credentials, route }
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
        "gemini" => (
            DEFAULT_GEMINI_MODEL.to_string(),
            DEFAULT_GEMINI_BASE_URL.to_string(),
        ),
        // Unknown providers: no silent Gemini default (descriptor path handles presets).
        _ => (String::new(), String::new()),
    }
}

/// Build resolved credentials from a connection row + keyring secret lookup.
pub fn from_connection_with_secret(
    conn: &ProviderConnection,
    get_secret: impl FnOnce(&str) -> Result<String, CredentialError>,
) -> Result<ResolvedCredentials, String> {
    let mut auth_mode = conn
        .auth_mode
        .as_deref()
        .and_then(crate::ai::platform::AuthMode::parse)
        .or_else(|| {
            crate::ai::platform::descriptor_by_id(&conn.provider).map(|d| d.default_auth_mode)
        })
        .unwrap_or(crate::ai::platform::AuthMode::ApiKeyBearer);
    // Privacy P0: mandatory-authless presets (Ollama, etc.) never read the keyring.
    if connection_is_mandatory_authless(conn) {
        auth_mode = AuthMode::LocalAuthless;
    }

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
            missing_connection_secret: false,
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
        missing_connection_secret: false,
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
    pub(crate) active: Option<ProviderConnection>,
    pub(crate) explicit: Option<ProviderConnection>,
    pub(crate) hosted_adapter_connected: bool,
}

impl CredentialSources {
    pub fn hosted_adapter_connected(&self) -> bool {
        self.hosted_adapter_connected
    }

    pub fn with_hosted_adapter_effective(mut self) -> Self {
        self.hosted_adapter_connected =
            super::hosted_session::hosted_adapter_effective(self.active.as_ref());
        self
    }
}

/// Database snapshot only — safe while the SQLite lock is held.
/// Call [`complete_credential_sources`] after releasing the lock so keyring
/// lookups for hosted-adapter gating never run under the database mutex.
pub fn read_credential_sources(db: &Database, connection_id: Option<&str>) -> CredentialSources {
    CredentialSources {
        active: db::get_active_provider_connection(db).ok().flatten(),
        explicit: connection_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|id| db::get_provider_connection(db, id).ok()),
        hosted_adapter_connected: false,
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
    let sources = read_credential_sources(db, connection_id);
    resolve_from_sources(&super::complete_credential_sources(sources))
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
        // Active BYOK with missing/invalid secret must not fall through to Hosted or .env.
        if !is_authless_local_connection(conn) && !connection_is_mandatory_authless(conn) {
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
            return ResolvedCredentials {
                provider: conn.provider.clone(),
                api_key: None,
                model,
                base_url,
                source: "connection".into(),
                active_connection_id: Some(conn.id.clone()),
                env_path: None,
                missing_connection_secret: true,
            };
        }
    }

    // 2. Explicit connection id (only when active is missing/unusable)
    // Explicit selection must fail closed — never fall through to Hosted or .env.
    if let Some(conn) = sources.explicit.as_ref() {
        if let Ok(resolved) = from_connection_with_secret(conn, &get_secret) {
            return resolved;
        }
        if !is_authless_local_connection(conn) && !connection_is_mandatory_authless(conn) {
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
            return ResolvedCredentials {
                provider: conn.provider.clone(),
                api_key: None,
                model,
                base_url,
                source: "connection".into(),
                active_connection_id: Some(conn.id.clone()),
                env_path: None,
                missing_connection_secret: true,
            };
        }
        // Explicit authless Local that failed resolution still must not fall through.
        if is_authless_local_connection(conn) || connection_is_mandatory_authless(conn) {
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
            return ResolvedCredentials {
                provider: conn.provider.clone(),
                api_key: None,
                model,
                base_url,
                source: "connection".into(),
                active_connection_id: Some(conn.id.clone()),
                env_path: None,
                // Authless has no secret; mark unavailable so callers fail closed.
                missing_connection_secret: true,
            };
        }
    }

    // Active authless Local AI is authoritative — never fall through to Hosted or .env.
    if let Some(conn) = sources.active.as_ref() {
        if is_authless_local_connection(conn) || connection_is_mandatory_authless(conn) {
            if let Ok(resolved) = from_connection_with_secret(conn, &get_secret) {
                return resolved;
            }
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
            return ResolvedCredentials {
                provider: conn.provider.clone(),
                api_key: None,
                model,
                base_url,
                source: "connection".into(),
                active_connection_id: Some(conn.id.clone()),
                env_path: None,
                missing_connection_secret: true,
            };
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
            missing_connection_secret: false,
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
        missing_connection_secret: false,
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

    #[test]
    fn authless_local_route_never_hosted() {
        let conn = ollama_authless_conn(Some("local_authless"));
        let resolved = from_connection_with_secret(&conn, |_| Err(CredentialError::NotFound))
            .expect("authless resolve");
        assert_eq!(resolved.access_route(), AiAccessRoute::LocalAuthless);
        assert!(resolved.is_authless_local());
    }

    #[test]
    fn authless_active_beats_hosted_adapter_in_resolution() {
        let conn = ollama_authless_conn(Some("local_authless"));
        let sources = CredentialSources {
            active: Some(conn),
            explicit: None,
            hosted_adapter_connected: true,
        };
        let env_cfg = AppConfig {
            provider: "gemini".into(),
            api_key: Some("env-key".into()),
            model: DEFAULT_GEMINI_MODEL.into(),
            base_url: DEFAULT_GEMINI_BASE_URL.into(),
            log_level: "info".into(),
            env_path: None,
        };
        let resolved =
            resolve_from_sources_with(&sources, |_| Err(CredentialError::NotFound), || env_cfg);
        assert_eq!(resolved.provider, "ollama");
        assert_eq!(resolved.access_route(), AiAccessRoute::LocalAuthless);
        assert_eq!(resolved.source, "connection");
    }

    #[test]
    fn resolve_ai_access_wraps_route() {
        let conn = ollama_authless_conn(Some("local_authless"));
        let sources = CredentialSources {
            active: Some(conn),
            explicit: None,
            hosted_adapter_connected: true,
        };
        let access = resolve_ai_access(&sources);
        assert_eq!(access.route, AiAccessRoute::LocalAuthless);
        assert_eq!(access.credentials.provider, "ollama");
    }

    #[test]
    fn mandatory_authless_ollama_blocks_hosted_despite_wrong_auth_mode() {
        let mut conn = ollama_authless_conn(Some("api_key_bearer"));
        conn.auth_mode = Some("api_key_bearer".into());
        let sources = CredentialSources {
            active: Some(conn),
            explicit: None,
            hosted_adapter_connected: true,
        };
        let resolved = resolve_from_sources_with(
            &sources,
            |_| Err(CredentialError::NotFound),
            || AppConfig {
                provider: "gemini".into(),
                api_key: Some("env-key".into()),
                model: DEFAULT_GEMINI_MODEL.into(),
                base_url: DEFAULT_GEMINI_BASE_URL.into(),
                log_level: "info".into(),
                env_path: None,
            },
        );
        assert_eq!(resolved.provider, "ollama");
        assert_eq!(resolved.access_route(), AiAccessRoute::LocalAuthless);
        assert_eq!(resolved.source, "connection");
    }

    #[test]
    fn connection_is_mandatory_authless_for_ollama() {
        let conn = ollama_authless_conn(None);
        assert!(connection_is_mandatory_authless(&conn));
    }

    #[test]
    fn connection_authless_route_for_compatible_local() {
        let creds = ResolvedCredentials {
            provider: "compatible".into(),
            api_key: None,
            model: "gpt-4o-mini".into(),
            base_url: "http://127.0.0.1:8080/v1".into(),
            source: "connection".into(),
            active_connection_id: Some("compat-1".into()),
            env_path: None,
            missing_connection_secret: false,
        };
        assert_eq!(creds.access_route(), AiAccessRoute::LocalAuthless);
        assert!(creds.is_authless_local());
    }

    #[test]
    fn byok_connection_route_is_user_byok_not_hosted() {
        let conn = sample_conn("byok-1", "openai", true);
        let secrets =
            HashMap::from([(account_for_connection("byok-1"), "sk-byok-key".to_string())]);
        let resolved = from_connection_with_secret(&conn, |account| {
            secrets
                .get(account)
                .cloned()
                .ok_or(CredentialError::NotFound)
        })
        .expect("byok resolve");
        assert_eq!(resolved.access_route(), AiAccessRoute::UserByok);
        assert_ne!(resolved.access_route(), AiAccessRoute::CoresideHosted);
    }

    #[test]
    fn defaults_for_kimi_and_mistral_use_descriptor_hints_not_gemini() {
        let (kimi_model, kimi_base) = defaults_for("kimi");
        assert_eq!(kimi_model, "kimi-k3");
        assert_eq!(kimi_base, "https://api.moonshot.ai/v1");
        assert_ne!(kimi_model, DEFAULT_GEMINI_MODEL);
        assert_ne!(kimi_base, DEFAULT_GEMINI_BASE_URL);

        let (mistral_model, mistral_base) = defaults_for("mistral");
        assert_eq!(mistral_model, "mistral-large-latest");
        assert_eq!(mistral_base, "https://api.mistral.ai/v1");
        assert_ne!(mistral_model, DEFAULT_GEMINI_MODEL);
    }

    #[test]
    fn defaults_for_unknown_provider_does_not_fall_back_to_gemini() {
        let (model, base) = defaults_for("not-a-real-provider");
        assert!(model.is_empty());
        assert!(base.is_empty());
    }

    #[test]
    fn broken_active_byok_blocks_hosted_fallback() {
        let conn = sample_conn("byok-1", "openai", true);
        let sources = CredentialSources {
            active: Some(conn),
            explicit: None,
            hosted_adapter_connected: true,
        };
        let resolved = resolve_from_sources_with(
            &sources,
            |_| Err(CredentialError::NotFound),
            || AppConfig {
                provider: "gemini".into(),
                api_key: Some("env-key".into()),
                model: DEFAULT_GEMINI_MODEL.into(),
                base_url: DEFAULT_GEMINI_BASE_URL.into(),
                log_level: "info".into(),
                env_path: None,
            },
        );
        assert_eq!(resolved.provider, "openai");
        assert_eq!(resolved.source, "connection");
        assert!(resolved.missing_connection_secret);
        assert_eq!(resolved.access_route(), AiAccessRoute::Unavailable);
        assert!(!resolved.has_api_key());
    }

    #[test]
    fn broken_explicit_byok_blocks_hosted_fallback() {
        let conn = sample_conn("byok-explicit", "openai", false);
        let sources = CredentialSources {
            active: None,
            explicit: Some(conn),
            hosted_adapter_connected: true,
        };
        let resolved = resolve_from_sources_with(
            &sources,
            |_| Err(CredentialError::NotFound),
            || AppConfig {
                provider: "gemini".into(),
                api_key: Some("env-key".into()),
                model: DEFAULT_GEMINI_MODEL.into(),
                base_url: DEFAULT_GEMINI_BASE_URL.into(),
                log_level: "info".into(),
                env_path: None,
            },
        );
        assert_eq!(resolved.provider, "openai");
        assert_eq!(resolved.source, "connection");
        assert!(resolved.missing_connection_secret);
        assert_eq!(resolved.access_route(), AiAccessRoute::Unavailable);
        assert_ne!(resolved.provider, "coreside_hosted");
        assert!(!resolved.has_api_key());
    }

    #[test]
    fn debug_does_not_leak_api_key() {
        let creds = ResolvedCredentials {
            provider: "openai".into(),
            api_key: Some("sentinel-api-key-xyz".into()),
            model: "gpt-4".into(),
            base_url: "https://api.openai.com/v1".into(),
            source: "env".into(),
            active_connection_id: None,
            env_path: None,
            missing_connection_secret: false,
        };
        let access = ResolvedAiAccess {
            credentials: creds,
            route: AiAccessRoute::DeveloperEnv,
        };
        let debug = format!("{access:?}");
        assert!(!debug.contains("sentinel-api-key-xyz"));
        assert!(debug.contains("[REDACTED]"));
    }
}
