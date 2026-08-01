use std::collections::HashSet;
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
}

/// Start the automation ticker on Tauri's async runtime (not a bare `tokio::spawn`
/// from `setup`, which has no reactor and aborts the process).
pub fn spawn_scheduler(app: AppHandle, handle: Arc<SchedulerHandle>) {
    tauri::async_runtime::spawn(async move {
        {
            if let Some(state) = app.try_state::<AppState>() {
                let mut db = state.db.lock();
                if let Err(e) = catch_up_missed(&mut db, &handle) {
                    tracing::warn!(error = %e, "automation missed-run catch-up failed");
                }
            }
        }

        let mut ticker = tokio::time::interval(Duration::from_secs(30));
        loop {
            ticker.tick().await;
            let Some(state) = app.try_state::<AppState>() else {
                continue;
            };
            let due = {
                let db = state.db.lock();
                db::list_due_automations(&db, &Utc::now().to_rfc3339()).unwrap_or_default()
            };
            for automation in due {
                if !handle.try_begin(&automation.id) {
                    continue;
                }
                let app_clone = app.clone();
                let handle_clone = handle.clone();
                let id = automation.id.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let parked_before = pending_approval_total(&app_clone);
                    if let Some(state) = app_clone.try_state::<AppState>() {
                        run_one(&state, &id);
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
    let now = Utc::now();
    let overdue = db::list_due_automations(db, &now.to_rfc3339())?;
    for automation in overdue {
        if automation.missed_run_policy == MissedRunPolicy::Skip {
            if let Some(next) = compute_next_after(&automation.trigger, now) {
                let _ = db::update_automation_schedule(
                    db,
                    &automation.id,
                    Some(&next.to_rfc3339()),
                    automation.last_run_at.as_deref(),
                    Some("skipped_missed"),
                    automation.consecutive_failures,
                    automation.enabled,
                );
            }
            continue;
        }
        if handle.try_begin(&automation.id) {
            let _ = run_one_db(db, &automation.id);
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
    let _ = run_one_db(&mut db, automation_id);
}

fn run_one_db(db: &mut Database, automation_id: &str) -> Result<(), db::DbError> {
    let automation = db::get_automation(db, automation_id)?;
    if !automation.enabled {
        return Ok(());
    }
    let started = Utc::now().to_rfc3339();
    let run_id = format!("run-{}", Uuid::new_v4());
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
    // A trigger that cannot produce a next run (corrupt time string) would leave
    // `next_run_at` NULL forever, silently orphaning the automation. Disable it
    // instead so the user can see and repair it.
    let next = compute_next_after(&automation.trigger, Utc::now()).map(|d| d.to_rfc3339());
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
        Some(&completed),
        Some(status),
        failures,
        schedulable && enabled && automation.enabled,
    )?;
    Ok(())
}

pub fn run_now(
    state: &AppState,
    automation_id: &str,
    handle: &SchedulerHandle,
) -> Result<String, String> {
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
