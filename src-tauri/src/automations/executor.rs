use serde_json::json;

use super::models::{Automation, AutomationAction};
use crate::application_kernel::registered_actions::{
    execute_registered_action_trusted, ActionOutcome,
};
use crate::db::{self, Database, DbResult};
use crate::security::is_protected;

pub fn execute_automation(db: &mut Database, automation: &Automation) -> DbResult<String> {
    // Tool-state mutations always go through the registered-action gateway
    // (OCC, approvals, project scope). Never write tool_state directly.
    if let AutomationAction::SetToolStateValue { tool_id, .. } = &automation.action {
        let application_id = automation
            .application_id
            .clone()
            .unwrap_or_else(|| tool_id.clone());
        return execute_application_bound(db, automation, application_id);
    }
    if let Some(application_id) = automation.application_id.clone() {
        return execute_application_bound(db, automation, application_id);
    }
    match &automation.action {
        AutomationAction::CycleWorkspaceBackgrounds {
            workspace_id,
            preset_ids,
        } => {
            if preset_ids.is_empty() {
                return Err(db::DbError::Invalid("No background presets".into()));
            }
            let current = db::get_setting(db, "activeWorkspaceBackgroundId").ok().flatten();
            let idx = current
                .as_deref()
                .and_then(|id| preset_ids.iter().position(|p| p == id))
                .map(|i| (i + 1) % preset_ids.len())
                .unwrap_or(0);
            let next = &preset_ids[idx];
            let preset = db::get_workspace_background(db, next)?;
            if preset.workspace_id != *workspace_id {
                return Err(db::DbError::Invalid(
                    "Background preset workspace mismatch".into(),
                ));
            }
            db::set_setting(db, "activeWorkspaceBackgroundId", next)?;
            db::set_setting(db, "workspaceBackgroundDefinition", &preset.definition_json)?;
            Ok(format!("Switched workspace background to {}", preset.name))
        }
        AutomationAction::SetWorkspaceBackground {
            workspace_id,
            preset_id,
        } => {
            let preset = db::get_workspace_background(db, preset_id)?;
            if preset.workspace_id != *workspace_id {
                return Err(db::DbError::Invalid(
                    "Background preset workspace mismatch".into(),
                ));
            }
            db::set_setting(db, "activeWorkspaceBackgroundId", preset_id)?;
            db::set_setting(db, "workspaceBackgroundDefinition", &preset.definition_json)?;
            Ok(format!("Set workspace background to {}", preset.name))
        }
        AutomationAction::SetToolStateValue { .. } => unreachable!(
            "SetToolStateValue is routed through the registered-action gateway above"
        ),
        AutomationAction::AiPrompt { .. } => Err(db::DbError::Invalid(
            "AI-backed automations require an active provider and explicit confirmation; deferred execution is not available without a provider".into(),
        )),
    }
}

/// Application-bound automations never touch application state directly. They
/// go through the registered action gateway with `presence: away`, which means
/// they only proceed on remembered, application-bound authority and otherwise
/// park for the user.
fn execute_application_bound(
    db: &mut Database,
    automation: &Automation,
    application_id: String,
) -> DbResult<String> {
    let (action_name, input) = match &automation.action {
        AutomationAction::SetToolStateValue {
            tool_id,
            path,
            value,
        } => {
            if tool_id != &application_id {
                return Err(db::DbError::Invalid(
                    "automation targets a different application".into(),
                ));
            }
            if is_protected(tool_id) || is_protected(path) {
                return Err(db::DbError::Invalid(
                    "Protected resources cannot be modified by automations".into(),
                ));
            }
            ("tool_state.set", json!({ "key": path, "value": value }))
        }
        _ => {
            return Err(db::DbError::Invalid(
                "this automation action is not available for application-bound automations".into(),
            ))
        }
    };

    let run_id = format!("automation-run-{}", uuid::Uuid::new_v4());
    let outcome = execute_registered_action_trusted(
        db,
        Some(application_id),
        &automation.id,
        &run_id,
        action_name,
        &input,
    );

    match outcome {
        ActionOutcome::Ok { .. } => {
            db::set_automation_runtime_flags(db, &automation.id, false, true)?;
            Ok(format!("Ran {action_name} while you were away"))
        }
        // Parking is the designed outcome for an away change without remembered
        // authority, so it is not counted as a failure.
        ActionOutcome::PendingApproval { .. } => {
            db::set_automation_runtime_flags(db, &automation.id, true, true)?;
            Ok("Waiting for your approval before making this change".into())
        }
        ActionOutcome::Blocked { reason, .. } => {
            db::set_automation_runtime_flags(db, &automation.id, false, false)?;
            Err(db::DbError::Invalid(reason))
        }
        ActionOutcome::Error { message, .. } => {
            db::set_automation_runtime_flags(db, &automation.id, false, true)?;
            Err(db::DbError::Invalid(message))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automations::models::{AutomationAction, AutomationTrigger, MissedRunPolicy};
    use tempfile::tempdir;

    fn sample_automation(action: AutomationAction) -> Automation {
        let now = chrono::Utc::now().to_rfc3339();
        Automation {
            id: "auto-1".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "test".into(),
            enabled: true,
            trigger: AutomationTrigger::Interval {
                interval_minutes: 60,
                timezone: None,
            },
            action,
            requires_ai: false,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(now.clone()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    #[test]
    fn set_tool_state_without_application_id_uses_gateway_not_direct_write() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("auto.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let automation = sample_automation(AutomationAction::SetToolStateValue {
            tool_id: "tool-orphan".into(),
            path: "count".into(),
            value: json!(1),
        });
        db::upsert_automation(&mut db, &automation).unwrap();
        // Gateway should refuse (no manifest / authority) — never silently write state.
        let before = db::get_tool_state(&db, "tool-orphan").unwrap();
        assert!(before.is_none());
        let result = execute_automation(&mut db, &automation);
        assert!(result.is_err() || result.as_ref().is_ok_and(|s| s.contains("Waiting")));
        let after = db::get_tool_state(&db, "tool-orphan").unwrap();
        assert!(
            after.is_none(),
            "legacy SetToolStateValue must not write tool_state directly"
        );
    }
}
