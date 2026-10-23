//! Chat-proposed automation (user must confirm before schedule).

use serde::{Deserialize, Serialize};

use crate::automations::{
    validate_automation, AutomationAction, AutomationTrigger, MissedRunPolicy,
};

/// Untrusted model proposal. Never schedules until the user confirms via
/// `confirm_automation_change` / `upsert_automation`.
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
        let requires_ai = self
            .requires_ai
            .unwrap_or_else(|| matches!(self.action, AutomationAction::AiPrompt { .. }));
        if matches!(self.action, AutomationAction::AiPrompt { .. }) && !requires_ai {
            return Err("AI prompt actions must set requiresAi".into());
        }
        // Chat may propose AI read/summarize prompts; executor enforces budgets,
        // credentials, recovery, and no recursive scheduling.
        validate_automation(&self.name, &self.trigger, &self.action, requires_ai)?;
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
        if let Some(app_id) = &self.application_id {
            if app_id.trim().is_empty() {
                return Err("applicationId cannot be empty".into());
            }
        }
        Ok(())
    }

    pub fn effective_requires_ai(&self) -> bool {
        self.requires_ai
            .unwrap_or_else(|| matches!(self.action, AutomationAction::AiPrompt { .. }))
    }

    /// Schedule-relevant equality for confirm binding (ignores changeSummary wording).
    /// Prevents bait-and-switch if message metadata is rewritten after the user reviewed it.
    pub fn same_confirm_intent(&self, other: &Self) -> bool {
        let norm_app = |id: &Option<String>| {
            id.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        self.name.trim() == other.name.trim()
            && self.trigger == other.trigger
            && self.action == other.action
            && self.effective_requires_ai() == other.effective_requires_ai()
            && norm_app(&self.application_id) == norm_app(&other.application_id)
            && self.missed_run_policy == other.missed_run_policy
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
    fn accepts_ai_prompt_from_chat_when_requires_ai() {
        let payload = AutomationChangePayload {
            name: "Morning review".into(),
            trigger: AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: Some("America/New_York".into()),
            },
            action: AutomationAction::AiPrompt {
                prompt: "Summarize my unfinished study tasks".into(),
                conversation_id: None,
            },
            requires_ai: Some(true),
            application_id: Some("app-study".into()),
            change_summary: Some("Weekday study review".into()),
            missed_run_policy: None,
        };
        assert!(payload.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_timezone() {
        let payload = AutomationChangePayload {
            name: "Morning review".into(),
            trigger: AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: Some("Not/Real".into()),
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
        let err = payload.validate().expect_err("bad tz blocked");
        assert!(err.contains("timezone") || err.contains("Timezone"));
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

    #[test]
    fn same_confirm_intent_ignores_summary_and_requires_ai_default() {
        let a = AutomationChangePayload {
            name: " Review ".into(),
            trigger: AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Summarize".into(),
                conversation_id: None,
            },
            requires_ai: None,
            application_id: Some(" app-1 ".into()),
            change_summary: Some("wording A".into()),
            missed_run_policy: None,
        };
        let b = AutomationChangePayload {
            name: "Review".into(),
            trigger: a.trigger.clone(),
            action: a.action.clone(),
            requires_ai: Some(true),
            application_id: Some("app-1".into()),
            change_summary: Some("wording B".into()),
            missed_run_policy: None,
        };
        assert!(a.same_confirm_intent(&b));
    }
}
