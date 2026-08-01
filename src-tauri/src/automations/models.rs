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
    /// Set when the automation acts on a generated application. Application-bound
    /// automations run their privileged work through the registered action
    /// gateway with `presence: away`.
    #[serde(default)]
    pub application_id: Option<String>,
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
    /// True when the last away run parked on a pending approval.
    #[serde(default)]
    pub waiting_approval: bool,
    /// False when the last away run was refused for missing authority.
    #[serde(default = "default_true")]
    pub permission_ready: bool,
    pub created_at: String,
    pub updated_at: String,
}

fn default_true() -> bool {
    true
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
#[derive(Default)]
pub enum MissedRunPolicy {
    #[default]
    RunOnce,
    Skip,
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

/// Resolve a local wall-clock date+time across DST boundaries.
///
/// `and_local_timezone(..).single()` returns `None` twice a year: once for the
/// "spring forward" gap where the wall-clock time never happens, and once for
/// the "fall back" hour that happens twice. Returning `None` there used to make
/// `compute_next_after` yield no next run, which permanently orphaned the
/// automation. Ambiguous times take the earlier instant; gap times step forward
/// in 15-minute increments until the clock is valid again.
fn resolve_local(day: chrono::NaiveDate, time: NaiveTime) -> Option<DateTime<Local>> {
    use chrono::offset::LocalResult;
    for step in 0..12 {
        let naive = day.and_time(time) + Duration::minutes(15 * step);
        match naive.and_local_timezone(Local) {
            LocalResult::Single(t) => return Some(t),
            LocalResult::Ambiguous(earliest, _) => return Some(earliest),
            LocalResult::None => continue,
        }
    }
    None
}

/// Scheduling always uses the host machine's local timezone. The `timezone`
/// field on a trigger is persisted for future use and is deliberately not read
/// here: honouring arbitrary IANA zones would require a timezone database
/// dependency that Coreside does not currently ship.
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
            let today = resolve_local(local.date_naive(), t)?;
            let candidate = if today > local {
                today
            } else {
                resolve_local(local.date_naive() + Duration::days(1), t)?
            };
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
                    if let Some(candidate) = resolve_local(day, t) {
                        if candidate > local {
                            return Some(candidate.with_timezone(&Utc));
                        }
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

    #[test]
    fn daily_always_produces_a_next_run() {
        // Sweep a full year of local dates so DST transitions in the host
        // timezone cannot produce a `None` that would orphan the automation.
        let start = Utc::now();
        for day in 0..366 {
            let from = start + Duration::days(day);
            for time in ["00:30", "02:30", "03:30", "13:00", "23:45"] {
                let next = compute_next_after(
                    &AutomationTrigger::Daily {
                        time: time.into(),
                        timezone: None,
                    },
                    from,
                );
                assert!(next.is_some(), "no next run for {time} from {from}");
                assert!(next.unwrap() > from);
            }
        }
    }

    #[test]
    fn weekly_always_produces_a_next_run() {
        let start = Utc::now();
        for day in 0..60 {
            let from = start + Duration::days(day);
            for weekday in 0..7 {
                let next = compute_next_after(
                    &AutomationTrigger::Weekly {
                        weekday,
                        time: "02:30".into(),
                        timezone: None,
                    },
                    from,
                );
                assert!(next.is_some(), "no next run for weekday {weekday}");
                assert!(next.unwrap() > from);
            }
        }
    }

    #[test]
    fn invalid_time_has_no_next_run() {
        assert!(compute_next_after(
            &AutomationTrigger::Daily {
                time: "not-a-time".into(),
                timezone: None,
            },
            Utc::now(),
        )
        .is_none());
    }
}
