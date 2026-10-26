use super::models::{parse_trigger_timezone, AutomationAction, AutomationTrigger};

pub const MIN_INTERVAL_MINUTES: u32 = 5;
pub const MIN_AI_INTERVAL_MINUTES: u32 = 30;
pub const MAX_CONSECUTIVE_FAILURES: i64 = 5;
/// Keep in sync with ai_exec::MAX_PROMPT_CHARS and frontend parseAction.
pub const MAX_AI_PROMPT_CHARS: usize = 4_000;

fn trigger_timezone(trigger: &AutomationTrigger) -> Option<&str> {
    match trigger {
        AutomationTrigger::Interval { timezone, .. }
        | AutomationTrigger::Daily { timezone, .. }
        | AutomationTrigger::Weekly { timezone, .. }
        | AutomationTrigger::Weekdays { timezone, .. } => timezone.as_deref(),
    }
}

pub fn validate_automation(
    name: &str,
    trigger: &AutomationTrigger,
    action: &AutomationAction,
    requires_ai: bool,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Automation name is required".into());
    }
    parse_trigger_timezone(trigger_timezone(trigger))?;
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
        AutomationTrigger::Daily { time, .. } => {
            if super::models::parse_hhmm(time).is_none() {
                return Err("Time must be HH:MM".into());
            }
        }
        AutomationTrigger::Weekly { weekday, time, .. } => {
            if *weekday > 6 {
                return Err("weekday must be 0 (Monday) through 6 (Sunday)".into());
            }
            if super::models::parse_hhmm(time).is_none() {
                return Err("Time must be HH:MM".into());
            }
        }
        AutomationTrigger::Weekdays { weekdays, time, .. } => {
            if weekdays.is_empty() {
                return Err("Weekdays selection cannot be empty".into());
            }
            let mut seen = std::collections::HashSet::new();
            for &w in weekdays {
                if w > 6 {
                    return Err("weekday must be 0 (Monday) through 6 (Sunday)".into());
                }
                if !seen.insert(w) {
                    return Err(format!("Duplicate weekday {w} in schedule"));
                }
            }
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
            if prompt.chars().count() > MAX_AI_PROMPT_CHARS {
                return Err(format!(
                    "AI prompt exceeds the allowed length ({MAX_AI_PROMPT_CHARS} characters)"
                ));
            }
        }
        AutomationAction::SetToolStateValue { tool_id, path, .. } => {
            if tool_id.trim().is_empty() {
                return Err("Tool state automations require a tool id".into());
            }
            if crate::security::is_protected(tool_id) {
                return Err("Protected resources cannot be automation targets".into());
            }
            if path.trim().is_empty() || path.contains("..") {
                return Err("Invalid tool state path".into());
            }
            if path.starts_with("core.") || crate::security::is_protected(path) {
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

    #[test]
    fn rejects_oversized_ai_prompt() {
        let prompt = "x".repeat(MAX_AI_PROMPT_CHARS + 1);
        let err = validate_automation(
            "review",
            &AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: None,
            },
            &AutomationAction::AiPrompt {
                prompt,
                conversation_id: None,
            },
            true,
        )
        .unwrap_err();
        assert!(err.contains("length"));
    }
}
