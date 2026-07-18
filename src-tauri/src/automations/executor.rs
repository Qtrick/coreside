use super::models::{Automation, AutomationAction};
use crate::db::{self, Database, DbResult};
use crate::security::is_protected;

pub fn execute_automation(db: &mut Database, automation: &Automation) -> DbResult<String> {
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
        AutomationAction::SetToolStateValue {
            tool_id,
            path,
            value,
        } => {
            if is_protected(tool_id) || is_protected(path) {
                return Err(db::DbError::Invalid(
                    "Protected resources cannot be modified by automations".into(),
                ));
            }
            let mut state = db::get_tool_state(db, tool_id)?
                .unwrap_or_else(|| serde_json::json!({}));
            if let Some(obj) = state.as_object_mut() {
                obj.insert(path.clone(), value.clone());
            } else {
                state = serde_json::json!({ path: value });
            }
            db::save_tool_state(db, tool_id, &state)?;
            Ok(format!("Updated {path} on tool {tool_id}"))
        }
        AutomationAction::AiPrompt { .. } => Err(db::DbError::Invalid(
            "AI-backed automations require an active provider and explicit confirmation; deferred execution is not available without a provider".into(),
        )),
    }
}
