//! Settings commands — Base Settings KV + Added Settings CRUD + dock icon.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State, WebviewWindow};

use super::CommandError;
use crate::ai::{
    normalize_hex_or_none, normalize_setting_kv, parse_wallpaper_setting, WallpaperConfig,
};
use crate::branding::{self, DockAuthority, DockIconCommitResult, DockIconConfig};
use crate::db;
use crate::security::{assert_not_protected, is_protected};
use crate::settings::{AddedSettingRecord, UpsertAddedSettingInput};
use crate::state::AppState;
use crate::windows;

const DEFAULT_WALLPAPER_JSON: &str = r#"{"kind":"none"}"#;

/// Atomic workspace appearance update (wallpaper pair and/or interface transparency).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetWorkspaceAppearanceInput {
    /// `Some` applies/clears the wallpaperJson + wallpaper pair; `None` leaves both unchanged.
    #[serde(default)]
    pub wallpaper_json: Option<String>,
    /// `Some` sets interface transparency; `None` leaves it unchanged.
    #[serde(default)]
    pub interface_transparency: Option<u8>,
}

/// Resolve wallpaperJson input into normalized `(wallpaperJson, wallpaper)` storage values.
/// Matches frontend `applyWorkspaceWallpaper` / `parseWallpaperJson` semantics.
fn resolve_wallpaper_pair(raw: &str) -> Result<(String, String), String> {
    let trimmed = raw.trim();
    let default_wallpaper = normalize_setting_kv("wallpaper", DEFAULT_WALLPAPER_JSON)?;

    if trimmed.is_empty() {
        return Ok((String::new(), default_wallpaper));
    }

    let value: Value = serde_json::from_str(trimmed)
        .map_err(|_| "wallpaperJson must be valid JSON".to_string())?;
    let Some(obj) = value.as_object() else {
        return Err("wallpaperJson must be a JSON object".into());
    };

    let schema_version = obj.get("schemaVersion").and_then(|v| v.as_str());
    let schema_type = obj.get("type").and_then(|v| v.as_str());
    if schema_version == Some("1") && schema_type.is_some() {
        let wallpaper_json = normalize_setting_kv("wallpaperJson", trimmed)?;
        if wallpaper_json.is_empty() {
            return Ok((String::new(), default_wallpaper));
        }
        return Ok((wallpaper_json, default_wallpaper));
    }

    if let Some(kind) = obj.get("kind").and_then(|v| v.as_str()) {
        if kind.eq_ignore_ascii_case("none") {
            return Ok((String::new(), default_wallpaper));
        }
        let wallpaper = normalize_setting_kv("wallpaper", trimmed)?;
        let wallpaper_json = normalize_setting_kv("wallpaperJson", "")?;
        return Ok((wallpaper_json, wallpaper));
    }

    // Mutation path must not silently clear on unrecognized payloads.
    Err("wallpaperJson must include schemaVersion/type or a legacy kind".into())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub sidebar_collapsed: bool,
    /// `"auto"` or a concrete provider model id.
    pub preferred_model: String,
    /// Versioned Dock preference (Follow macOS or manual Classic/Split).
    pub dock_icon: DockIconConfig,
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
    /// Interface transparency percent (0–60). Default 20.
    pub interface_transparency: u8,
    /// Adaptive window sizing: `smart` | `ask` | `off`.
    pub adaptive_window_sizing: String,
    /// Preferred chat/tool split ratio (0.28–0.72).
    pub chat_tool_split_ratio: f64,
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
        .map(|s| branding::parse_dock_icon_setting(s))
        .unwrap_or_else(DockIconConfig::follow_macos);

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
        background_dark: color("backgroundDark", "background_dark", DEFAULT_BACKGROUND_DARK),
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
        interface_transparency: map
            .get("interfaceTransparency")
            .or_else(|| map.get("interface_transparency"))
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(|n| n.min(60))
            .unwrap_or(20),
        adaptive_window_sizing: map
            .get("adaptiveWindowSizing")
            .or_else(|| map.get("adaptive_window_sizing"))
            .map(|s| s.trim().to_lowercase())
            .filter(|s| matches!(s.as_str(), "smart" | "ask" | "off"))
            .unwrap_or_else(|| "smart".to_string()),
        chat_tool_split_ratio: map
            .get("chatToolSplitRatio")
            .or_else(|| map.get("chat_tool_split_ratio"))
            .and_then(|s| s.trim().parse::<f64>().ok())
            .map(|n| n.clamp(0.28, 0.72))
            .unwrap_or(0.5),
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
    state.require_profile()?;
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
    state.require_profile()?;
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
    // Wallpaper pair + transparency must stay atomic via set_workspace_appearance.
    if matches!(
        key.trim(),
        "wallpaper"
            | "wallpaperJson"
            | "wallpaper_json"
            | "interfaceTransparency"
            | "interface_transparency"
    ) {
        return Err(CommandError::new(
            "forbidden",
            "Use set_workspace_appearance; wallpaper and interface transparency cannot be changed via set_setting.",
        ));
    }
    // Dock icon is a single-authority native+DB transaction.
    if matches!(key.trim(), "dockIcon" | "dock_icon") {
        return Err(CommandError::new(
            "forbidden",
            "Use commit_dock_icon_preference; Dock icon cannot be changed via set_setting.",
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
    let stored =
        normalize_setting_kv(&key, &stored).map_err(|e| CommandError::new("invalid", e))?;
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
pub fn set_workspace_appearance(
    state: State<'_, AppState>,
    input: SetWorkspaceAppearanceInput,
) -> Result<AppSettings, CommandError> {
    state.require_profile()?;
    if input.wallpaper_json.is_none() && input.interface_transparency.is_none() {
        return Err(CommandError::new(
            "invalid",
            "set_workspace_appearance requires wallpaperJson and/or interfaceTransparency",
        ));
    }

    // Validate all proposed writes before touching the DB.
    let mut pending: Vec<(String, String)> = Vec::new();
    if let Some(raw) = &input.wallpaper_json {
        let (wallpaper_json, wallpaper) =
            resolve_wallpaper_pair(raw).map_err(|e| CommandError::new("invalid", e))?;
        pending.push(("wallpaperJson".into(), wallpaper_json));
        pending.push(("wallpaper".into(), wallpaper));
    }
    if let Some(n) = input.interface_transparency {
        let normalized = normalize_setting_kv("interfaceTransparency", &n.to_string())
            .map_err(|e| CommandError::new("invalid", e))?;
        pending.push(("interfaceTransparency".into(), normalized));
    }

    let mut db = state.db.lock();
    db.with_transaction(|conn| {
        for (key, value) in &pending {
            db::set_setting_on_conn(conn, key, value)?;
        }
        Ok(())
    })?;
    let map = db::get_settings(&db)?;
    Ok(settings_from_map(&map))
}

#[tauri::command]
pub fn list_added_settings(
    state: State<'_, AppState>,
    owner_tool_id: Option<String>,
) -> Result<Vec<AddedSettingRecord>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(db::list_added_settings(&db, owner_tool_id.as_deref())?)
}

#[tauri::command]
pub fn upsert_added_setting(
    state: State<'_, AppState>,
    input: UpsertAddedSettingInput,
) -> Result<AddedSettingRecord, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::upsert_added_setting(&mut db, &input)?)
}

#[tauri::command]
pub fn delete_added_setting(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::delete_added_setting(&mut db, &id)?)
}

