//! Settings commands — Base Settings KV + Added Settings CRUD + dock icon.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};

use super::CommandError;
use crate::ai::{
    normalize_hex_or_none, normalize_setting_kv, parse_wallpaper_setting, WallpaperConfig,
};
use crate::branding;
use crate::db;
use crate::security::{assert_not_protected, is_protected};
use crate::settings::{AddedSettingRecord, UpsertAddedSettingInput};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub sidebar_collapsed: bool,
    /// `"auto"` or a concrete provider model id.
    pub preferred_model: String,
    /// Dock icon preference: `auto` | `dark` | `light`.
    pub dock_icon: String,
    /// Accent colors (hex). Light/dark variants for when Appearance theme resolves.
    pub accent_primary_light: String,
    pub accent_primary_dark: String,
    pub accent_secondary_light: String,
    pub accent_secondary_dark: String,
    /// Background / surface colors (hex).
    pub background_light: String,
    pub background_dark: String,
    pub surface_light: String,
    pub surface_dark: String,
    pub surface_muted_light: String,
    pub surface_muted_dark: String,
    pub border_light: String,
    pub border_dark: String,
    pub text_primary_light: String,
    pub text_primary_dark: String,
    pub text_secondary_light: String,
    pub text_secondary_dark: String,
    /// Declarative live wallpaper (`none`, `matrix`, …).
    pub wallpaper: WallpaperConfig,
    /// Schema wallpaper JSON for media / gradient backgrounds (validated).
    pub wallpaper_json: Option<String>,
    /// Base Setting: show sanitized agent Action Log (default off).
    pub action_log_enabled: bool,
    /// Base Setting: `off` | `always` | `intelligent`.
    pub action_log_mode: String,
    /// Base Setting: safe search level for local research.
    pub safe_search: String,
    /// Base Setting: Developer Mode (technical AI details). Default off.
    pub developer_mode: bool,
}

