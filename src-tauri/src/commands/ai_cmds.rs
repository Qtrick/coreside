//! App info and AI status commands.

use serde::Serialize;
use tauri::State;
use tokio_util::sync::CancellationToken;

use super::CommandError;
use crate::ai::create_provider;
use crate::config::PublicAiStatus;
use crate::state::AppState;

const APP_DESCRIPTION: &str =
    "An AI-native personal software environment that begins as a chatbot and builds tools inside itself.";

const MISSING_KEY_MESSAGE: &str =
    "Add AI_API_KEY (or GEMINI_API_KEY) to the project .env file, then restart npm run dev.";

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
    let (status, message) = if state.config.has_api_key() {
        ("ready", None)
    } else if state.config.provider.eq_ignore_ascii_case("mock") {
        // Mock provider is usable without a key.
        ("ready", Some("Using mock AI provider (no API key required).".to_string()))
    } else {
        ("missing_key", Some(MISSING_KEY_MESSAGE.to_string()))
    };
    state.config.public_ai_status(status, message)
}

#[tauri::command]
pub async fn test_ai_connection(
    state: State<'_, AppState>,
) -> Result<crate::ai::ProviderHealth, CommandError> {
    let config = state.config.clone();
    let key = config.api_key.clone();

    // Never silently fall back to mock when a real provider is selected but unconfigured —
    // that would report a false-positive "connection successful".
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
