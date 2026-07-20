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
use crate::ai::{create_provider, AiProvider};
use crate::config::AppConfig;
use crate::credentials::{self, defaults_for, resolve_connection_by_id, resolve_credentials};
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
    let has_key = credentials::has_secret(&row.keyring_account);
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
    }
}

fn normalize_provider(raw: &str) -> Result<String, CommandError> {
    let p = raw.trim().to_lowercase();
    match p.as_str() {
        "gemini" | "openai" | "anthropic" | "openrouter" | "compatible" => Ok(p),
        "claude" => Ok("anthropic".into()),
        _ => Err(CommandError::new(
            "invalid",
            format!("Unsupported provider '{raw}'. Choose gemini, openai, anthropic, openrouter, or compatible."),
        )),
    }
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
    if provider == "compatible" && base_url.trim().is_empty() {
        return Err(CommandError::new(
            "invalid",
            "Custom OpenAI-compatible providers require a base URL",
        ));
    }
    let model = model_default
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(default_model.as_str())
        .to_string();
    Ok(AppConfig {
        provider: provider.to_string(),
        api_key: Some(api_key.to_string()),
        model,
        base_url,
        log_level: "info".into(),
        env_path: None,
    })
}

async fn health_check_key(config: &AppConfig) -> Result<(), CommandError> {
    let key = config.api_key.clone();
    let provider = create_provider(config).map_err(|e| {
        CommandError::sanitized(e.code(), e, key.as_deref())
    })?;
    let cancel = CancellationToken::new();
    provider.health_check(cancel).await.map_err(|e| {
        CommandError::sanitized(e.code(), e, key.as_deref())
    })?;
    Ok(())
}

#[tauri::command]
pub fn list_provider_connections(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderConnectionView>, CommandError> {
    let db = state.db.lock();
    let rows = db::list_provider_connections(&db)?;
    Ok(rows.into_iter().map(to_view).collect())
}

#[tauri::command]
pub async fn upsert_provider_connection(
    state: State<'_, AppState>,
    input: UpsertProviderConnectionInput,
) -> Result<ProviderConnectionView, CommandError> {
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

    let mut last_status = existing.as_ref().and_then(|e| e.last_status.clone());
    let mut last_tested_at = existing.as_ref().and_then(|e| e.last_tested_at.clone());
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(ref key) = api_key {
        // Test-then-save: never persist a new key until health_check succeeds.
        let probe = build_probe_config(
            &provider,
            key,
            base_url.as_deref().or(existing.as_ref().and_then(|e| e.base_url.as_deref())),
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
        credentials::set_secret(&keyring_account, key).map_err(|e| {
            CommandError::sanitized("credential_store", e, Some(key.as_str()))
        })?;
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

    let set_active = input
        .set_active
        .unwrap_or(existing.as_ref().map(|e| e.is_active).unwrap_or(true));
    let row = ProviderConnection {
        id: id.clone(),
        provider,
        label: label.to_string(),
        base_url: base_url.or_else(|| existing.as_ref().and_then(|e| e.base_url.clone())),
        model_default: model_default
            .or_else(|| existing.as_ref().and_then(|e| e.model_default.clone())),
        keyring_account: keyring_account.clone(),
        is_active: false,
        last_status,
        last_tested_at,
        created_at: existing
            .as_ref()
            .map(|e| e.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
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
    credentials::delete_secret(&account).map_err(|e| {
        CommandError::new("credential_store", sanitize_error(&e.to_string(), None))
    })?;
    Ok(())
}

#[tauri::command]
pub fn set_active_provider_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<ProviderConnectionView, CommandError> {
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
    let explicit_id = connection_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // When testing a specific connection, resolve it directly so an active
    // connection does not shadow the target (needed for test-before-activate).
    let resolved = {
        let db = state.db.lock();
        if let Some(ref id) = explicit_id {
            resolve_connection_by_id(&db, id).map_err(|e| {
                CommandError::new("not_found", sanitize_error(&e, None))
            })?
        } else {
            resolve_credentials(&db, None)
        }
    };
    let key = resolved.api_key.clone();
    let config = resolved.to_app_config();
    let provider = create_provider(&config).map_err(|e| {
        CommandError::sanitized(e.code(), e, key.as_deref())
    })?;
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
    vec![
        ProviderHint {
            id: "gemini".into(),
            label: "Google Gemini".into(),
            key_placeholder: "Usually starts with AIza…".into(),
            default_model: crate::config::DEFAULT_GEMINI_MODEL.into(),
            docs_url: Some("https://aistudio.google.com/apikey".into()),
            supports_base_url: false,
        },
        ProviderHint {
            id: "openai".into(),
            label: "OpenAI".into(),
            key_placeholder: "Usually starts with sk-… or sk-proj-…".into(),
            default_model: crate::config::DEFAULT_OPENAI_MODEL.into(),
            docs_url: Some("https://platform.openai.com/api-keys".into()),
            supports_base_url: false,
        },
        ProviderHint {
            id: "anthropic".into(),
            label: "Anthropic".into(),
            key_placeholder: "Usually starts with sk-ant-…".into(),
            default_model: crate::config::DEFAULT_ANTHROPIC_MODEL.into(),
            docs_url: Some("https://console.anthropic.com/settings/keys".into()),
            supports_base_url: false,
        },
        ProviderHint {
            id: "openrouter".into(),
            label: "OpenRouter".into(),
            key_placeholder: "Usually starts with sk-or-v1-…".into(),
            default_model: crate::config::DEFAULT_OPENROUTER_MODEL.into(),
            docs_url: Some("https://openrouter.ai/keys".into()),
            supports_base_url: false,
        },
        ProviderHint {
            id: "compatible".into(),
            label: "Custom OpenAI-compatible".into(),
            key_placeholder: "Enter the API key required by this endpoint".into(),
            default_model: "gpt-4.1-mini".into(),
            docs_url: None,
            supports_base_url: true,
        },
    ]
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
}

/// Persist Supabase Auth session JSON in OS keyring (never SQLite / localStorage).
#[tauri::command]
pub fn store_hosted_auth_session(session_json: String) -> Result<HostedAuthStatus, CommandError> {
    credentials::store_session_json(&session_json)?;
    Ok(hosted_auth_status_inner())
}

#[tauri::command]
pub fn clear_hosted_auth_session() -> Result<HostedAuthStatus, CommandError> {
    credentials::clear_session()?;
    Ok(hosted_auth_status_inner())
}

#[tauri::command]
pub fn get_hosted_auth_status() -> HostedAuthStatus {
    hosted_auth_status_inner()
}

fn hosted_auth_status_inner() -> HostedAuthStatus {
    let configured = crate::config::supabase_publishable_config().is_some();
    match credentials::load_valid_session(None) {
        Some(session) => HostedAuthStatus {
            signed_in: true,
            adapter_ready: configured,
            user_id: Some(session.user_id),
            expires_at: Some(session.expires_at),
            configured,
        },
        None => HostedAuthStatus {
            signed_in: false,
            adapter_ready: false,
            user_id: None,
            expires_at: None,
            configured,
        },
    }
}