const DEFAULT_ACCENT_PRIMARY_LIGHT: &str = "#2f8f63";
const DEFAULT_ACCENT_PRIMARY_DARK: &str = "#69c994";
const DEFAULT_ACCENT_SECONDARY_LIGHT: &str = "#d38b3d";
const DEFAULT_ACCENT_SECONDARY_DARK: &str = "#e0a158";
const DEFAULT_BACKGROUND_LIGHT: &str = "#f5f6f1";
const DEFAULT_BACKGROUND_DARK: &str = "#141714";
const DEFAULT_SURFACE_LIGHT: &str = "#ffffff";
const DEFAULT_SURFACE_DARK: &str = "#1c201c";
const DEFAULT_SURFACE_MUTED_LIGHT: &str = "#ecefe8";
const DEFAULT_SURFACE_MUTED_DARK: &str = "#242a24";
const DEFAULT_BORDER_LIGHT: &str = "#d8d8d4";
const DEFAULT_BORDER_DARK: &str = "#3a3a3a";
const DEFAULT_TEXT_PRIMARY_LIGHT: &str = "#1d211c";
const DEFAULT_TEXT_PRIMARY_DARK: &str = "#eef2ec";
const DEFAULT_TEXT_SECONDARY_LIGHT: &str = "#687066";
const DEFAULT_TEXT_SECONDARY_DARK: &str = "#a6afa3";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateChangeTargetsResult {
    pub ok: bool,
    pub rejected: Vec<String>,
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

    let preferred_model = map
        .get("preferredModel")
        .or_else(|| map.get("preferred_model"))
        .cloned()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "auto".to_string());

    let dock_icon = map
        .get("dockIcon")
        .or_else(|| map.get("dock_icon"))
        .cloned()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| matches!(s.as_str(), "auto" | "dark" | "light"))
        .unwrap_or_else(|| "auto".to_string());

    let color = |key: &str, alt: &str, default: &str| {
        map.get(key)
            .or_else(|| map.get(alt))
            .and_then(|s| normalize_hex_or_none(s))
            .unwrap_or_else(|| default.to_string())
    };

    AppSettings {
        theme,
        sidebar_collapsed,
        preferred_model,
        dock_icon,
        accent_primary_light: color(
            "accentPrimaryLight",
            "accent_primary_light",
            DEFAULT_ACCENT_PRIMARY_LIGHT,
        ),
        accent_primary_dark: color(
            "accentPrimaryDark",
            "accent_primary_dark",
            DEFAULT_ACCENT_PRIMARY_DARK,
        ),
        accent_secondary_light: color(
            "accentSecondaryLight",
            "accent_secondary_light",
            DEFAULT_ACCENT_SECONDARY_LIGHT,
        ),
        accent_secondary_dark: color(
            "accentSecondaryDark",
            "accent_secondary_dark",
            DEFAULT_ACCENT_SECONDARY_DARK,
        ),
        background_light: color(
            "backgroundLight",
            "background_light",
            DEFAULT_BACKGROUND_LIGHT,
        ),
        background_dark: color(
            "backgroundDark",
            "background_dark",
            DEFAULT_BACKGROUND_DARK,
        ),
        surface_light: color("surfaceLight", "surface_light", DEFAULT_SURFACE_LIGHT),
        surface_dark: color("surfaceDark", "surface_dark", DEFAULT_SURFACE_DARK),
        surface_muted_light: color(
            "surfaceMutedLight",
            "surface_muted_light",
            DEFAULT_SURFACE_MUTED_LIGHT,
        ),
        surface_muted_dark: color(
            "surfaceMutedDark",
            "surface_muted_dark",
            DEFAULT_SURFACE_MUTED_DARK,
        ),
        border_light: color("borderLight", "border_light", DEFAULT_BORDER_LIGHT),
        border_dark: color("borderDark", "border_dark", DEFAULT_BORDER_DARK),
        text_primary_light: color(
            "textPrimaryLight",
            "text_primary_light",
            DEFAULT_TEXT_PRIMARY_LIGHT,
        ),
        text_primary_dark: color(
            "textPrimaryDark",
            "text_primary_dark",
            DEFAULT_TEXT_PRIMARY_DARK,
        ),
        text_secondary_light: color(
            "textSecondaryLight",
            "text_secondary_light",
            DEFAULT_TEXT_SECONDARY_LIGHT,
        ),
        text_secondary_dark: color(
            "textSecondaryDark",
            "text_secondary_dark",
            DEFAULT_TEXT_SECONDARY_DARK,
        ),
        wallpaper: map
            .get("wallpaper")
            .map(|s| parse_wallpaper_setting(s))
            .unwrap_or_default(),
        wallpaper_json: map
            .get("wallpaperJson")
            .or_else(|| map.get("wallpaper_json"))
            .cloned()
            .filter(|s| !s.trim().is_empty()),
        action_log_enabled: {
            let mode = db::ActionLogMode::parse(
                map.get("actionLogMode")
                    .or_else(|| map.get("action_log_mode"))
                    .map(|s| s.as_str())
                    .unwrap_or_else(|| {
                        map.get("actionLogEnabled")
                            .or_else(|| map.get("action_log_enabled"))
                            .map(|s| s.as_str())
                            .unwrap_or("off")
                    }),
            );
            mode.collects()
        },
        action_log_mode: {
            if let Some(mode) = map
                .get("actionLogMode")
                .or_else(|| map.get("action_log_mode"))
            {
                db::ActionLogMode::parse(mode).as_str().to_string()
            } else if map
                .get("actionLogEnabled")
                .or_else(|| map.get("action_log_enabled"))
                .map(|v| {
                    let lower = v.trim().to_lowercase();
                    lower == "true" || lower == "1" || lower == "yes"
                })
                .unwrap_or(false)
            {
                "always".into()
            } else {
                "off".into()
            }
        },
        safe_search: map
            .get("safeSearch")
            .or_else(|| map.get("safe_search"))
            .cloned()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| matches!(s.as_str(), "strict" | "standard" | "off"))
            .unwrap_or_else(|| "standard".to_string()),
        developer_mode: map
            .get("developerMode")
            .or_else(|| map.get("developer_mode"))
            .map(|v| {
                let lower = v.trim().to_lowercase();
                lower == "true" || lower == "1" || lower == "yes"
            })
            .unwrap_or(false),
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
            "Secrets cannot be stored via settings; use secure provider credentials.",
        ));
    }
    // Exa budget / profile / usage are dedicated commands — block generic bypass.
    if matches!(
        key.trim(),
        "exaMonthlyBudgetUsd"
            | "exaBudgetSoftPercent"
            | "exaBudgetCriticalPercent"
            | "exaBudgetHardPercent"
            | "searchProfile"
    ) {
        return Err(CommandError::new(
            "forbidden",
            "Use Exa budget / search profile commands; this setting cannot be changed via set_setting.",
        ));
    }
    let stored = value_to_storage(&value);
    // Reject values that look like provider API keys.
    let stored_lower = stored.to_lowercase();
    if stored_lower.contains("sk-ant-")
        || stored_lower.contains("sk-or-v1-")
        || stored_lower.contains("sk-proj-")
        || stored_lower.starts_with("sk-")
        || stored_lower.starts_with("aizas")
        || (stored_lower.contains("bearer ") && stored.len() > 20)
    {
        return Err(CommandError::new(
            "forbidden",
            "Secrets cannot be stored via settings; use secure provider credentials.",
        ));
    }
    let stored = normalize_setting_kv(&key, &stored)
        .map_err(|e| CommandError::new("invalid", e))?;
    let mut db = state.db.lock();
    db::set_setting(&mut db, &key, &stored)?;
    // Keep Action Log boolean + mode keys in sync for older readers.
    let key_trim = key.trim();
    if key_trim == "actionLogMode" || key_trim == "action_log_mode" {
        let mode = db::ActionLogMode::parse(&stored);
        let _ = db::set_setting(
            &mut db,
            "actionLogEnabled",
            if mode.collects() { "true" } else { "false" },
        );
        let _ = db::set_setting(&mut db, "actionLogMode", mode.as_str());
    } else if key_trim == "actionLogEnabled" || key_trim == "action_log_enabled" {
        let mode = if stored == "true" {
            db::ActionLogMode::Always
        } else {
            db::ActionLogMode::Off
        };
        let _ = db::set_setting(&mut db, "actionLogMode", mode.as_str());
    }
    let map = db::get_settings(&db)?;
    Ok(settings_from_map(&map))
}