/// Rejects any protected / `core.*` ids. Used before applying tool or resource changes.
#[tauri::command]
pub fn validate_change_targets(
    ids: Vec<String>,
) -> Result<ValidateChangeTargetsResult, CommandError> {
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

fn require_main_dock_window(window: &WebviewWindow) -> Result<(), CommandError> {
    if windows::caller_bound_tool_id(window).is_some() || window.label() != "main" {
        return Err(CommandError::new(
            "forbidden",
            "Only the main Coreside window can change the Dock icon.",
        ));
    }
    Ok(())
}

/// Single-authority Dock preference commit: validate → preflight → native plan → AppKit → persist (with rollback).
#[tauri::command]
pub fn commit_dock_icon_preference(
    app: AppHandle,
    state: State<'_, AppState>,
    window: WebviewWindow,
    preference: DockIconConfig,
) -> Result<DockIconCommitResult, CommandError> {
    state.require_profile()?;
    require_main_dock_window(&window)?;

    branding::commit_dock_preference(&app, &state, preference).map_err(|e| {
        if e.rollback_failed {
            CommandError::new(
                "dock_rollback_failed",
                "Couldn't save the Dock icon setting, and restoring the previous Dock icon also failed.",
            )
        } else {
            CommandError::new(e.code, e.message)
        }
    })
}

/// Query authoritative Dock icon status.
#[tauri::command]
pub fn get_dock_icon_status(
    app: AppHandle,
    state: State<'_, AppState>,
    window: WebviewWindow,
) -> Result<DockIconCommitResult, CommandError> {
    state.require_profile()?;
    require_main_dock_window(&window)?;

    branding::get_dock_icon_status(Some(&app), &state)
        .map_err(|e| CommandError::new("dock_status", e))
}

/// Reapply the persisted Dock preference at startup (main window only).
#[tauri::command]
pub fn apply_persisted_dock_icon(
    app: AppHandle,
    state: State<'_, AppState>,
    window: WebviewWindow,
) -> Result<DockIconCommitResult, CommandError> {
    state.require_profile()?;
    require_main_dock_window(&window)?;

    branding::reconcile_dock_on_startup(&app, &state.db)
        .map_err(|e| CommandError::new("dock_icon", format!("Couldn't update the Dock icon: {e}")))
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_wallpaper_pair_clear_legacy_and_schema() {
        let (wj, wall) = resolve_wallpaper_pair("").unwrap();
        assert_eq!(wj, "");
        assert!(wall.contains("\"kind\":\"none\""));

        let (wj, wall) = resolve_wallpaper_pair(r#"{"kind":"none"}"#).unwrap();
        assert_eq!(wj, "");
        assert!(wall.contains("\"kind\":\"none\""));

        let (wj, wall) = resolve_wallpaper_pair(r#"{"kind":"matrix"}"#).unwrap();
        assert_eq!(wj, "");
        assert!(wall.contains("\"kind\":\"matrix\""));

        let schema = r#"{"schemaVersion":"1","type":"canvas-preset","preset":"aurora"}"#;
        let (wj, wall) = resolve_wallpaper_pair(schema).unwrap();
        assert!(wj.contains("canvas-preset"));
        assert!(wall.contains("\"kind\":\"none\""));
    }

    #[test]
    fn wallpaper_pair_writes_atomically_in_transaction() {
        let dir = tempdir().unwrap();
        let mut db = db::Database::open_path(&dir.path().join("t.db")).unwrap();
        let (wallpaper_json, wallpaper) = resolve_wallpaper_pair(
            r#"{"schemaVersion":"1","type":"canvas-preset","preset":"matrix"}"#,
        )
        .unwrap();

        db.with_transaction(|conn| {
            db::set_setting_on_conn(conn, "wallpaperJson", &wallpaper_json)?;
            db::set_setting_on_conn(conn, "wallpaper", &wallpaper)?;
            Ok(())
        })
        .unwrap();

        let map = db::get_settings(&db).unwrap();
        assert!(map
            .get("wallpaperJson")
            .map(|s| s.contains("canvas-preset"))
            .unwrap_or(false));
        assert!(map
            .get("wallpaper")
            .map(|s| s.contains("\"kind\":\"none\""))
            .unwrap_or(false));
    }

    #[test]
    fn resolve_wallpaper_pair_rejects_invalid_schema() {
        let err =
            resolve_wallpaper_pair(r#"{"schemaVersion":"1","type":"not-a-real-wallpaper-type"}"#)
                .unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn resolve_wallpaper_pair_rejects_unparseable_and_unrecognized() {
        let err = resolve_wallpaper_pair("not-json").unwrap_err();
        assert!(err.contains("valid JSON"));

        let err = resolve_wallpaper_pair("[]").unwrap_err();
        assert!(err.contains("JSON object"));

        let err = resolve_wallpaper_pair(r#"{"foo":1}"#).unwrap_err();
        assert!(err.contains("schemaVersion") || err.contains("legacy kind"));
    }
}
