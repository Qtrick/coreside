//! Settings commands.

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use super::CommandError;
use crate::db;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub sidebar_collapsed: bool,
}

fn settings_from_map(map: &std::collections::HashMap<String, String>) -> AppSettings {
    let theme = map
        .get("theme")
        .cloned()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "system".to_string());

    let sidebar_collapsed = map
        .get("sidebarCollapsed")
        .or_else(|| map.get("sidebar_collapsed"))
        .map(|v| {
            let lower = v.trim().to_lowercase();
            lower == "true" || lower == "1" || lower == "yes"
        })
        .unwrap_or(false);

    AppSettings {
        theme,
        sidebar_collapsed,
    }
}

fn value_to_storage(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, CommandError> {
    let db = state.db.lock();
    let map = db::get_settings(&db)?;
    Ok(settings_from_map(&map))
}

#[tauri::command]
pub fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: Value,
) -> Result<AppSettings, CommandError> {
    if key.trim().is_empty() {
        return Err(CommandError::new("invalid", "Setting key cannot be empty"));
    }
    // Never persist API keys via settings IPC.
    let lower = key.to_lowercase();
    if lower.contains("api_key") || lower.contains("apikey") || lower.contains("secret") {
        return Err(CommandError::new(
            "forbidden",
            "Secrets cannot be stored via settings; use environment variables.",
        ));
    }
    let stored = value_to_storage(&value);
    let mut db = state.db.lock();
    db::set_setting(&mut db, &key, &stored)?;
    let map = db::get_settings(&db)?;
    Ok(settings_from_map(&map))
}
