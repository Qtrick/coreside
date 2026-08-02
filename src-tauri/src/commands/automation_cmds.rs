//! Automation IPC commands.

use chrono::Utc;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use super::CommandError;
use crate::automations::{
    compute_next_run, validate_automation, Automation, AutomationAction, AutomationTrigger,
    MissedRunPolicy, SchedulerHandle,
};
use crate::db::{self, DEFAULT_WORKSPACE_ID};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAutomationInput {
    pub id: Option<String>,
    pub name: String,
    pub enabled: Option<bool>,
    pub trigger: AutomationTrigger,
    pub action: AutomationAction,
    pub requires_ai: Option<bool>,
    pub missed_run_policy: Option<MissedRunPolicy>,
    pub owner_tool_id: Option<String>,
    pub workspace_id: Option<String>,
    /// Bind this automation to a generated application so its privileged work
    /// runs through the registered action gateway.
    pub application_id: Option<String>,
}

#[tauri::command]
pub fn list_automations(state: State<'_, AppState>) -> Result<Vec<Automation>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(db::list_automations(&db)?)
}

#[tauri::command]
pub fn upsert_automation(
    state: State<'_, AppState>,
    input: UpsertAutomationInput,
) -> Result<Automation, CommandError> {
    state.require_profile()?;
    let requires_ai = input.requires_ai.unwrap_or(false);
    validate_automation(&input.name, &input.trigger, &input.action, requires_ai)
        .map_err(|e| CommandError::new("invalid", e))?;

    let id = input
        .id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("auto-{}", Uuid::new_v4()));
    let now = Utc::now();
    let next = compute_next_run(&input.trigger, now).map(|d| d.to_rfc3339());
    let existing = {
        let db = state.db.lock();
        db::get_automation(&db, &id).ok()
    };
    let row = Automation {
        id: id.clone(),
        workspace_id: input
            .workspace_id
            .unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string()),
        owner_tool_id: input.owner_tool_id,
        application_id: input
            .application_id
            .filter(|s| !s.trim().is_empty())
            .or_else(|| existing.as_ref().and_then(|e| e.application_id.clone())),
        name: input.name.trim().to_string(),
        enabled: input.enabled.unwrap_or(true),
        trigger: input.trigger,
        action: input.action,
        requires_ai,
        provider_connection_id: None,
        missed_run_policy: input.missed_run_policy.unwrap_or_default(),
        next_run_at: next.or_else(|| existing.as_ref().and_then(|e| e.next_run_at.clone())),
        last_run_at: existing.as_ref().and_then(|e| e.last_run_at.clone()),
        last_status: existing.as_ref().and_then(|e| e.last_status.clone()),
        consecutive_failures: existing
            .as_ref()
            .map(|e| e.consecutive_failures)
            .unwrap_or(0),
        waiting_approval: existing.as_ref().is_some_and(|e| e.waiting_approval),
        permission_ready: existing
            .as_ref()
            .map(|e| e.permission_ready)
            .unwrap_or(true),
        created_at: existing
            .as_ref()
            .map(|e| e.created_at.clone())
            .unwrap_or_else(|| now.to_rfc3339()),
        updated_at: now.to_rfc3339(),
    };
    let mut db = state.db.lock();
    Ok(db::upsert_automation(&mut db, &row)?)
}

#[tauri::command]
pub fn set_automation_enabled(
    state: State<'_, AppState>,
    automation_id: String,
    enabled: bool,
) -> Result<Automation, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    let mut row = db::get_automation(&db, automation_id.trim())?;
    row.enabled = enabled;
    row.updated_at = Utc::now().to_rfc3339();
    if enabled && row.next_run_at.is_none() {
        row.next_run_at = compute_next_run(&row.trigger, Utc::now()).map(|d| d.to_rfc3339());
    }
    Ok(db::upsert_automation(&mut db, &row)?)
}

#[tauri::command]
pub fn delete_automation(
    state: State<'_, AppState>,
    automation_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(db::delete_automation(&mut db, automation_id.trim())?)
}

#[tauri::command]
pub fn list_automation_runs(
    state: State<'_, AppState>,
    automation_id: String,
) -> Result<Vec<crate::automations::AutomationRun>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(db::list_automation_runs(&db, automation_id.trim(), 50)?)
}

#[tauri::command]
pub fn run_automation_now(
    state: State<'_, AppState>,
    scheduler: State<'_, std::sync::Arc<SchedulerHandle>>,
    automation_id: String,
) -> Result<String, CommandError> {
    state.require_profile()?;
    crate::automations::run_now(&state, automation_id.trim(), &scheduler)
        .map_err(|e| CommandError::new("automation", e))
}

#[tauri::command]
pub fn list_workspace_backgrounds(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
) -> Result<Vec<db::WorkspaceBackground>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let ws = workspace_id.unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string());
    Ok(db::list_workspace_backgrounds(&db, &ws)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertBackgroundInput {
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    pub definition: serde_json::Value,
    pub workspace_id: Option<String>,
    pub sort_order: Option<i64>,
}

#[tauri::command]
pub fn upsert_workspace_background(
    state: State<'_, AppState>,
    input: UpsertBackgroundInput,
) -> Result<db::WorkspaceBackground, CommandError> {
    state.require_profile()?;
    let kind = input.kind.trim().to_lowercase();
    if !matches!(
        kind.as_str(),
        "solid" | "linear" | "radial" | "image" | "pattern"
    ) {
        return Err(CommandError::new(
            "invalid",
            "Background kind must be solid, linear, radial, image, or pattern",
        ));
    }
    let now = Utc::now().to_rfc3339();
    let id = input
        .id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("bg-{}", Uuid::new_v4()));
    let row = db::WorkspaceBackground {
        id,
        workspace_id: input
            .workspace_id
            .unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string()),
        name: input.name.trim().to_string(),
        kind,
        definition_json: input.definition.to_string(),
        sort_order: input.sort_order.unwrap_or(0),
        created_at: now.clone(),
        updated_at: now,
    };
    let mut db = state.db.lock();
    Ok(db::upsert_workspace_background(&mut db, &row)?)
}
