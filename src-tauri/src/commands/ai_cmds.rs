//! App info and AI status commands.

use serde::Serialize;
use tauri::State;
use tokio_util::sync::CancellationToken;

use super::CommandError;
use crate::ai::{create_provider, model_catalog, ModelCatalog};
use crate::config::PublicAiStatus;
use crate::credentials::resolve_credentials;
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
    let _ = state.reload_config();
    let resolved = {
        let db = state.db.lock();
        resolve_credentials(&db, None)
    };

    let (status, message) = if resolved.has_api_key() {
        let msg = match resolved.source.as_str() {
            "connection" => Some("Connected with a secure provider credential.".to_string()),
            "env" => Some("Using development environment credential.".to_string()),
            _ => None,
        };
        ("ready", msg)
    } else if resolved.provider.eq_ignore_ascii_case("mock") {
        (
            "ready",
            Some("Using mock AI provider (no API key required).".to_string()),
        )
    } else {
        (
            "missing_key",
            Some(
                "Connect an AI provider in Settings to chat. Your key stays on this device."
                    .to_string(),
            ),
        )
    };

    resolved.to_app_config().public_ai_status_with_source(
        status,
        message,
        &resolved.source,
        resolved.active_connection_id.clone(),
    )
}

#[tauri::command]
pub fn get_model_catalog(state: State<'_, AppState>) -> Result<ModelCatalog, CommandError> {
    let resolved = {
        let db = state.db.lock();
        resolve_credentials(&db, None)
    };
    let selected = {
        let db = state.db.lock();
        db::get_settings(&db)?
            .get("preferredModel")
            .cloned()
            .unwrap_or_else(|| "auto".to_string())
    };
    Ok(model_catalog(&resolved.to_app_config(), &selected))
}

#[tauri::command]
pub async fn test_ai_connection(
    state: State<'_, AppState>,
) -> Result<crate::ai::ProviderHealth, CommandError> {
    let resolved = {
        let db = state.db.lock();
        resolve_credentials(&db, None)
    };
    let key = resolved.api_key.clone();
    let config = resolved.to_app_config();

    let provider = create_provider(&config).map_err(|e| {
        CommandError::sanitized(e.code(), e, key.as_deref())
    })?;

    let cancel = CancellationToken::new();
    let mut health = provider.health_check(cancel).await.map_err(|e| {
        CommandError::sanitized(e.code(), e, key.as_deref())
    })?;
    let provider_name = crate::ai::AiProvider::display_name(provider.as_ref());
    health.message = format!("{provider_name} — {}", health.message);
    Ok(health)
}
