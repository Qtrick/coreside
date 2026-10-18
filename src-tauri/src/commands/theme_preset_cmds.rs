//! Theme preset CRUD + atomic appearance apply (main window only).

use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, State, WebviewWindow};

use super::{settings_cmds, CommandError};
use crate::ai::SettingsChangePayload;
use crate::credentials::{
    contract_default_plan_presentation, fetch_server_plan_presentation, HostedPlanPresentation,
};
use crate::db;
use crate::state::AppState;
use crate::theme_presets::{
    self, apply_appearance_changes, apply_appearance_snapshot, max_presets_for_plan,
    AppearanceSnapshot, ThemePresetCapabilities, ThemePresetRecord, FREE_THEME_PRESET_LIMIT,
};
use crate::windows;

pub const THEME_PRESETS_CHANGED_EVENT: &str = "theme-presets-changed";
pub const APPEARANCE_CHANGED_EVENT: &str = "appearance-changed";
const PLAN_CACHE_TTL: Duration = Duration::from_secs(30);
struct CachedPlan {
    fetched_at: Instant,
    user_id: String,
    plan: HostedPlanPresentation,
}
static PLAN_CACHE: Lazy<tokio::sync::Mutex<Option<CachedPlan>>> =
    Lazy::new(|| tokio::sync::Mutex::new(None));

fn require_main_window(window: &WebviewWindow) -> Result<(), CommandError> {
    if windows::caller_bound_tool_id(window).is_some() || window.label() != "main" {
        return Err(CommandError::new(
            "forbidden",
            "Theme presets can only be managed from the main Coreside window.",
        ));
    }
    Ok(())
}

fn emit_presets_changed(app: &AppHandle) {
    let _ = app.emit(THEME_PRESETS_CHANGED_EVENT, ());
}

fn emit_appearance_changed(app: &AppHandle) {
    let _ = app.emit(APPEARANCE_CHANGED_EVENT, ());
}

/// Resolve plan for quota. Free is always safe offline.
///
/// Cold fetches are bounded (2s) so Customization / Save never wait on the
/// full hosted entitlement timeout (~15s). Cached server plans stay authoritative
/// within PLAN_CACHE_TTL; failed/timeout lookups fall back to contract Free.
async fn resolve_plan() -> HostedPlanPresentation {
    let Some(session) = crate::credentials::load_valid_session(None) else {
        return contract_default_plan_presentation();
    };
    {
        let cache = PLAN_CACHE.lock().await;
        if let Some(cached) = cache.as_ref() {
            if cached.user_id == session.user_id && cached.fetched_at.elapsed() < PLAN_CACHE_TTL {
                return cached.plan.clone();
            }
        }
    }
    const ENTITLEMENT_LOOKUP_BUDGET: Duration = Duration::from_secs(2);
    let plan =
        match tokio::time::timeout(ENTITLEMENT_LOOKUP_BUDGET, fetch_server_plan_presentation())
            .await
        {
            Ok(Ok(plan)) => plan,
            Ok(Err(_)) | Err(_) => contract_default_plan_presentation(),
        };
    // The keyring session can change while the network request is in flight.
    // Never cache or apply an entitlement fetched under a different user.
    let current_session = crate::credentials::load_valid_session(None);
    if current_session.as_ref().map(|current| current.user_id.as_str())
        != Some(session.user_id.as_str())
    {
        return contract_default_plan_presentation();
    }
    let mut cache = PLAN_CACHE.lock().await;
    *cache = Some(CachedPlan {
        fetched_at: Instant::now(),
        user_id: session.user_id,
        plan: plan.clone(),
    });
    plan
}

fn capabilities_for(used: u32, plan: &HostedPlanPresentation) -> ThemePresetCapabilities {
    let limit = max_presets_for_plan(&plan.plan_id, &plan.source);
    ThemePresetCapabilities {
        used,
        limit,
        can_create: used < limit,
        plan_id: plan.plan_id.clone(),
        plan_source: plan.source.clone(),
        free_limit: FREE_THEME_PRESET_LIMIT,
    }
}