#[tauri::command]
pub fn list_added_settings(
    state: State<'_, AppState>,
    owner_tool_id: Option<String>,
) -> Result<Vec<AddedSettingRecord>, CommandError> {
    let db = state.db.lock();
    Ok(db::list_added_settings(
        &db,
        owner_tool_id.as_deref(),
    )?)
}

#[tauri::command]
pub fn upsert_added_setting(
    state: State<'_, AppState>,
    input: UpsertAddedSettingInput,
) -> Result<AddedSettingRecord, CommandError> {
    let mut db = state.db.lock();
    Ok(db::upsert_added_setting(&mut db, &input)?)
}

#[tauri::command]
pub fn delete_added_setting(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    Ok(db::delete_added_setting(&mut db, &id)?)
}

/// Rejects any protected / `core.*` ids. Used before applying tool or resource changes.
#[tauri::command]
pub fn validate_change_targets(ids: Vec<String>) -> Result<ValidateChangeTargetsResult, CommandError> {
    let mut rejected = Vec::new();
    for id in &ids {
        if is_protected(id) {
            rejected.push(id.clone());
        }
    }
    // Touch list surface so protected id inventory stays linked into the binary.
    let _ = crate::security::list_protected_ids();
    let _ = crate::security::PROTECTED_IDS.len();
    Ok(ValidateChangeTargetsResult {
        ok: rejected.is_empty(),
        rejected,
    })
}

/// Apply the macOS dock icon for a preference (`auto` | `dark` | `light`).
/// When `auto`, `os_is_dark` selects which of the two tiles to use.
#[tauri::command]
pub fn set_dock_icon(
    app: AppHandle,
    preference: String,
    os_is_dark: bool,
) -> Result<(), CommandError> {
    branding::set_dock_icon(&app, &preference, os_is_dark)
        .map_err(|e| CommandError::new("dock_icon", e))
}

/// Back-compat: OS appearance with preference treated as `auto`.
#[tauri::command]
pub fn set_dock_icon_for_os_appearance(
    app: AppHandle,
    is_dark: bool,
) -> Result<(), CommandError> {
    branding::set_dock_icon_for_os_appearance(&app, is_dark)
        .map_err(|e| CommandError::new("dock_icon", e))
}

/// Helper used by apply paths — returns Err when any id is protected.
pub fn reject_protected_ids(ids: &[&str]) -> Result<(), CommandError> {
    for id in ids {
        if let Err(msg) = assert_not_protected(id) {
            return Err(CommandError::new("forbidden", msg));
        }
    }
    Ok(())
}
