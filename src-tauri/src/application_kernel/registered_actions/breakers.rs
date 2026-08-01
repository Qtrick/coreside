//! Protected circuit breakers for the registered action runtime.
//! The agent and generated applications cannot raise these ceilings.

use std::collections::HashSet;

use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::db::{Database, DbResult};
use rusqlite::params;

pub const MAX_CALLS_PER_APPLICATION_PER_MINUTE: i64 = 60;
pub const MAX_WRITES_PER_RUN: i64 = 20;
pub const MAX_ACTION_DEPTH: u32 = 8;
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
/// Consecutive failures for one application+action before it is suspended.
pub const FAILURE_SUSPENSION_THRESHOLD: i64 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakerTrip {
    RateLimited,
    WriteBudgetExhausted,
    DepthExceeded,
    InputTooLarge,
    OutputTooLarge,
    DuplicateInFlight,
    Suspended,
}

impl BreakerTrip {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::RateLimited => "Too many actions from this application. Try again in a moment.",
            Self::WriteBudgetExhausted => "This run reached its limit for changes.",
            Self::DepthExceeded => "Actions are nested too deeply.",
            Self::InputTooLarge => "The action input is too large.",
            Self::OutputTooLarge => "The action result is too large to return.",
            Self::DuplicateInFlight => "This action is already running.",
            Self::Suspended => {
                "This action keeps failing and is paused until the application is reviewed."
            }
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::RateLimited => "rate_limited",
            Self::WriteBudgetExhausted => "write_budget_exhausted",
            Self::DepthExceeded => "depth_exceeded",
            Self::InputTooLarge => "input_too_large",
            Self::OutputTooLarge => "output_too_large",
            Self::DuplicateInFlight => "duplicate_in_flight",
            Self::Suspended => "action_suspended",
        }
    }
}

static IN_FLIGHT: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// RAII marker for one in-flight call. Dropping it releases the slot even if
/// the handler panics or returns early.
pub struct InFlightGuard(String);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        IN_FLIGHT.lock().remove(&self.0);
    }
}

pub fn begin_in_flight(call_key: &str) -> Option<InFlightGuard> {
    let mut set = IN_FLIGHT.lock();
    if set.contains(call_key) {
        return None;
    }
    set.insert(call_key.to_string());
    Some(InFlightGuard(call_key.to_string()))
}

pub fn check_depth(depth: u32) -> Option<BreakerTrip> {
    (depth >= MAX_ACTION_DEPTH).then_some(BreakerTrip::DepthExceeded)
}

pub fn check_input_size(input: &serde_json::Value) -> Option<BreakerTrip> {
    (input.to_string().len() > MAX_INPUT_BYTES).then_some(BreakerTrip::InputTooLarge)
}

pub fn check_output_size(output: &serde_json::Value) -> Option<BreakerTrip> {
    (output.to_string().len() > MAX_OUTPUT_BYTES).then_some(BreakerTrip::OutputTooLarge)
}

/// Calls attributed to one application in the last minute.
pub fn check_rate(db: &Database, application_id: Option<&str>) -> DbResult<Option<BreakerTrip>> {
    let Some(app) = application_id else {
        return Ok(None);
    };
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM runtime_audit_events
         WHERE kind = 'action' AND application_id = ?1
           AND created_at > ?2",
        params![app, minutes_ago(1)],
        |r| r.get(0),
    )?;
    Ok((count >= MAX_CALLS_PER_APPLICATION_PER_MINUTE).then_some(BreakerTrip::RateLimited))
}

/// Non-read actions already applied within one run.
pub fn check_write_budget(db: &Database, run_id: &str) -> DbResult<Option<BreakerTrip>> {
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM runtime_audit_events
         WHERE kind = 'action' AND run_id = ?1 AND outcome = 'ok'
           AND risk IN ('write','destructive')",
        [run_id],
        |r| r.get(0),
    )?;
    Ok((count >= MAX_WRITES_PER_RUN).then_some(BreakerTrip::WriteBudgetExhausted))
}

