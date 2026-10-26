use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use parking_lot::Mutex;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::ai_exec::{finish_ai_prepare, run_prepared_ai, snapshot_ai_prepare};
use super::executor::execute_automation;
use super::models::{compute_next_after, Automation, AutomationAction, MissedRunPolicy};
use super::outcome::AutomationRunOutcome;
use super::validation::MAX_CONSECUTIVE_FAILURES;
use crate::db::{self, Database};
use crate::state::AppState;

pub use super::models::compute_next_after as compute_next_run;

#[derive(Default)]
pub struct SchedulerHandle {
    running: Mutex<HashSet<String>>,
    /// Ensures at most one ticker loop (setup or post-recovery restore).
    started: AtomicBool,
    /// When true, due automations are skipped (restore / maintenance).
    paused: AtomicBool,
}

impl SchedulerHandle {
    pub fn try_begin(&self, id: &str) -> bool {
        let mut guard = self.running.lock();
        if guard.contains(id) {
            return false;
        }
        guard.insert(id.to_string());
        true
    }

    pub fn end(&self, id: &str) {
        self.running.lock().remove(id);
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn status(&self) -> SchedulerStatus {
        SchedulerStatus {
            started: self.started.load(Ordering::SeqCst),
            paused: self.is_paused(),
            running_count: self.running.lock().len(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerStatus {
    pub started: bool,
    pub paused: bool,
    pub running_count: usize,
}

/// Start the automation ticker on Tauri's async runtime (not a bare `tokio::spawn`
/// from `setup`, which has no reactor and aborts the process).
///
/// Idempotent: a second call after recovery restore is a no-op if already running.
pub fn spawn_scheduler(app: AppHandle, handle: Arc<SchedulerHandle>) {
    if handle.started.swap(true, Ordering::SeqCst) {
        tracing::debug!("automation scheduler already running");
        return;
    }
    tauri::async_runtime::spawn(async move {
        {
            if let Some(state) = app.try_state::<AppState>() {
                if state.profile_ready() {
                    let mut db = state.db.lock();
                    if let Err(e) = db::reconcile_interrupted_runs(db.conn()) {
                        tracing::warn!(error = %e, "reconciling interrupted automation runs failed");
                    }
                    if let Err(e) = catch_up_missed(&mut db, &handle) {
                        tracing::warn!(error = %e, "automation missed-run catch-up failed");
                    }
                }
            }
        }

        let mut ticker = tokio::time::interval(Duration::from_secs(30));
        loop {
            ticker.tick().await;
            let Some(state) = app.try_state::<AppState>() else {
                continue;
            };
            // Never schedule against the recovery shell database.
            if !state.profile_ready() {
                continue;
            }
            if handle.is_paused() {
                continue;
            }
            let due = {
                let db = state.db.lock();
                match db::list_due_automations(&db, &Utc::now().to_rfc3339()) {
                    Ok(list) => list,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to list due automations");
                        Vec::new()
                    }
                }
            };
            // Bound concurrent automation workers to avoid DB mutex stampedes
            // when many schedules are overdue.
            let mut started = 0usize;
            const MAX_CONCURRENT_RUNS: usize = 2;
            for automation in due {
                if started >= MAX_CONCURRENT_RUNS {
                    break;
                }
                if !handle.try_begin(&automation.id) {
                    continue;
                }
                started += 1;
                let app_clone = app.clone();
                let handle_clone = handle.clone();
                let id = automation.id.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let parked_before = pending_approval_total(&app_clone);
                    if let Some(state) = app_clone.try_state::<AppState>() {
                        if state.profile_ready() {
                            run_one(&state, &id);
                        }
                    }
                    handle_clone.end(&id);
                    // An away run that parks an approval must reach open windows
                    // without waiting for a focus event.
                    use tauri::Emitter;
                    let _ = app_clone.emit(crate::commands::AUTOMATIONS_CHANGED_EVENT, ());
                    if pending_approval_total(&app_clone) != parked_before {
                        let _ = app_clone.emit(crate::commands::APPROVALS_CHANGED_EVENT, ());
                    }
                });
            }
        }
    });
}

fn catch_up_missed(db: &mut Database, handle: &SchedulerHandle) -> Result<(), db::DbError> {
    if handle.is_paused() {
        return Ok(());
    }
    db::reconcile_interrupted_runs(db.conn())?;
    let now = Utc::now();
    let overdue = db::list_due_automations(db, &now.to_rfc3339())?;
    for automation in overdue {
        if automation.missed_run_policy == MissedRunPolicy::Skip {
            if let Some(next) = compute_next_after(&automation.trigger, now) {
                if let Err(e) = db::update_automation_schedule(
                    db,
                    &automation.id,
                    Some(&next.to_rfc3339()),
                    automation.last_run_at.as_deref(),
                    Some("skipped_missed"),
                    automation.consecutive_failures,
                    automation.enabled,
                ) {
                    tracing::error!(automation_id = %automation.id, error = %e, "failed to update skipped automation schedule");
                }
            }
            continue;
        }
        // AI catch-up needs the unlocked ticker path so provider I/O does not
        // hold the profile DB mutex during startup.
        if matches!(automation.action, AutomationAction::AiPrompt { .. }) {
            continue;
        }
        if handle.try_begin(&automation.id) {
            if let Err(e) = run_one_db(db, &automation.id, RunOrigin::CatchUp) {
                tracing::error!(automation_id = %automation.id, error = %e, "failed to run automation during catch-up");
            }
            handle.end(&automation.id);
        }
    }
    Ok(())
}

fn pending_approval_total(app: &AppHandle) -> i64 {
    app.try_state::<AppState>()
        .and_then(|state| {
            let db = state.db.lock();
            crate::application_kernel::registered_actions::gateway::pending_approval_count(&db).ok()
        })
        .unwrap_or(-1)
}

fn run_one(state: &AppState, automation_id: &str) {
    if let Err(e) = run_one_releasing_lock_for_ai(state, automation_id, RunOrigin::Scheduled) {
        tracing::error!(automation_id = %automation_id, error = %e, "automation execution failed");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOrigin {
    Scheduled,
    Manual,
    CatchUp,
}

pub struct ClaimedRun {
    pub automation: Automation,
    pub run_id: String,
    pub started: String,
    pub next: Option<String>,
    pub schedulable: bool,
    pub scheduled_at: Option<String>,
    pub origin: RunOrigin,
}

pub fn claim_run(
    db: &mut Database,
    automation_id: &str,
    origin: RunOrigin,
) -> Result<Option<ClaimedRun>, db::DbError> {
    let automation = db::get_automation(db, automation_id)?;
    if !automation.enabled {
        return Ok(None);
    }
    // Exclusivity: prevent duplicate concurrent runs across workers/connections.
    if automation.last_status.as_deref() == Some("running") {
        tracing::debug!(
            automation_id,
            "automation is already running; skipping duplicate claim"
        );
        return Ok(None);
    }

    let started_dt = Utc::now();
    let started = started_dt.to_rfc3339();
    let run_id = format!("run-{}", Uuid::new_v4());

    // Scheduled/catch-up runs advance next_run_at to the subsequent cycle.
    // Manual "Run now" executions do NOT silently shift or skip a fixed daily/weekly/weekday schedule.
    let (next, scheduled_at) = match origin {
        RunOrigin::Manual => (automation.next_run_at.clone(), None),
        RunOrigin::Scheduled | RunOrigin::CatchUp => {
            let next_dt =
                compute_next_after(&automation.trigger, started_dt).map(|d| d.to_rfc3339());
            (next_dt, automation.next_run_at.clone())
        }
    };

    let schedulable = match origin {
        RunOrigin::Manual => true,
        _ => next.is_some(),
    };
    if !schedulable {
        tracing::warn!(
            automation_id,
            "automation trigger has no next run; disabling"
        );
    }

    db.with_transaction(|conn| {
        db::insert_automation_run_conn(
            conn,
            &run_id,
            automation_id,
            scheduled_at.as_deref(),
            &started,
            None,
            "running",
            None,
            None,
        )?;
        db::update_automation_schedule_conn(
            conn,
            automation_id,
            next.as_deref(),
            Some(&started),
            Some("running"),
            automation.consecutive_failures,
            schedulable && automation.enabled,
        )?;
        Ok(())
    })?;

    Ok(Some(ClaimedRun {
        automation,
        run_id,
        started,
        next,
        schedulable,
        scheduled_at,
        origin,
    }))
}

pub fn finalize_run(
    db: &mut Database,
    claim: &ClaimedRun,
    outcome: &AutomationRunOutcome,
) -> Result<(), db::DbError> {
    let completed = Utc::now().to_rfc3339();

    db.with_transaction(|conn| {
        // Authoritative current state check:
        let current = match db::get_automation_conn(conn, &claim.automation.id) {
            Ok(a) => Some(a),
            Err(db::DbError::NotFound(_)) => None,
            Err(e) => return Err(e),
        };

        let Some(current) = current else {
            // The automation was deleted while in flight.
            // Foreign key CASCADE has already removed automation_runs for this id.
            tracing::info!(
                automation_id = %claim.automation.id,
                "automation was deleted during in-flight execution; skipping finalization write"
            );
            return Ok(());
        };

        // Failure count: computed from current authoritative state
        let failures = if outcome.counts_as_failure() {
            current.consecutive_failures + 1
        } else if outcome.status == "success" {
            0_i64
        } else {
            current.consecutive_failures
        };
        let keep_enabled = if outcome.counts_as_failure() {
            failures < MAX_CONSECUTIVE_FAILURES
        } else {
            true
        };

        // If the user paused the automation during execution (current.enabled == false),
        // it MUST REMAIN PAUSED! Finalization must never silently re-enable it.
        let final_enabled = current.enabled && claim.schedulable && keep_enabled;

        // If user edited the trigger or manually rescheduled next_run_at during execution,
        // preserve the user's new schedule:
        let user_rescheduled = current.trigger != claim.automation.trigger
            || (current.next_run_at != claim.next && current.next_run_at != claim.automation.next_run_at);

        let final_next = if user_rescheduled {
            current.next_run_at
        } else {
            claim.next.clone()
        };

        // Prevent older overlapping runs from overwriting a newer run's terminal status:
        let newer_run_completed = current
            .last_run_at
            .as_deref()
            .is_some_and(|l| l > claim.started.as_str());
        let (final_last_run, final_last_status) = if newer_run_completed {
            (current.last_run_at, current.last_status)
        } else {
            (Some(completed.clone()), Some(outcome.status.clone()))
        };

        // Update the run record:
        let _ = db::update_automation_run_conn(
            conn,
            &claim.run_id,
            Some(&completed),
            &outcome.status,
            Some(outcome.summary.as_str()),
            outcome.error_category.as_deref(),
        );

        // Update the automation schedule:
        db::update_automation_schedule_conn(
            conn,
            &claim.automation.id,
            final_next.as_deref(),
            final_last_run.as_deref(),
            final_last_status.as_deref(),
            failures,
            final_enabled,
        )?;

        // Prune old runs (keep latest 100)
        if let Err(e) = conn.execute(
            "DELETE FROM automation_runs WHERE automation_id = ?1 AND id NOT IN (
                SELECT id FROM (
                    SELECT id FROM automation_runs WHERE automation_id = ?1
                    ORDER BY created_at DESC LIMIT 100
                )
             )",
            rusqlite::params![claim.automation.id],
        ) {
            tracing::warn!(automation_id = %claim.automation.id, error = %e, "pruning old automation runs failed");
        }

        Ok(())
    })?;

    Ok(())
}

fn execute_claimed(db: &mut Database, claim: &ClaimedRun) -> AutomationRunOutcome {
    match execute_automation(db, &claim.automation) {
        Ok(outcome) => outcome,
        Err(e) => AutomationRunOutcome::error(e.to_string(), "execution_error"),
    }
}

fn execute_ai_unlocked(state: &AppState, claim: &ClaimedRun) -> AutomationRunOutcome {
    let AutomationAction::AiPrompt { prompt, .. } = &claim.automation.action else {
        return AutomationRunOutcome::error("Expected AI prompt action", "validation");
    };
    if !claim.automation.requires_ai {
        return AutomationRunOutcome::error("AI prompt actions must set requiresAi", "validation");
    }
    // DB snapshot only under the mutex; keyring + provider I/O run unlocked.
    let snapshot = {
        let db = state.db.lock();
        snapshot_ai_prepare(&db, &claim.automation, &claim.run_id, prompt)
    };
    match snapshot.and_then(finish_ai_prepare) {
        Ok(prepared) => run_prepared_ai(prepared),
        Err(outcome) => outcome,
    }
}

/// Production path: release the DB mutex before provider I/O for AI automations.
fn run_one_releasing_lock_for_ai(
    state: &AppState,
    automation_id: &str,
    origin: RunOrigin,
) -> Result<AutomationRunOutcome, db::DbError> {
    let claim = {
        let mut db = state.db.lock();
        claim_run(&mut db, automation_id, origin)?
    };
    let Some(claim) = claim else {
        return Ok(AutomationRunOutcome::skipped(
            "Automation is disabled or currently busy",
        ));
    };
    let outcome = if matches!(claim.automation.action, AutomationAction::AiPrompt { .. }) {
        execute_ai_unlocked(state, &claim)
    } else {
        let mut db = state.db.lock();
        execute_claimed(&mut db, &claim)
    };
    {
        let mut db = state.db.lock();
        finalize_run(&mut db, &claim, &outcome)?;
    }
    Ok(outcome)
}

/// Single-lock path for tests and non-AI catch-up (must not call provider I/O).
pub fn run_one_db(
    db: &mut Database,
    automation_id: &str,
    origin: RunOrigin,
) -> Result<AutomationRunOutcome, db::DbError> {
    let Some(claim) = claim_run(db, automation_id, origin)? else {
        return Ok(AutomationRunOutcome::skipped(
            "Automation is disabled or currently busy",
        ));
    };
    let outcome = execute_claimed(db, &claim);
    finalize_run(db, &claim, &outcome)?;
    Ok(outcome)
}

pub fn run_now(
    state: &AppState,
    automation_id: &str,
    handle: &SchedulerHandle,
) -> Result<AutomationRunOutcome, String> {
    if !state.profile_ready() {
        return Err("Automations are unavailable until Recovery finishes.".into());
    }
    if !handle.try_begin(automation_id) {
        return Ok(AutomationRunOutcome::busy());
    }
    let result = run_one_releasing_lock_for_ai(state, automation_id, RunOrigin::Manual);
    handle.end(automation_id);
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automations::models::{
        Automation, AutomationAction, AutomationTrigger, MissedRunPolicy,
    };
    use crate::db::Database;
    use tempfile::tempdir;

    #[test]
    fn catch_up_missed_is_noop_when_paused() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("sched.db")).unwrap();
        let now = Utc::now();
        let past = (now - chrono::Duration::hours(2)).to_rfc3339();
        db::ensure_default_workspace(&db).unwrap();
        let auto = Automation {
            id: "auto-paused".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "paused catch-up".into(),
            enabled: true,
            trigger: AutomationTrigger::Interval {
                interval_minutes: 60,
                timezone: None,
            },
            action: AutomationAction::SetWorkspaceBackground {
                workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
                preset_id: "missing-preset".into(),
            },
            requires_ai: false,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(past.clone()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: past.clone(),
            updated_at: past.clone(),
        };
        db::upsert_automation(&mut db, &auto).unwrap();
        let handle = SchedulerHandle::default();
        handle.pause();
        catch_up_missed(&mut db, &handle).unwrap();
        let after = db::get_automation(&db, "auto-paused").unwrap();
        assert_eq!(after.next_run_at.as_deref(), Some(past.as_str()));
        assert_eq!(after.last_status, None);
        let runs = db::list_automation_runs(&db, "auto-paused", 10).unwrap();
        assert!(
            runs.is_empty(),
            "paused catch-up must not execute automations"
        );
    }

    #[test]
    fn awaiting_approval_does_not_increment_consecutive_failures() {
        use crate::application_kernel::registered_actions::testing::seed_application;
        use serde_json::json;

        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("await-fail.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        const APP: &str = "app-auto-park";
        seed_application(&mut db, APP, &["tool_state.set"]);

        let past = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        let auto = Automation {
            id: "auto-park".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: Some(APP.into()),
            name: "park for approval".into(),
            enabled: true,
            trigger: AutomationTrigger::Interval {
                interval_minutes: 60,
                timezone: None,
            },
            action: AutomationAction::SetToolStateValue {
                tool_id: APP.into(),
                path: "count".into(),
                value: json!(1),
            },
            requires_ai: false,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(past.clone()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 2,
            waiting_approval: false,
            permission_ready: true,
            created_at: past.clone(),
            updated_at: past,
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        let outcome = run_one_db(&mut db, "auto-park", RunOrigin::Scheduled).unwrap();
        assert_eq!(
            outcome.status, "awaiting_approval",
            "away tool_state.set without standing grant must park: {}",
            outcome.summary
        );

        let after = db::get_automation(&db, "auto-park").unwrap();
        assert_eq!(
            after.consecutive_failures, 2,
            "awaiting_approval must not escalate consecutive failures"
        );
        assert!(
            after.enabled,
            "parked approval must not disable the automation"
        );
        assert_eq!(after.last_status.as_deref(), Some("awaiting_approval"));
        assert!(after.waiting_approval);
    }

    #[test]
    fn real_failure_increments_consecutive_failures() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("real-fail.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let past = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        let auto = Automation {
            id: "auto-fail".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "missing preset".into(),
            enabled: true,
            trigger: AutomationTrigger::Interval {
                interval_minutes: 60,
                timezone: None,
            },
            action: AutomationAction::SetWorkspaceBackground {
                workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
                preset_id: "does-not-exist".into(),
            },
            requires_ai: false,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(past.clone()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 1,
            waiting_approval: false,
            permission_ready: true,
            created_at: past.clone(),
            updated_at: past,
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        let outcome = run_one_db(&mut db, "auto-fail", RunOrigin::Scheduled).unwrap();
        assert!(
            outcome.counts_as_failure(),
            "missing preset should be a failure: {} / {:?}",
            outcome.status,
            outcome.error_category
        );
        let after = db::get_automation(&db, "auto-fail").unwrap();
        assert_eq!(after.consecutive_failures, 2);
    }

    #[test]
    fn claim_run_persists_running_run_record_before_completion() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("claim-persist.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let past = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
        let auto = Automation {
            id: "auto-claim-test".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "Claim test".into(),
            enabled: true,
            trigger: AutomationTrigger::Interval {
                interval_minutes: 30,
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Review unfinished work".into(),
                conversation_id: None,
            },
            requires_ai: true,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(past.clone()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: past.clone(),
            updated_at: past,
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        let claim = claim_run(&mut db, "auto-claim-test", RunOrigin::Scheduled)
            .expect("claim succeeds")
            .expect("claim not none");
        assert_eq!(claim.automation.id, "auto-claim-test");

        // Verify that in SQLite, the run row is ALREADY inserted with status = "running"
        let runs = db::list_automation_runs(&db, "auto-claim-test", 10).unwrap();
        assert_eq!(
            runs.len(),
            1,
            "claim must insert run row before effects start"
        );
        assert_eq!(runs[0].id, claim.run_id);
        assert_eq!(runs[0].status, "running");
        assert!(runs[0].completed_at.is_none());

        // Verify that automation schedule was advanced and last_status is "running"
        let current_auto = db::get_automation(&db, "auto-claim-test").unwrap();
        assert_eq!(current_auto.last_status.as_deref(), Some("running"));
        assert!(current_auto.next_run_at.unwrap() > claim.started);

        // Finalize the run
        let outcome = AutomationRunOutcome::success("Generated 3 suggestions");
        finalize_run(&mut db, &claim, &outcome).unwrap();

        // Verify run record updated to terminal outcome
        let finalized_runs = db::list_automation_runs(&db, "auto-claim-test", 10).unwrap();
        assert_eq!(finalized_runs.len(), 1);
        assert_eq!(finalized_runs[0].status, "success");
        assert_eq!(
            finalized_runs[0].result_summary.as_deref(),
            Some("Generated 3 suggestions")
        );
        assert!(finalized_runs[0].completed_at.is_some());

        let final_auto = db::get_automation(&db, "auto-claim-test").unwrap();
        assert_eq!(final_auto.last_status.as_deref(), Some("success"));
    }

    #[test]
    fn reconcile_interrupted_runs_recovers_stale_claims() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("reconcile-test.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let now = Utc::now().to_rfc3339();

        let auto = Automation {
            id: "auto-orphaned".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "Orphaned test".into(),
            enabled: true,
            trigger: AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Test".into(),
                conversation_id: None,
            },
            requires_ai: true,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(now.clone()),
            last_run_at: None,
            last_status: Some("running".into()),
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        // Simulate crash leaving run row in "running"
        db::insert_automation_run(
            &mut db,
            "run-stale-1",
            "auto-orphaned",
            Some(&now),
            &now,
            None,
            "running",
            None,
            None,
        )
        .unwrap();

        // On reboot / startup, scheduler calls reconcile_interrupted_runs
        let reconciled = db::reconcile_interrupted_runs(db.conn()).unwrap();
        assert_eq!(reconciled, 1);

        let runs = db::list_automation_runs(&db, "auto-orphaned", 10).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "interrupted");
        assert!(runs[0].completed_at.is_some());
        assert!(runs[0]
            .result_summary
            .as_deref()
            .unwrap()
            .contains("interrupted"));

        let updated_auto = db::get_automation(&db, "auto-orphaned").unwrap();
        assert_eq!(updated_auto.last_status.as_deref(), Some("interrupted"));
    }

    #[test]
    fn pause_during_run_remains_paused_after_finalization() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("pause-race.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let now = Utc::now().to_rfc3339();

        let mut auto = Automation {
            id: "auto-pause-race".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "Pause Race".into(),
            enabled: true,
            trigger: AutomationTrigger::Interval {
                interval_minutes: 60,
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Test".into(),
                conversation_id: None,
            },
            requires_ai: true,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(now.clone()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        // 1. Claim the run while enabled
        let claim = claim_run(&mut db, "auto-pause-race", RunOrigin::Scheduled)
            .unwrap()
            .unwrap();

        // 2. User pauses the automation while the AI request is in flight
        auto.enabled = false;
        auto.updated_at = Utc::now().to_rfc3339();
        db::upsert_automation(&mut db, &auto).unwrap();

        let paused_before = db::get_automation(&db, "auto-pause-race").unwrap();
        assert!(
            !paused_before.enabled,
            "user pause must take effect immediately"
        );

        // 3. In-flight request finishes and calls finalize_run
        let outcome = AutomationRunOutcome::success("AI output completed");
        finalize_run(&mut db, &claim, &outcome).unwrap();

        // 4. Authoritative check: automation MUST REMAIN PAUSED, not silently re-enabled!
        let finalized_auto = db::get_automation(&db, "auto-pause-race").unwrap();
        assert!(
            !finalized_auto.enabled,
            "stale claim must NOT re-enable an automation paused during in-flight execution"
        );
        assert_eq!(finalized_auto.last_status.as_deref(), Some("success"));
    }

    #[test]
    fn reschedule_during_run_is_preserved_after_finalization() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("resched-race.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let now = Utc::now();
        let t1 = now.to_rfc3339();
        let user_t3 = (now + chrono::Duration::hours(5)).to_rfc3339();

        let mut auto = Automation {
            id: "auto-resched-race".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "Reschedule Race".into(),
            enabled: true,
            trigger: AutomationTrigger::Interval {
                interval_minutes: 60,
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Test".into(),
                conversation_id: None,
            },
            requires_ai: true,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(t1),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        // 1. Claim run
        let claim = claim_run(&mut db, "auto-resched-race", RunOrigin::Scheduled)
            .unwrap()
            .unwrap();

        // 2. User modifies trigger to 120 minutes and sets a custom next_run_at during the run
        auto.trigger = AutomationTrigger::Interval {
            interval_minutes: 120,
            timezone: None,
        };
        auto.next_run_at = Some(user_t3.clone());
        auto.updated_at = Utc::now().to_rfc3339();
        db::upsert_automation(&mut db, &auto).unwrap();

        // 3. Finalize run
        let outcome = AutomationRunOutcome::success("Done");
        finalize_run(&mut db, &claim, &outcome).unwrap();

        // 4. Verify user-edited next_run_at is NOT overwritten by the old computed progression!
        let after = db::get_automation(&db, "auto-resched-race").unwrap();
        assert_eq!(
            after.next_run_at.as_deref(),
            Some(user_t3.as_str()),
            "user's new schedule must not be overwritten by previous claim's calculated next"
        );
    }

    #[test]
    fn delete_during_run_safely_skips_without_error() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("delete-race.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let now = Utc::now().to_rfc3339();

        let auto = Automation {
            id: "auto-del-race".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "Delete Race".into(),
            enabled: true,
            trigger: AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Test".into(),
                conversation_id: None,
            },
            requires_ai: true,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(now.clone()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        // 1. Claim run
        let claim = claim_run(&mut db, "auto-del-race", RunOrigin::Scheduled)
            .unwrap()
            .unwrap();

        // 2. User deletes automation while run is in flight
        db::delete_automation(&mut db, "auto-del-race").unwrap();
        assert!(db::get_automation(&db, "auto-del-race").is_err());

        // 3. In-flight request finishes; finalization must succeed safely without error
        let outcome = AutomationRunOutcome::success("Done");
        let final_result = finalize_run(&mut db, &claim, &outcome);
        assert!(
            final_result.is_ok(),
            "finalizing a deleted automation must not error or resurrect records"
        );

        // 4. Verify it was not resurrected
        assert!(db::get_automation(&db, "auto-del-race").is_err());
    }

    #[test]
    fn manual_run_now_preserves_scheduled_slot() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("manual-slot.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let planned_8am = "2026-10-27T08:00:00Z";

        let auto = Automation {
            id: "auto-manual-test".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "Daily Digest".into(),
            enabled: true,
            trigger: AutomationTrigger::Daily {
                time: "08:00".into(),
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Digest".into(),
                conversation_id: None,
            },
            requires_ai: true,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(planned_8am.into()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: planned_8am.into(),
            updated_at: planned_8am.into(),
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        // Manual "Run now" at 7:30 AM before the 8:00 AM slot:
        let claim = claim_run(&mut db, "auto-manual-test", RunOrigin::Manual)
            .unwrap()
            .unwrap();
        assert_eq!(claim.origin, RunOrigin::Manual);
        assert!(
            claim.scheduled_at.is_none(),
            "manual run is not scheduled_at"
        );

        // The next_run_at must STILL be 8:00 AM, not shifted to tomorrow!
        let auto_after_claim = db::get_automation(&db, "auto-manual-test").unwrap();
        assert_eq!(
            auto_after_claim.next_run_at.as_deref(),
            Some(planned_8am),
            "manual Run now must NOT advance a scheduled daily slot into the future"
        );

        // Finalize manual run
        let outcome = AutomationRunOutcome::success("Summary created");
        finalize_run(&mut db, &claim, &outcome).unwrap();

        let final_auto = db::get_automation(&db, "auto-manual-test").unwrap();
        assert_eq!(
            final_auto.next_run_at.as_deref(),
            Some(planned_8am),
            "scheduled 8 AM run remains intact after manual run completes"
        );
    }

    #[test]
    fn concurrent_claim_of_running_automation_is_rejected() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("concurrent-claim.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let now = Utc::now().to_rfc3339();

        let auto = Automation {
            id: "auto-excl-test".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "Exclusivity Test".into(),
            enabled: true,
            trigger: AutomationTrigger::Interval {
                interval_minutes: 15,
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Test".into(),
                conversation_id: None,
            },
            requires_ai: true,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(now.clone()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        // Worker 1 claims execution
        let claim1 = claim_run(&mut db, "auto-excl-test", RunOrigin::Scheduled).unwrap();
        assert!(claim1.is_some(), "worker 1 must succeed in claiming");

        // Worker 2 attempts to claim while worker 1 is running
        let claim2 = claim_run(&mut db, "auto-excl-test", RunOrigin::Scheduled).unwrap();
        assert!(
            claim2.is_none(),
            "worker 2 claim must be rejected when status is running"
        );
    }

    #[test]
    fn older_run_does_not_overwrite_newer_run_status() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("older-run.db")).unwrap();
        db::ensure_default_workspace(&db).unwrap();
        let now = Utc::now();
        let t10 = (now - chrono::Duration::seconds(50)).to_rfc3339();
        let t20 = (now - chrono::Duration::seconds(30)).to_rfc3339();
        let t30 = (now - chrono::Duration::seconds(10)).to_rfc3339();

        let auto = Automation {
            id: "auto-order-test".into(),
            workspace_id: db::DEFAULT_WORKSPACE_ID.into(),
            owner_tool_id: None,
            application_id: None,
            name: "Order Test".into(),
            enabled: true,
            trigger: AutomationTrigger::Interval {
                interval_minutes: 10,
                timezone: None,
            },
            action: AutomationAction::AiPrompt {
                prompt: "Test".into(),
                conversation_id: None,
            },
            requires_ai: true,
            provider_connection_id: None,
            missed_run_policy: MissedRunPolicy::RunOnce,
            next_run_at: Some(t10.clone()),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            waiting_approval: false,
            permission_ready: true,
            created_at: t10.clone(),
            updated_at: t10.clone(),
        };
        db::upsert_automation(&mut db, &auto).unwrap();

        // Run A started at t10
        let mut claim_a = claim_run(&mut db, "auto-order-test", RunOrigin::Scheduled)
            .unwrap()
            .unwrap();
        claim_a.started = t10;

        // Simulate Run B completing at t30 with status 'success'
        db::update_automation_schedule(
            &mut db,
            "auto-order-test",
            Some(&t30),
            Some(&t30),
            Some("success"),
            0,
            true,
        )
        .unwrap();

        // Now Run A (started at t10) finishes late with 'error'
        let outcome_a = AutomationRunOutcome::error("Timed out", "timeout");
        finalize_run(&mut db, &claim_a, &outcome_a).unwrap();

        // The automation's last_run_at and last_status should NOT be rolled back to Run A's status!
        let after = db::get_automation(&db, "auto-order-test").unwrap();
        assert_eq!(after.last_status.as_deref(), Some("success"));
        assert_eq!(after.last_run_at.as_deref(), Some(t30.as_str()));
    }
}
