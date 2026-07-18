use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, Utc, Weekday};
use serde::{Deserialize, Serialize};

/// Floor used when advancing interval schedules (must stay ≥ validation::MIN_INTERVAL_MINUTES).
const MIN_SCHEDULE_INTERVAL_MINUTES: u32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Automation {
    pub id: String,
    pub workspace_id: String,
    pub owner_tool_id: Option<String>,
    pub name: String,
    pub enabled: bool,
    pub trigger: AutomationTrigger,
    pub action: AutomationAction,
    pub requires_ai: bool,
    pub provider_connection_id: Option<String>,
    pub missed_run_policy: MissedRunPolicy,
    pub next_run_at: Option<String>,
    pub last_run_at: Option<String>,
    pub last_status: Option<String>,
    pub consecutive_failures: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AutomationTrigger {
    #[serde(rename = "interval")]
    Interval {
        interval_minutes: u32,
        #[serde(default)]
        timezone: Option<String>,
    },
    #[serde(rename = "daily")]
    Daily {
        /// Local time `HH:MM`
        time: String,
        #[serde(default)]
        timezone: Option<String>,
    },
    #[serde(rename = "weekly")]
    Weekly {
        /// 0 = Monday … 6 = Sunday (ISO-ish consumer label)
        weekday: u8,
        time: String,
        #[serde(default)]
        timezone: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AutomationAction {
    #[serde(rename = "cycle_workspace_backgrounds")]
    CycleWorkspaceBackgrounds {
        workspace_id: String,
        preset_ids: Vec<String>,
    },
    #[serde(rename = "set_workspace_background")]
    SetWorkspaceBackground {
        workspace_id: String,
        preset_id: String,
    },
    #[serde(rename = "set_tool_state_value")]
    SetToolStateValue {
        tool_id: String,
        path: String,
        value: serde_json::Value,
    },
    #[serde(rename = "ai_prompt")]
    AiPrompt {
        prompt: String,
        conversation_id: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MissedRunPolicy {
    RunOnce,
    Skip,
}

impl Default for MissedRunPolicy {
    fn default() -> Self {
        Self::RunOnce
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRun {
    pub id: String,
    pub automation_id: String,
    pub scheduled_at: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub result_summary: Option<String>,
    pub error_category: Option<String>,
    pub created_at: String,
}

pub fn parse_hhmm(time: &str) -> Option<NaiveTime> {
    let parts: Vec<_> = time.trim().split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    NaiveTime::from_hms_opt(hour, minute, 0)
}

pub fn compute_next_after(
    trigger: &AutomationTrigger,
    from: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let local = from.with_timezone(&Local);
    match trigger {
        AutomationTrigger::Interval {
            interval_minutes, ..
        } => {
            // Defense in depth: never schedule faster than the non-AI floor,
            // even if a row was tampered below validation limits.
            let mins = (*interval_minutes).max(MIN_SCHEDULE_INTERVAL_MINUTES) as i64;
            Some(from + Duration::minutes(mins))
        }
        AutomationTrigger::Daily { time, .. } => {
            let t = parse_hhmm(time)?;
            let mut candidate = local
                .date_naive()
                .and_time(t)
                .and_local_timezone(Local)
                .single()?;
            if candidate <= local {
                candidate += Duration::days(1);
            }
            Some(candidate.with_timezone(&Utc))
        }
        AutomationTrigger::Weekly { weekday, time, .. } => {
            let t = parse_hhmm(time)?;
            let target = match *weekday {
                0 => Weekday::Mon,
                1 => Weekday::Tue,
                2 => Weekday::Wed,
                3 => Weekday::Thu,
                4 => Weekday::Fri,
                5 => Weekday::Sat,
                _ => Weekday::Sun,
            };
            let mut day = local.date_naive();
            for _ in 0..8 {
                if day.weekday() == target {
                    let candidate = day
                        .and_time(t)
                        .and_local_timezone(Local)
                        .single()?;
                    if candidate > local {
                        return Some(candidate.with_timezone(&Utc));
                    }
                }
                day += Duration::days(1);
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_next_is_forward() {
        let now = Utc::now();
        let next = compute_next_after(
            &AutomationTrigger::Interval {
                interval_minutes: 300,
                timezone: None,
            },
            now,
        )
        .unwrap();
        assert!(next > now);
    }
}