/// Write budget applies only to non-read actions.
pub fn check_write_budget_if_write(
    db: &Database,
    run_id: &str,
    risk: super::descriptor::ActionRisk,
) -> DbResult<Option<BreakerTrip>> {
    if risk == super::descriptor::ActionRisk::Read {
        return Ok(None);
    }
    check_write_budget(db, run_id)
}

/// Suspend an application+action pair after repeated consecutive failures.
pub fn check_failure_suspension(
    db: &Database,
    application_id: Option<&str>,
    action_name: &str,
) -> DbResult<Option<BreakerTrip>> {
    let Some(app) = application_id else {
        return Ok(None);
    };
    let mut stmt = db.conn().prepare(
        "SELECT outcome FROM runtime_audit_events
         WHERE kind = 'action' AND application_id = ?1 AND action_name = ?2
         ORDER BY created_at DESC, rowid DESC LIMIT ?3",
    )?;
    let outcomes: Vec<String> = stmt
        .query_map(
            params![app, action_name, FAILURE_SUSPENSION_THRESHOLD],
            |r| r.get(0),
        )?
        .filter_map(|r| r.ok())
        .collect();
    let tripped = outcomes.len() as i64 >= FAILURE_SUSPENSION_THRESHOLD
        && outcomes.iter().all(|o| o == "error");
    Ok(tripped.then_some(BreakerTrip::Suspended))
}

fn minutes_ago(minutes: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::minutes(minutes)).to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_kernel::registered_actions::testing::{insert_audit_row, test_db};
    use serde_json::json;

    #[test]
    fn depth_and_size_limits() {
        assert!(check_depth(0).is_none());
        assert!(check_depth(MAX_ACTION_DEPTH).is_some());
        let big = json!({ "x": "y".repeat(MAX_INPUT_BYTES + 10) });
        assert_eq!(check_input_size(&big), Some(BreakerTrip::InputTooLarge));
        assert_eq!(check_output_size(&big), Some(BreakerTrip::OutputTooLarge));
        assert!(check_input_size(&json!({ "x": 1 })).is_none());
    }

    #[test]
    fn duplicate_in_flight_is_suppressed() {
        let key = "test-dup-key";
        let first = begin_in_flight(key).expect("first call takes the slot");
        assert!(begin_in_flight(key).is_none());
        drop(first);
        assert!(begin_in_flight(key).is_some());
    }

    #[test]
    fn rate_limit_trips_after_ceiling() {
        let (mut db, _dir) = test_db();
        for _ in 0..MAX_CALLS_PER_APPLICATION_PER_MINUTE {
            insert_audit_row(&mut db, "app-1", "local_data.query", "ok", "read", "run-1");
        }
        assert_eq!(
            check_rate(&db, Some("app-1")).unwrap(),
            Some(BreakerTrip::RateLimited)
        );
        assert!(check_rate(&db, Some("app-2")).unwrap().is_none());
    }

    #[test]
    fn write_budget_counts_only_writes() {
        let (mut db, _dir) = test_db();
        for _ in 0..MAX_WRITES_PER_RUN {
            insert_audit_row(&mut db, "app-1", "local_data.query", "ok", "read", "run-1");
        }
        assert!(check_write_budget(&db, "run-1").unwrap().is_none());
        for _ in 0..MAX_WRITES_PER_RUN {
            insert_audit_row(&mut db, "app-1", "local_data.write", "ok", "write", "run-1");
        }
        assert_eq!(
            check_write_budget(&db, "run-1").unwrap(),
            Some(BreakerTrip::WriteBudgetExhausted)
        );
        assert!(check_write_budget(&db, "run-2").unwrap().is_none());
    }

    #[test]
    fn repeated_failures_suspend_then_recover() {
        let (mut db, _dir) = test_db();
        for _ in 0..FAILURE_SUSPENSION_THRESHOLD {
            insert_audit_row(&mut db, "app-1", "local_data.write", "error", "write", "run-1");
        }
        assert_eq!(
            check_failure_suspension(&db, Some("app-1"), "local_data.write").unwrap(),
            Some(BreakerTrip::Suspended)
        );
        insert_audit_row(&mut db, "app-1", "local_data.write", "ok", "write", "run-1");
        assert!(check_failure_suspension(&db, Some("app-1"), "local_data.write")
            .unwrap()
            .is_none());
    }
}
