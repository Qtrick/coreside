//! Provider connection CRUD + secure key storage commands.
//!
//! Keys are written to the OS keychain only after a successful provider health
//! check when a new `apiKey` is supplied (test-then-save). Metadata-only updates
//! of an existing connection may skip re-testing.

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::CommandError;
use crate::ai::{create_provider_for_access, AiProvider};
use crate::config::AppConfig;
use crate::credentials::{self, defaults_for, ResolvedAiAccess, ResolvedCredentials};
use crate::db::{self, ProviderConnection};
use crate::security::sanitize_error;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionView {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub base_url: Option<String>,
    pub model_default: Option<String>,
    pub is_active: bool,
    pub has_key: bool,
    pub last_status: Option<String>,
    pub last_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_family: Option<String>,
    pub local: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertProviderConnectionInput {
    pub id: Option<String>,
    pub provider: String,
    pub label: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_default: Option<String>,
    pub set_active: Option<bool>,
}

fn to_view(row: ProviderConnection) -> ProviderConnectionView {
    let desc = crate::ai::platform::descriptor_by_id(&row.provider);
    let auth_mode = row
        .auth_mode
        .as_deref()
        .and_then(crate::ai::platform::AuthMode::parse)
        .or_else(|| desc.as_ref().map(|d| d.default_auth_mode));
    let local = desc.as_ref().map(|d| d.local).unwrap_or(false)
        || auth_mode == Some(crate::ai::platform::AuthMode::LocalAuthless);
    let has_key = if auth_mode == Some(crate::ai::platform::AuthMode::LocalAuthless) {
        true
    } else {
        credentials::has_secret(&row.keyring_account)
    };
    ProviderConnectionView {
        id: row.id,
        provider: row.provider,
        label: row.label,
        base_url: row.base_url,
        model_default: row.model_default,
        is_active: row.is_active,
        has_key,
        last_status: row.last_status,
        last_tested_at: row.last_tested_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        auth_mode: row.auth_mode,
        endpoint_class: row.endpoint_class,
        protocol_family: row.protocol_family,
        local,
    }
}

fn normalize_provider(raw: &str) -> Result<String, CommandError> {
    let desc = crate::ai::platform::descriptor_by_id(raw).ok_or_else(|| {
        CommandError::new(
            "invalid",
            format!("Unsupported provider '{raw}'. Choose a provider from AI connections."),
        )
    })?;
    Ok(desc.id.to_string())
}

fn build_probe_config(
    provider: &str,
    api_key: &str,
    base_url: Option<&str>,
    model_default: Option<&str>,
) -> Result<AppConfig, CommandError> {
    let (default_model, default_base) = defaults_for(provider);
    let base_url = base_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or(default_base);
    let desc = crate::ai::platform::descriptor_by_id(provider);
    if desc
        .as_ref()
        .map(|d| {
            d.endpoint_class
                == crate::ai::platform::EndpointClass::UserConfiguredRemoteCompatible
        })
        .unwrap_or(false)
        && base_url.trim().is_empty()
    {
        return Err(CommandError::new(
            "invalid",
            "Custom compatible providers require a base URL",
        ));
    }
    let model = model_default
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(default_model.as_str())
        .to_string();
    let key = api_key.trim();
    Ok(AppConfig {
        provider: provider.to_string(),
        api_key: if key.is_empty() {
            None
        } else {
            Some(key.to_string())
        },
        model,
        base_url,
        log_level: "info".into(),
        env_path: None,
    })
}

fn access_from_probe_config(config: &AppConfig) -> ResolvedAiAccess {
    let credentials = ResolvedCredentials {
        provider: config.provider.clone(),
        api_key: config.api_key.clone(),
        model: config.model.clone(),
        base_url: config.base_url.clone(),
        source: "connection".into(),
        active_connection_id: None,
        env_path: None,
        missing_connection_secret: false,
    };
    let route = credentials.access_route();
    ResolvedAiAccess {
        credentials,
        route,
    }
}

async fn health_check_key(config: &AppConfig) -> Result<(), CommandError> {
    let key = config.api_key.clone();
    let access = access_from_probe_config(config);
    let provider = create_provider_for_access(&access, None)
        .map_err(|e| CommandError::sanitized(e.code(), e, key.as_deref()))?;
    let cancel = CancellationToken::new();
    provider
        .health_check(cancel)
        .await
        .map_err(|e| CommandError::sanitized(e.code(), e, key.as_deref()))?;
    Ok(())
}

#[tauri::command]
pub fn list_provider_connections(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderConnectionView>, CommandError> {
    state.require_profile()?;
    // `to_view` probes the keychain per row; release the lock before that loop.
    let rows = {
        let db = state.db.lock();
        db::list_provider_connections(&db)?
    };
    Ok(rows.into_iter().map(to_view).collect())
}

#[tauri::command]
pub async fn upsert_provider_connection(
    state: State<'_, AppState>,
    input: UpsertProviderConnectionInput,
) -> Result<ProviderConnectionView, CommandError> {
    state.require_profile()?;
    let provider = normalize_provider(&input.provider)?;
    let label = input.label.trim();
    if label.is_empty() {
        return Err(CommandError::new("invalid", "Label cannot be empty"));
    }

    let id = input
        .id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let keyring_account = credentials::account_for_connection(&id);

    let api_key = input
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let base_url = input
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let model_default = input
        .model_default
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    if provider == "compatible" && base_url.is_none() {
        let existing_has_base = {
            let db = state.db.lock();
            db::get_provider_connection(&db, &id)
                .ok()
                .and_then(|e| e.base_url)
                .filter(|s| !s.trim().is_empty())
                .is_some()
        };
        if !existing_has_base {
            return Err(CommandError::new(
                "invalid",
                "Custom OpenAI-compatible providers require a base URL",
            ));
        }
    }

    let existing = {
        let db = state.db.lock();
        db::get_provider_connection(&db, &id).ok()
    };

    let descriptor = crate::ai::platform::descriptor_by_id(&provider).ok_or_else(|| {
        CommandError::new("invalid", format!("Unknown provider descriptor '{provider}'"))
    })?;
    let auth_mode = descriptor.default_auth_mode;
    let endpoint_class = descriptor.endpoint_class;

    let resolved_base = base_url
        .clone()
        .or_else(|| existing.as_ref().and_then(|e| e.base_url.clone()))
        .or_else(|| descriptor.default_endpoint.map(|s| s.to_string()));

    if let Some(ref url) = resolved_base {
        let is_override = descriptor.default_endpoint.map(|d| d != url.as_str()).unwrap_or(true);
        crate::ai::platform::classify_and_validate_endpoint(
            url,
            endpoint_class,
            descriptor.allows_endpoint_override,
            is_override,
        )
        .map_err(|e| CommandError::new("invalid_endpoint", e.to_string()))?;
    } else if descriptor.endpoint_class
        == crate::ai::platform::EndpointClass::UserConfiguredRemoteCompatible
        || descriptor.local
    {
        return Err(CommandError::new(
            "invalid",
            "This connection requires a base URL",
        ));
    }

    let mut last_status = existing.as_ref().and_then(|e| e.last_status.clone());
    let mut last_tested_at = existing.as_ref().and_then(|e| e.last_tested_at.clone());
    let now = chrono::Utc::now().to_rfc3339();

    if auth_mode.requires_secret() {
        if let Some(ref key) = api_key {
            // Test-then-save: never persist a new key until health_check succeeds.
            let probe = build_probe_config(
                &provider,
                key,
                resolved_base.as_deref(),
                model_default
                    .as_deref()
                    .or(existing.as_ref().and_then(|e| e.model_default.as_deref())),
            )?;
            if let Err(err) = health_check_key(&probe).await {
                return Err(CommandError::new(
                    "provider_test_failed",
                    format!(
                        "Connection test failed; API key was not saved. {}",
                        err.message
                    ),
                ));
            }
            credentials::set_secret(&keyring_account, key)
                .map_err(|e| CommandError::sanitized("credential_store", e, Some(key.as_str())))?;
            last_status = Some("connected".into());
            last_tested_at = Some(now.clone());
        } else if existing.is_none() {
            return Err(CommandError::new(
                "invalid",
                "API key is required when creating a provider connection",
            ));
        } else if !credentials::has_secret(&keyring_account) {
            return Err(CommandError::new(
                "missing_key",
                "No API key stored for this connection. Enter a key to save.",
            ));
        }
    } else {
        // Authless Local AI: prefer native Ollama health when applicable.
        if provider == "ollama" {
            let origin = resolved_base.as_deref().unwrap_or("http://127.0.0.1:11434");
            let cancel = CancellationToken::new();
            if let Err(err) = crate::ai::ollama_server_ready(origin, &cancel).await {
                return Err(CommandError::new(
                    "provider_test_failed",
                    format!(
                        "Ollama did not respond at {origin}. Is the app running? {}",
                        sanitize_error(&err.to_string(), None)
                    ),
                ));
            }
        } else {
            let probe = build_probe_config(
                &provider,
                "",
                resolved_base.as_deref(),
                model_default
                    .as_deref()
                    .or(existing.as_ref().and_then(|e| e.model_default.as_deref())),
            )?;
            if let Err(err) = health_check_key(&probe).await {
                return Err(CommandError::new(
                    "provider_test_failed",
                    format!(
                        "Local AI server did not respond. Is it running? {}",
                        err.message
                    ),
                ));
            }
        }
        last_status = Some("connected".into());
        last_tested_at = Some(now.clone());
    }

    let set_active = input
        .set_active
        .unwrap_or(existing.as_ref().map(|e| e.is_active).unwrap_or(true));
    let row = ProviderConnection {
        id: id.clone(),
        provider: provider.clone(),
        label: label.to_string(),
        base_url: resolved_base,
        model_default: model_default
            .or_else(|| existing.as_ref().and_then(|e| e.model_default.clone()))
            .or_else(|| descriptor.default_model_hint.map(|s| s.to_string())),
        keyring_account: keyring_account.clone(),
        is_active: false,
        last_status,
        last_tested_at,
        created_at: existing
            .as_ref()
            .map(|e| e.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
        provider_descriptor_id: Some(descriptor.id.to_string()),
        protocol_family: Some(descriptor.protocol_family.as_str().to_string()),
        auth_mode: Some(auth_mode.as_str().to_string()),
        endpoint_class: Some(endpoint_class.as_str().to_string()),
        api_version: existing.as_ref().and_then(|e| e.api_version.clone()),
        region: existing.as_ref().and_then(|e| e.region.clone()),
        deployment: existing.as_ref().and_then(|e| e.deployment.clone()),
        organization_id: existing.as_ref().and_then(|e| e.organization_id.clone()),
        project_id: existing.as_ref().and_then(|e| e.project_id.clone()),
        capability_profile_json: serde_json::to_string(&descriptor.capability_profile).ok(),
        capability_checked_at: None,
        model_catalog_checked_at: None,
        provider_preset_version: Some(descriptor.preset_version.to_string()),
        enabled: true,
    };

    let saved = {
        let mut db = state.db.lock();
        match db::upsert_provider_connection(&mut db, &row) {
            Ok(saved) => {
                if set_active {
                    db::set_active_provider_connection(&mut db, &saved.id)?
                } else {
                    saved
                }
            }
            Err(e) => {
                // Roll back a newly written key if metadata persist fails.
                if api_key.is_some() && existing.is_none() {
                    let _ = credentials::delete_secret(&keyring_account);
                }
                return Err(e.into());
            }
        }
    };
    Ok(to_view(saved))
}

#[tauri::command]
pub fn delete_provider_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let id = connection_id.trim();
    if id.is_empty() {
        return Err(CommandError::new("invalid", "connectionId is required"));
    }
    let account = {
        let db = state.db.lock();
        let row = db::get_provider_connection(&db, id)?;
        row.keyring_account
    };
    {
        let mut db = state.db.lock();
        db::delete_provider_connection(&mut db, id)?;
    }
    credentials::delete_secret(&account)
        .map_err(|e| CommandError::new("credential_store", sanitize_error(&e.to_string(), None)))?;
    Ok(())
}

#[tauri::command]
pub fn set_active_provider_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<ProviderConnectionView, CommandError> {
    state.require_profile()?;
    let id = connection_id.trim();
    if id.is_empty() {
        return Err(CommandError::new("invalid", "connectionId is required"));
    }
    let mut db = state.db.lock();
    let row = db::set_active_provider_connection(&mut db, id)?;
    Ok(to_view(row))
}

#[tauri::command]
pub async fn test_provider_connection(
    state: State<'_, AppState>,
    connection_id: Option<String>,
) -> Result<crate::ai::ProviderHealth, CommandError> {
    state.require_profile()?;
    let explicit_id = connection_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // When testing a specific connection, resolve it directly so an active
    // connection does not shadow the target (needed for test-before-activate).
    // Row reads happen under the lock; the keychain lookup happens after it.
    enum Lookup {
        One(Box<db::ProviderConnection>),
        Effective(Box<credentials::CredentialSources>),
    }
    let lookup = {
        let db = state.db.lock();
        match explicit_id {
            Some(ref id) => Lookup::One(Box::new(
                credentials::read_connection(&db, id)
                    .map_err(|e| CommandError::new("not_found", sanitize_error(&e, None)))?,
            )),
            None => Lookup::Effective(Box::new(credentials::read_credential_sources(&db, None))),
        }
    };
    let lookup = match lookup {
        Lookup::Effective(sources) => {
            Lookup::Effective(Box::new(credentials::complete_credential_sources(*sources)))
        }
        other => other,
    };
    let access = match lookup {
        Lookup::One(conn) => {
            let credentials = credentials::resolve_connection_secret(&conn)
                .map_err(|e| CommandError::new("not_found", sanitize_error(&e, None)))?;
            let route = credentials.access_route();
            ResolvedAiAccess {
                credentials,
                route,
            }
        }
        Lookup::Effective(sources) => credentials::resolve_ai_access(&sources),
    };
    let resolved = &access.credentials;
    let key = resolved.api_key.clone();
    let provider = create_provider_for_access(&access, None)
        .map_err(|e| CommandError::sanitized(e.code(), e, key.as_deref()))?;
    let cancel = CancellationToken::new();
    let health = provider.health_check(cancel).await;

    let status_id = resolved
        .active_connection_id
        .as_deref()
        .or(explicit_id.as_deref());

    match health {
        Ok(mut health) => {
            let name = AiProvider::display_name(provider.as_ref());
            health.message = format!("{name} — {}", health.message);
            if let Some(id) = status_id {
                let mut db = state.db.lock();
                let _ = db::update_provider_connection_status(&mut db, id, "connected");
            }
            Ok(health)
        }
        Err(e) => {
            if let Some(id) = status_id {
                let mut db = state.db.lock();
                let _ = db::update_provider_connection_status(&mut db, id, "error");
            }
            Err(CommandError::sanitized(e.code(), e, key.as_deref()))
        }
    }
}

#[tauri::command]
pub fn provider_key_hints() -> Vec<ProviderHint> {
    crate::ai::platform::list_consumer_descriptors()
        .into_iter()
        .map(|d| ProviderHint {
            id: d.id.to_string(),
            label: d.display_name.to_string(),
            key_placeholder: if d.default_auth_mode.requires_secret() {
                "Paste your API key".into()
            } else {
                "No API key required for Local AI".into()
            },
            default_model: d.default_model_hint.unwrap_or("").to_string(),
            docs_url: d.docs_url.map(|s| s.to_string()),
            supports_base_url: d.allows_endpoint_override,
            requires_api_key: d.default_auth_mode.requires_secret(),
            local: d.local,
            default_base_url: d.default_endpoint.map(|s| s.to_string()),
            auth_mode: d.default_auth_mode.as_str().to_string(),
            experimental: d.experimental,
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHint {
    pub id: String,
    pub label: String,
    pub key_placeholder: String,
    pub default_model: String,
    pub docs_url: Option<String>,
    pub supports_base_url: bool,
    pub requires_api_key: bool,
    pub local: bool,
    pub default_base_url: Option<String>,
    pub auth_mode: String,
    pub experimental: bool,
}

impl From<credentials::CredentialError> for CommandError {
    fn from(value: credentials::CredentialError) -> Self {
        Self::new("credential_store", sanitize_error(&value.to_string(), None))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedAuthStatus {
    pub signed_in: bool,
    pub adapter_ready: bool,
    pub user_id: Option<String>,
    pub expires_at: Option<f64>,
    pub configured: bool,
    /// Read-only plan presentation (server when available; no client plan writes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<credentials::HostedPlanPresentation>,
    /// Read-only catalog summaries for Settings UI (server is authoritative).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub available_plans: Vec<credentials::HostedPlanPresentation>,
}

/// Persist Supabase Auth session JSON in OS keyring (never SQLite / localStorage).
#[tauri::command]
pub fn store_hosted_auth_session(session_json: String) -> Result<HostedAuthStatus, CommandError> {
    credentials::store_session_json(&session_json)?;
    // Sync path: plan will refresh on next async status read.
    Ok(hosted_auth_status_sync_fallback())
}

#[tauri::command]
pub fn clear_hosted_auth_session() -> Result<HostedAuthStatus, CommandError> {
    credentials::clear_session()?;
    Ok(hosted_auth_status_sync_fallback())
}

fn hosted_auth_status_sync_fallback() -> HostedAuthStatus {
    let configured = crate::config::supabase_publishable_config().is_some();
    let catalog = if configured {
        credentials::contract_plan_catalog_summaries()
    } else {
        Vec::new()
    };
    match credentials::load_valid_session(None) {
        Some(session) => HostedAuthStatus {
            signed_in: true,
            adapter_ready: configured,
            user_id: Some(session.user_id),
            expires_at: Some(session.expires_at),
            configured,
            plan: None,
            available_plans: catalog,
        },
        None => HostedAuthStatus {
            signed_in: credentials::has_stored_session(),
            adapter_ready: false,
            user_id: None,
            expires_at: None,
            configured,
            plan: None,
            available_plans: catalog,
        },
    }
}

#[tauri::command]
pub async fn get_hosted_auth_status() -> HostedAuthStatus {
    hosted_auth_status_inner().await
}

async fn hosted_auth_status_inner() -> HostedAuthStatus {
    let configured = crate::config::supabase_publishable_config().is_some();
    let catalog = if configured {
        credentials::contract_plan_catalog_summaries()
    } else {
        Vec::new()
    };

    async fn resolve_plan() -> Option<credentials::HostedPlanPresentation> {
        credentials::fetch_server_plan_presentation().await.ok()
    }

    match credentials::load_valid_session(None) {
        Some(session) => HostedAuthStatus {
            signed_in: true,
            adapter_ready: configured,
            user_id: Some(session.user_id),
            expires_at: Some(session.expires_at),
            configured,
            plan: resolve_plan().await,
            available_plans: catalog,
        },
        None => HostedAuthStatus {
            signed_in: credentials::has_stored_session(),
            adapter_ready: false,
            user_id: None,
            expires_at: None,
            configured,
            plan: None,
            available_plans: catalog,
        },
    }
}
