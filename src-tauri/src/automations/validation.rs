use super::models::{AutomationAction, AutomationTrigger};

pub const MIN_INTERVAL_MINUTES: u32 = 5;
pub const MIN_AI_INTERVAL_MINUTES: u32 = 30;
pub const MAX_CONSECUTIVE_FAILURES: i64 = 5;

pub fn validate_automation(
    name: &str,
    trigger: &AutomationTrigger,
    action: &AutomationAction,
    requires_ai: bool,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Automation name is required".into());
    }
    match trigger {
        AutomationTrigger::Interval {
            interval_minutes, ..
        } => {
            let min = if requires_ai {
                MIN_AI_INTERVAL_MINUTES
            } else {
                MIN_INTERVAL_MINUTES
            };
            if *interval_minutes < min {
                return Err(format!(
                    "Interval must be at least {min} minutes for this automation type"
                ));
            }
        }
        AutomationTrigger::Daily { time, .. } | AutomationTrigger::Weekly { time, .. } => {
            if super::models::parse_hhmm(time).is_none() {
                return Err("Time must be HH:MM".into());
            }
        }
    }

    match action {
        AutomationAction::CycleWorkspaceBackgrounds { preset_ids, .. } => {
            if preset_ids.len() < 2 {
                return Err("Background rotation needs at least two presets".into());
            }
        }
        AutomationAction::AiPrompt { prompt, .. } => {
            if !requires_ai {
                return Err("AI prompt actions must set requiresAi".into());
            }
            if prompt.trim().is_empty() {
                return Err("AI prompt cannot be empty".into());
            }
        }
        AutomationAction::SetToolStateValue { path, .. } => {
            if path.trim().is_empty() || path.contains("..") {
                return Err("Invalid tool state path".into());
            }
            if path.starts_with("core.") {
                return Err("Protected resources cannot be automation targets".into());
            }
        }
        AutomationAction::SetWorkspaceBackground { .. } => {}
    }

    if matches!(action, AutomationAction::AiPrompt { .. }) && !requires_ai {
        return Err("AI actions require requiresAi confirmation".into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automations::models::AutomationAction;

    #[test]
    fn rejects_short_interval() {
        let err = validate_automation(
            "test",
            &AutomationTrigger::Interval {
                interval_minutes: 2,
                timezone: None,
            },
            &AutomationAction::SetWorkspaceBackground {
                workspace_id: "ws".into(),
                preset_id: "p1".into(),
            },
            false,
        );
        assert!(err.is_err());
    }
}