fn current_committed_snapshot(db: &db::Database) -> Result<AppearanceSnapshot, CommandError> {
    let map = db::get_settings(db)?;
    AppearanceSnapshot::from_settings_map(&map).map_err(|e| CommandError::new("invalid", e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveThemePresetInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameThemePresetInput {
    pub id: String,
    pub name: String,
    pub expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateThemePresetInput {
    pub id: String,
    pub expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteThemePresetInput {
    pub id: String,
    pub expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyThemePresetInput {
    pub id: String,
    pub expected_revision: i64,
    /// Fingerprint of the committed appearance when the Apply confirmation opened.
    /// Prevents a stale dialog from overwriting a newer appearance change.
    pub expected_appearance_fingerprint: String,
}

#[tauri::command]
pub fn list_theme_presets(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<ThemePresetRecord>, CommandError> {
    require_main_window(&window)?;
    state.require_profile()?;
    let db = state.db.lock();
    Ok(theme_presets::list_theme_presets(&db)?)
}

#[tauri::command]
pub async fn get_theme_preset_capabilities(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<ThemePresetCapabilities, CommandError> {
    require_main_window(&window)?;
    state.require_profile()?;
    let used = {
        let db = state.db.lock();
        theme_presets::count_theme_presets(&db)?
    };
    let plan = resolve_plan().await;
    Ok(capabilities_for(used, &plan))
}

#[tauri::command]
pub async fn save_current_theme_preset(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    input: SaveThemePresetInput,
) -> Result<ThemePresetRecord, CommandError> {
    require_main_window(&window)?;
    state.require_profile()?;
    let plan = resolve_plan().await;
    let limit = max_presets_for_plan(&plan.plan_id, &plan.source);
    let mut db = state.db.lock();
    let used = theme_presets::count_theme_presets(&db)?;
    if used >= limit {
        return Err(CommandError::new(
            "invalid",
            format!("Theme preset limit reached ({limit}). Delete or update an existing theme."),
        ));
    }
    let snapshot = current_committed_snapshot(&db)?;
    let record = theme_presets::insert_theme_preset(
        &mut db,
        &input.name,
        &input.description,
        &snapshot,
        limit,
    )?;
    drop(db);
    emit_presets_changed(&app);
    Ok(record)
}

#[tauri::command]
pub fn rename_theme_preset(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    input: RenameThemePresetInput,
) -> Result<ThemePresetRecord, CommandError> {
    require_main_window(&window)?;
    state.require_profile()?;
    let mut db = state.db.lock();
    let record = theme_presets::rename_theme_preset(
        &mut db,
        &input.id,
        &input.name,
        input.expected_revision,
    )?;
    drop(db);
    emit_presets_changed(&app);
    Ok(record)
}

#[tauri::command]
pub fn update_theme_preset_from_current(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    input: UpdateThemePresetInput,
) -> Result<ThemePresetRecord, CommandError> {
    require_main_window(&window)?;
    state.require_profile()?;
    let mut db = state.db.lock();
    let snapshot = current_committed_snapshot(&db)?;
    let record = theme_presets::update_theme_preset_snapshot(
        &mut db,
        &input.id,
        &snapshot,
        input.expected_revision,
    )?;
    drop(db);
    emit_presets_changed(&app);
    Ok(record)
}

#[tauri::command]
pub fn delete_theme_preset(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    input: DeleteThemePresetInput,
) -> Result<(), CommandError> {
    require_main_window(&window)?;
    state.require_profile()?;
    let mut db = state.db.lock();
    theme_presets::delete_theme_preset(&mut db, &input.id, input.expected_revision)?;
    drop(db);
    emit_presets_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn get_appearance_fingerprint(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    require_main_window(&window)?;
    state.require_profile()?;
    let db = state.db.lock();
    AppearanceSnapshot::committed_fingerprint_from_db(&db)
        .map_err(|e| CommandError::new("invalid", e))
}

#[tauri::command]
pub fn apply_theme_preset(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    input: ApplyThemePresetInput,
) -> Result<settings_cmds::AppSettings, CommandError> {
    require_main_window(&window)?;
    state.require_profile()?;
    let mut db = state.db.lock();
    let preset = theme_presets::get_theme_preset(&db, &input.id)?;
    if preset.revision != input.expected_revision {
        return Err(CommandError::new(
            "conflict",
            "Theme preset was updated elsewhere. Refresh and try again.",
        ));
    }
    if input.expected_appearance_fingerprint.trim().is_empty() {
        return Err(CommandError::new(
            "invalid",
            "Apply confirmation is missing an appearance fingerprint. Close the dialog and try again.",
        ));
    }
    let current_fp = AppearanceSnapshot::committed_fingerprint_from_db(&db)
        .map_err(|e| CommandError::new("invalid", e))?;
    if current_fp != input.expected_appearance_fingerprint {
        return Err(CommandError::new(
            "conflict",
            "Your appearance changed since this confirmation opened. Review and try Apply again.",
        ));
    }
    apply_appearance_snapshot(&mut db, &preset.snapshot)?;
    let map = db::get_settings(&db)?;
    let settings = settings_cmds::settings_from_map(&map);
    drop(db);
    emit_appearance_changed(&app);
    Ok(settings)
}

/// Atomic appearance bundle for user-approved settings proposals (not model-direct).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyAppearanceBundleInput {
    pub changes: SettingsChangePayload,
}

#[tauri::command]
pub fn apply_appearance_bundle(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    input: ApplyAppearanceBundleInput,
) -> Result<settings_cmds::AppSettings, CommandError> {
    require_main_window(&window)?;
    state.require_profile()?;
    let mut db = state.db.lock();
    apply_appearance_changes(&mut db, &input.changes)?;
    let map = db::get_settings(&db)?;
    let settings = settings_cmds::settings_from_map(&map);
    drop(db);
    emit_appearance_changed(&app);
    Ok(settings)
}
