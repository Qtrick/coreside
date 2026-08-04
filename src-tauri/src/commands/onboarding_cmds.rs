//! Onboarding / tutorial IPC commands.

use tauri::{State, WebviewWindow};

use super::CommandError;
use crate::db::{
    self, OnboardingState, TutorialProgress, TutorialSampleSeed, UpsertTutorialProgressInput,
};
use crate::state::AppState;
use crate::windows;

#[tauri::command]
pub fn get_onboarding_state(
    state: State<'_, AppState>,
    window: WebviewWindow,
) -> Result<OnboardingState, CommandError> {
    state.require_profile()?;
    let is_secondary = windows::caller_bound_tool_id(&window).is_some();
    let db = state.db.lock();
    Ok(db::get_onboarding_state(&db, is_secondary)?)
}

#[tauri::command]
pub fn upsert_tutorial_progress(
    state: State<'_, AppState>,
    tutorial_id: String,
    tutorial_version: i64,
    status: String,
    current_step_id: Option<String>,
    completed_step_ids: Option<Vec<String>>,
) -> Result<TutorialProgress, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::upsert_tutorial_progress(
        &mut db,
        UpsertTutorialProgressInput {
            tutorial_id,
            tutorial_version,
            status,
            current_step_id,
            completed_step_ids,
        },
    )?)
}

#[tauri::command]
pub fn reset_tutorial_progress(
    state: State<'_, AppState>,
    tutorial_id: Option<String>,
) -> Result<u64, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::reset_tutorial_progress(
        &mut db,
        tutorial_id.as_deref(),
    )?)
}

#[tauri::command]
pub fn seed_tutorial_sample(
    state: State<'_, AppState>,
) -> Result<TutorialSampleSeed, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::seed_tutorial_sample(&mut db)?)
}

#[tauri::command]
pub fn cleanup_tutorial_sample(state: State<'_, AppState>) -> Result<u64, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::cleanup_tutorial_sample(&mut db)?)
}
