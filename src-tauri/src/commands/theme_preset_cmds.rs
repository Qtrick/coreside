//! Theme preset CRUD + atomic appearance apply (main window only).

use serde::Deserialize;
use tauri::{AppHandle, Emitter, State, WebviewWindow};

use super::{settings_cmds, CommandError};
use crate::credentials::{
    contract_default_plan_presentation, fetch_server_plan_presentation, HostedPlanPresentation,
};
use crate::db;
use crate::state::AppState;
use crate::theme_presets::{
    self, apply_appearance_snapshot, max_presets_for_plan, AppearanceSnapshot,
    ThemePresetCapabilities, ThemePresetRecord, FREE_THEME_PRESET_LIMIT,
};
use crate::windows;

pub const THEME_PRESETS_CHANGED_EVENT: &str = "theme-presets-changed";
pub const APPEARANCE_CHANGED_EVENT: &str = "appearance-changed";

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

async fn resolve_plan() -> HostedPlanPresentation {
    fetch_server_plan_presentation()
        .await
        .unwrap_or_else(|_| contract_default_plan_presentation())
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
    pub snapshot: AppearanceSnapshot,
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
    apply_appearance_snapshot(&mut db, &input.snapshot)?;
    let map = db::get_settings(&db)?;
    let settings = settings_cmds::settings_from_map(&map);
    drop(db);
    emit_appearance_changed(&app);
    Ok(settings)
}
