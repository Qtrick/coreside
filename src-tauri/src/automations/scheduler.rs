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
    // Claim the schedule slot BEFORE side effects so a crash mid-execute cannot
    // leave next_run_at due and re-fire the same mutation indefinitely.
    let next = compute_next_after(&automation.trigger, started_dt).map(|d| d.to_rfc3339());
    let schedulable = next.is_some();
    if !schedulable {
        tracing::warn!(
            automation_id,
            "automation trigger has no next run; disabling"
        );
    }
    let scheduled_at = automation.next_run_at.clone();
    db::update_automation_schedule(
        db,
        automation_id,
        next.as_deref(),
        Some(&started),
        Some("running"),
        automation.consecutive_failures,
        schedulable && automation.enabled,
    )?;
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
    db::insert_automation_run(
        db,
        &claim.run_id,
        &claim.automation.id,
        claim.scheduled_at.as_deref(),
        &claim.started,
        Some(&completed),
        &outcome.status,
        Some(outcome.summary.as_str()),
        outcome.error_category.as_deref(),
    )?;
    db::update_automation_schedule(
        db,
        &claim.automation.id,
        claim.next.as_deref(),
        Some(&completed),
        Some(&outcome.status),
        failures,
        claim.schedulable && keep_enabled && claim.automation.enabled,
    )?;
    let _ = db.conn().execute(
        "DELETE FROM automation_runs WHERE automation_id = ?1 AND id NOT IN (
            SELECT id FROM (
                SELECT id FROM automation_runs WHERE automation_id = ?1
                ORDER BY created_at DESC LIMIT 100
            )
         )",
        rusqlite::params![claim.automation.id],
    );
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
        return AutomationRunOutcome::error(
            "AI prompt actions must set requiresAi",
            "validation",
        );
    }
    // DB snapshot only under the mutex; keyring + provider I/O run unlocked.
    let snapshot = {
        let db = state.db.lock();
        snapshot_ai_prepare(&db, &claim.automation, prompt)
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
}
