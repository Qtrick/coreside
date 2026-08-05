//! App info and AI status commands.

use serde::Serialize;
use tauri::State;
use tokio_util::sync::CancellationToken;

use super::CommandError;
use crate::ai::{create_provider_for_access, model_catalog, ModelCatalog};
use crate::config::PublicAiStatus;
use crate::credentials::{complete_credential_sources, read_credential_sources, resolve_ai_access};
use crate::db;
use crate::state::AppState;

const APP_DESCRIPTION: &str =
    "An AI-native personal software environment that begins as a chatbot and builds tools inside itself.";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub prompt_version: String,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "Coreside".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: APP_DESCRIPTION.into(),
        prompt_version: crate::ai::PROMPT_VERSION.into(),
    }
}

#[tauri::command]
pub fn get_ai_status(state: State<'_, AppState>) -> PublicAiStatus {
    if !state.profile_ready() {
        return PublicAiStatus {
            provider: "none".into(),
            model: "none".into(),
            key_detected: false,
            status: "database_unavailable".into(),
            message: Some(
                "Coreside needs Recovery before AI status is available.".into(),
            ),
            base_url: String::new(),
            env_path: None,
            source: "none".into(),
            active_connection_id: None,
            access_mode: Some("unavailable".into()),
            consumer_display_name: None,
            user_facing_status: Some("Recovery required".into()),
            disclosure: None,
        };
    }
    let _ = state.reload_config();
    // Read DB rows under the lock; keychain lookups happen after it is released.
    let sources = complete_credential_sources({
        let db = state.db.lock();
        read_credential_sources(&db, None)
    });
    let access = resolve_ai_access(&sources);
    let resolved = &access.credentials;
    let developer_mode = {
        let db = state.db.lock();
        db::get_settings(&db)
            .ok()
            .and_then(|s| s.get("developerMode").cloned())
            .map(|v| v == "true" || v == "1" || v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false)
    };
    let hosted_connected = sources.hosted_adapter_connected();

    let has_key = resolved.has_api_key();
    let is_local =
        resolved.is_authless_local() || crate::credentials::is_authless_local_provider(&resolved.provider);
    let presentation = crate::ai::resolve_access_presentation(
        &resolved.source,
        &resolved.provider,
        has_key,
        hosted_connected,
        developer_mode,
        is_local,
    );

    let (status, message) = if presentation.available {
        let msg = if presentation.disclosure.show_credential_source {
            match resolved.source.as_str() {
                "connection" => Some("Connected with a secure provider credential.".to_string()),
                "env" => Some("Development environment credential (Developer Mode).".to_string()),
                _ => None,
            }
        } else {
            None
        };
        ("ready", msg)
    } else {
        (
            "missing_key",
            Some("Connect your own AI provider or configure local AI in Settings.".to_string()),
        )
    };

    resolved.to_app_config().public_ai_status_with_presentation(
        status,
        message,
        &resolved.source,
        resolved.active_connection_id.clone(),
        Some(presentation),
    )
}

#[tauri::command]
pub fn get_model_catalog(state: State<'_, AppState>) -> Result<ModelCatalog, CommandError> {
    state.require_profile()?;
    // Read the database half under the lock, then release it: the keychain
    // lookup inside `resolve_from_sources` is a blocking OS call.
    let sources = complete_credential_sources({
        let db = state.db.lock();
        read_credential_sources(&db, None)
    });
    let access = resolve_ai_access(&sources);
    let resolved = &access.credentials;
    let developer_mode = {
        let db = state.db.lock();
        db::get_settings(&db)
            .ok()
            .and_then(|s| s.get("developerMode").cloned())
            .map(|v| v == "true" || v == "1" || v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false)
    };
    let hosted_connected = sources.hosted_adapter_connected();
    let presentation = crate::ai::resolve_access_presentation(
        &resolved.source,
        &resolved.provider,
        resolved.has_api_key(),
        hosted_connected,
        developer_mode,
        resolved.is_authless_local()
            || crate::credentials::is_authless_local_provider(&resolved.provider),
    );
    let selected = {
        let db = state.db.lock();
        db::get_settings(&db)?
            .get("preferredModel")
            .cloned()
            .unwrap_or_else(|| "auto".to_string())
    };
    if !presentation.disclosure.show_provider_catalog {
        // Consumer-safe stub — no upstream model slugs.
        return Ok(crate::ai::auto_only_catalog(&selected));
    }
    Ok(model_catalog(&resolved.to_app_config(), &selected))
}

#[tauri::command]
pub async fn test_ai_connection(
    state: State<'_, AppState>,
) -> Result<crate::ai::ProviderHealth, CommandError> {
    state.require_profile()?;
    // Read the database half under the lock, then release it: the keychain
    // lookup inside `resolve_from_sources` is a blocking OS call.
    let sources = complete_credential_sources({
        let db = state.db.lock();
        read_credential_sources(&db, None)
    });
    let access = resolve_ai_access(&sources);
    let resolved = &access.credentials;
    let key = resolved.api_key.clone();
    let _config = resolved.to_app_config();

    let provider = create_provider_for_access(&access, None)
        .map_err(|e| CommandError::sanitized(e.code(), e, key.as_deref()))?;

    let cancel = CancellationToken::new();
    let mut health = provider
        .health_check(cancel)
        .await
        .map_err(|e| CommandError::sanitized(e.code(), e, key.as_deref()))?;
    let developer_mode = {
        let db = state.db.lock();
        db::get_settings(&db)
            .ok()
            .and_then(|s| s.get("developerMode").cloned())
            .map(|v| v == "true" || v == "1" || v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false)
    };
    let hosted_connected = sources.hosted_adapter_connected();
    let presentation = crate::ai::resolve_access_presentation(
        &resolved.source,
        &resolved.provider,
        resolved.has_api_key(),
        hosted_connected,
        developer_mode,
        resolved.is_authless_local()
            || crate::credentials::is_authless_local_provider(&resolved.provider),
    );
    if presentation.disclosure.show_provider_identity {
        let provider_name = crate::ai::AiProvider::display_name(provider.as_ref());
        health.message = format!("{provider_name} — {}", health.message);
    } else {
        // Consumer-safe: no upstream provider or model-count leakage.
        health.message = "Connection succeeded.".into();
        health.models.clear();
    }
    Ok(health)
}
