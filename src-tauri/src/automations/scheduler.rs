use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use parking_lot::Mutex;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::executor::execute_automation;
use super::models::{compute_next_after, MissedRunPolicy};
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
    let mut db = state.db.lock();
    if let Err(e) = run_one_db(&mut db, automation_id) {
        tracing::error!(automation_id = %automation_id, error = %e, "automation execution failed");
    }
}

fn run_one_db(db: &mut Database, automation_id: &str) -> Result<(), db::DbError> {
    let automation = db::get_automation(db, automation_id)?;
    if !automation.enabled {
        return Ok(());
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
    db::update_automation_schedule(
        db,
        automation_id,
        next.as_deref(),
        Some(&started),
        Some("running"),
        automation.consecutive_failures,
        schedulable && automation.enabled,
    )?;

    let result = execute_automation(db, &automation);
    let (status, summary, err_cat, failures, enabled) = match result {
        Ok(summary) => ("success", Some(summary), None, 0_i64, true),
        Err(e) => {
            let failures = automation.consecutive_failures + 1;
            let enabled = failures < MAX_CONSECUTIVE_FAILURES;
            (
                "error",
                Some(e.to_string()),
                Some("execution_error".to_string()),
                failures,
                enabled,
            )
        }
    };
    let completed = Utc::now().to_rfc3339();
    db::insert_automation_run(
        db,
        &run_id,
        automation_id,
        automation.next_run_at.as_deref(),
        &started,
        Some(&completed),
        status,
        summary.as_deref(),
        err_cat.as_deref(),
    )?;
    db::update_automation_schedule(
        db,
        automation_id,
        next.as_deref(),
        Some(&completed),
        Some(status),
        failures,
        schedulable && enabled && automation.enabled,
    )?;
    // Bound automation_runs growth: keep the newest 100 rows per automation.
    // Nested SELECT materializes the keep-set so SQLite allows same-table DELETE.
    let _ = db.conn().execute(
        "DELETE FROM automation_runs WHERE automation_id = ?1 AND id NOT IN (
            SELECT id FROM (
                SELECT id FROM automation_runs WHERE automation_id = ?1
                ORDER BY created_at DESC LIMIT 100
            )
         )",
        rusqlite::params![automation_id],
    );
    Ok(())
}

pub fn run_now(
    state: &AppState,
    automation_id: &str,
    handle: &SchedulerHandle,
) -> Result<String, String> {
    if !state.profile_ready() {
        return Err("Automations are unavailable until Recovery finishes.".into());
    }
    if !handle.try_begin(automation_id) {
        return Err("Automation is already running".into());
    }
    let mut db = state.db.lock();
    let result = run_one_db(&mut db, automation_id);
    handle.end(automation_id);
    result
        .map(|_| "Automation run completed".into())
        .map_err(|e| e.to_string())
}
