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
                    if pending_approval_total(&app_clone) != parked_before {
                        use tauri::Emitter;
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
            if let Err(e) = run_one_db(db, &automation.id) {
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
    if let Err(e) = run_one_releasing_lock_for_ai(state, automation_id) {
        tracing::error!(automation_id = %automation_id, error = %e, "automation execution failed");
    }
}

struct ClaimedRun {
    automation: Automation,
    run_id: String,
    started: String,
    next: Option<String>,
    schedulable: bool,
    scheduled_at: Option<String>,
}

fn claim_run(db: &mut Database, automation_id: &str) -> Result<Option<ClaimedRun>, db::DbError> {
    let automation = db::get_automation(db, automation_id)?;
    if !automation.enabled {
        return Ok(None);
    }
    let started_dt = Utc::now();
    let started = started_dt.to_rfc3339();
    let run_id = format!("run-{}", Uuid::new_v4());
    // Claim the schedule slot and persist the running claim in automation_runs
    // within a single atomic transaction before side effects occur.
    let next = compute_next_after(&automation.trigger, started_dt).map(|d| d.to_rfc3339());
    let schedulable = next.is_some();
    if !schedulable {
        tracing::warn!(
            automation_id,
            "automation trigger has no next run; disabling"
        );
    }
    let scheduled_at = automation.next_run_at.clone();

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
    }))
}

fn finalize_run(
    db: &mut Database,
    claim: &ClaimedRun,
    outcome: &AutomationRunOutcome,
) -> Result<(), db::DbError> {
    let failures = if outcome.counts_as_failure() {
        claim.automation.consecutive_failures + 1
    } else if outcome.status == "success" {
        0_i64
    } else {
        claim.automation.consecutive_failures
    };
    let keep_enabled = if outcome.counts_as_failure() {
        failures < MAX_CONSECUTIVE_FAILURES
    } else {
        true
    };
    let completed = Utc::now().to_rfc3339();

    // Atomic transaction for updating the run record and updating the automation row:
    db.with_transaction(|conn| {
        db::update_automation_run_conn(
            conn,
            &claim.run_id,
            Some(&completed),
            &outcome.status,
            Some(outcome.summary.as_str()),
            outcome.error_category.as_deref(),
        )?;
        db::update_automation_schedule_conn(
            conn,
            &claim.automation.id,
            claim.next.as_deref(),
            Some(&completed),
            Some(&outcome.status),
            failures,
            claim.schedulable && keep_enabled && claim.automation.enabled,
        )?;
        let _ = conn.execute(
            "DELETE FROM automation_runs WHERE automation_id = ?1 AND id NOT IN (
                SELECT id FROM (
                    SELECT id FROM automation_runs WHERE automation_id = ?1
                    ORDER BY created_at DESC LIMIT 100
                )
             )",
            rusqlite::params![claim.automation.id],
        );
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
) -> Result<AutomationRunOutcome, db::DbError> {
    let claim = {
        let mut db = state.db.lock();
        claim_run(&mut db, automation_id)?
    };
    let Some(claim) = claim else {
        return Ok(AutomationRunOutcome::skipped("Automation is disabled"));
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
fn run_one_db(db: &mut Database, automation_id: &str) -> Result<AutomationRunOutcome, db::DbError> {
    let Some(claim) = claim_run(db, automation_id)? else {
        return Ok(AutomationRunOutcome::skipped("Automation is disabled"));
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
    let result = run_one_releasing_lock_for_ai(state, automation_id);
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

        let outcome = run_one_db(&mut db, "auto-park").unwrap();
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

        let outcome = run_one_db(&mut db, "auto-fail").unwrap();
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

        let claim = claim_run(&mut db, "auto-claim-test")
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
}
