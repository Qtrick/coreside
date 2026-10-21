//! Chat-proposed automation (user must confirm before schedule).

use serde::{Deserialize, Serialize};

use crate::automations::{
    validate_automation, AutomationAction, AutomationTrigger, MissedRunPolicy,
};

/// Untrusted model proposal. Never schedules until the user confirms via
/// `upsert_automation`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationChangePayload {
    pub name: String,
    pub trigger: AutomationTrigger,
    pub action: AutomationAction,
    #[serde(default)]
    pub requires_ai: Option<bool>,
    #[serde(default)]
    pub application_id: Option<String>,
    #[serde(default)]
    pub change_summary: Option<String>,
    #[serde(default)]
    pub missed_run_policy: Option<MissedRunPolicy>,
}

impl AutomationChangePayload {
    /// Structural + product gates for chat proposals.
    pub fn validate(&self) -> Result<(), String> {
        let requires_ai = self.requires_ai.unwrap_or(false);
        if matches!(self.action, AutomationAction::AiPrompt { .. }) || requires_ai {
            return Err(
                "chat automation proposals cannot use AI-backed actions yet; use a deterministic action"
                    .into(),
            );
        }
        validate_automation(&self.name, &self.trigger, &self.action, false)?;
        if let AutomationTrigger::Weekly { weekday, .. } = &self.trigger {
            if *weekday > 6 {
                return Err("weekday must be 0 (Monday) through 6 (Sunday)".into());
            }
        }
        if let Some(summary) = &self.change_summary {
            if summary.chars().count() > 280 {
                return Err("changeSummary is too long".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DEFAULT_WORKSPACE_ID;

    #[test]
    fn accepts_daily_wallpaper_rotation() {
        let payload = AutomationChangePayload {
            name: "Morning wallpaper".into(),
            trigger: AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: None,
            },
            action: AutomationAction::SetWorkspaceBackground {
                workspace_id: DEFAULT_WORKSPACE_ID.into(),
                preset_id: "preset-1".into(),
            },
            requires_ai: Some(false),
            application_id: None,
            change_summary: Some("Change wallpaper each morning".into()),
            missed_run_policy: None,
        };
        assert!(payload.validate().is_ok());
    }

    #[test]
    fn rejects_ai_prompt_from_chat() {
        let payload = AutomationChangePayload {
            name: "Morning review".into(),
            trigger: AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Summarize my tasks".into(),
                conversation_id: None,
            },
            requires_ai: Some(true),
            application_id: None,
            change_summary: None,
            missed_run_policy: None,
        };
        let err = payload.validate().expect_err("ai prompt blocked");
        assert!(err.contains("AI-backed"));
    }

    #[test]
    fn rejects_weekday_out_of_range() {
        let payload = AutomationChangePayload {
            name: "Weekly wallpaper".into(),
            trigger: AutomationTrigger::Weekly {
                weekday: 9,
                time: "08:00".into(),
                timezone: None,
            },
            action: AutomationAction::SetWorkspaceBackground {
                workspace_id: DEFAULT_WORKSPACE_ID.into(),
                preset_id: "preset-1".into(),
            },
            requires_ai: Some(false),
            application_id: None,
            change_summary: None,
            missed_run_policy: None,
        };
        let err = payload.validate().expect_err("weekday blocked");
        assert!(err.contains("weekday"));
    }
}
