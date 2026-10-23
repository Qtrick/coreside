use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Timelike, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

/// Floor used when advancing interval schedules (must stay ≥ validation::MIN_INTERVAL_MINUTES).
const MIN_SCHEDULE_INTERVAL_MINUTES: u32 = 5;

/// Parse an optional IANA timezone. Empty / missing → host local zone at schedule time.
/// Invalid names fail closed so we never silently ignore an explicit zone.
pub fn parse_trigger_timezone(timezone: Option<&str>) -> Result<Option<Tz>, String> {
    let Some(raw) = timezone.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    raw.parse::<Tz>()
        .map(Some)
        .map_err(|_| format!("Unsupported timezone '{raw}'. Use an IANA name such as America/New_York."))
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AutomationTrigger {
    #[serde(rename = "interval")]
    Interval {
        // Persist snake_case; accept camelCase from chat / Tauri IPC.
        #[serde(alias = "intervalMinutes")]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AutomationAction {
    #[serde(rename = "cycle_workspace_backgrounds")]
    CycleWorkspaceBackgrounds {
        #[serde(alias = "workspaceId")]
        workspace_id: String,
        #[serde(alias = "presetIds")]
        preset_ids: Vec<String>,
    },
    #[serde(rename = "set_workspace_background")]
    SetWorkspaceBackground {
        #[serde(alias = "workspaceId")]
        workspace_id: String,
        #[serde(alias = "presetId")]
        preset_id: String,
    },
    #[serde(rename = "set_tool_state_value")]
    SetToolStateValue {
        #[serde(alias = "toolId")]
        tool_id: String,
        path: String,
        value: serde_json::Value,
    },
    #[serde(rename = "ai_prompt")]
    AiPrompt {
        prompt: String,
        #[serde(default, alias = "conversationId")]
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

/// Resolve a wall-clock date+time in `tz` across DST boundaries.
///
/// Ambiguous times (fall back) take the earlier instant. Nonexistent times
/// (spring forward) step forward in 15-minute increments until valid.
fn resolve_in_tz(day: chrono::NaiveDate, time: NaiveTime, tz: Tz) -> Option<DateTime<Tz>> {
    use chrono::offset::LocalResult;
    for step in 0..12 {
        let naive = day.and_time(time) + Duration::minutes(15 * step);
        match tz.from_local_datetime(&naive) {
            LocalResult::Single(t) => return Some(t),
            LocalResult::Ambiguous(earliest, _) => return Some(earliest),
            LocalResult::None => continue,
        }
    }
    None
}

fn resolve_host_local(day: chrono::NaiveDate, time: NaiveTime) -> Option<DateTime<Local>> {
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

fn trigger_timezone(trigger: &AutomationTrigger) -> Option<&str> {
    match trigger {
        AutomationTrigger::Interval { timezone, .. }
        | AutomationTrigger::Daily { timezone, .. }
        | AutomationTrigger::Weekly { timezone, .. } => timezone.as_deref(),
    }
}

/// Next run after `from`. Explicit IANA `timezone` fields are honoured; missing
/// timezone uses the host local zone. Invalid zones yield `None` (caller should
/// have failed validation earlier).
pub fn compute_next_after(
    trigger: &AutomationTrigger,
    from: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    match trigger {
        AutomationTrigger::Interval {
            interval_minutes, ..
        } => {
            // Defense in depth: never schedule faster than the non-AI floor,
            // even if a row was tampered below validation limits.
            let mins = (*interval_minutes).max(MIN_SCHEDULE_INTERVAL_MINUTES) as i64;
            return Some(from + Duration::minutes(mins));
        }
        _ => {}
    }

    let tz = match parse_trigger_timezone(trigger_timezone(trigger)) {
        Ok(z) => z,
        Err(_) => return None,
    };

    if let Some(tz) = tz {
        let local = from.with_timezone(&tz);
        match trigger {
            AutomationTrigger::Daily { time, .. } => {
                let t = parse_hhmm(time)?;
                let today = resolve_in_tz(local.date_naive(), t, tz)?;
                let candidate = if today > local {
                    today
                } else {
                    resolve_in_tz(local.date_naive() + Duration::days(1), t, tz)?
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
                        if let Some(candidate) = resolve_in_tz(day, t, tz) {
                            if candidate > local {
                                return Some(candidate.with_timezone(&Utc));
                            }
                        }
                    }
                    day += Duration::days(1);
                }
                None
            }
            AutomationTrigger::Interval { .. } => unreachable!(),
        }
    } else {
        let local = from.with_timezone(&Local);
        match trigger {
            AutomationTrigger::Daily { time, .. } => {
                let t = parse_hhmm(time)?;
                let today = resolve_host_local(local.date_naive(), t)?;
                let candidate = if today > local {
                    today
                } else {
                    resolve_host_local(local.date_naive() + Duration::days(1), t)?
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
                        if let Some(candidate) = resolve_host_local(day, t) {
                            if candidate > local {
                                return Some(candidate.with_timezone(&Utc));
                            }
                        }
                    }
                    day += Duration::days(1);
                }
                None
            }
            AutomationTrigger::Interval { .. } => unreachable!(),
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

    #[test]
    fn action_deserializes_camel_case_from_chat_ipc() {
        let raw = serde_json::json!({
            "type": "set_workspace_background",
            "workspaceId": "workspace-default",
            "presetId": "preset-1"
        });
        let action: AutomationAction = serde_json::from_value(raw).unwrap();
        assert_eq!(
            action,
            AutomationAction::SetWorkspaceBackground {
                workspace_id: "workspace-default".into(),
                preset_id: "preset-1".into(),
            }
        );
        // Persisted snake_case rows must keep loading.
        let legacy = serde_json::json!({
            "type": "set_workspace_background",
            "workspace_id": "workspace-default",
            "preset_id": "preset-1"
        });
        assert_eq!(
            serde_json::from_value::<AutomationAction>(legacy).unwrap(),
            action
        );
    }

    #[test]
    fn invalid_timezone_parse_fails_closed() {
        let err = parse_trigger_timezone(Some("Not/A_Zone")).unwrap_err();
        assert!(err.contains("Unsupported timezone"));
        assert!(compute_next_after(
            &AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: Some("Not/A_Zone".into()),
            },
            Utc::now(),
        )
        .is_none());
    }

    #[test]
    fn explicit_timezone_shifts_next_run_vs_utc_naive() {
        // 2026-03-10 12:00 UTC = 21:00 Asia/Tokyo → next 08:00 Tokyo is Mar 11 08:00 JST
        // = 2026-03-10T23:00:00Z.
        let from = DateTime::parse_from_rfc3339("2026-03-10T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next_host = compute_next_after(
            &AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: None,
            },
            from,
        )
        .unwrap();
        let next_tokyo = compute_next_after(
            &AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: Some("Asia/Tokyo".into()),
            },
            from,
        )
        .unwrap();
        let tokyo = next_tokyo.with_timezone(&chrono_tz::Asia::Tokyo);
        assert_eq!(tokyo.time().hour(), 8);
        assert_eq!(tokyo.time().minute(), 0);
        assert_eq!(tokyo.date_naive().to_string(), "2026-03-11");
        assert_eq!(
            next_tokyo,
            DateTime::parse_from_rfc3339("2026-03-10T23:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
        // Host-local (no timezone) is a different schedule contract than Asia/Tokyo.
        let _ = next_host;
    }

    #[test]
    fn empty_timezone_string_uses_host_local_not_fail_closed() {
        let from = DateTime::parse_from_rfc3339("2026-06-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(parse_trigger_timezone(Some("")).unwrap().is_none());
        assert!(parse_trigger_timezone(Some("   ")).unwrap().is_none());
        assert!(compute_next_after(
            &AutomationTrigger::Daily {
                time: "09:00".into(),
                timezone: Some("".into()),
            },
            from,
        )
        .is_some());
    }
}
