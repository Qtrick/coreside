//! Global maintenance-mode boundary for restore / migration / profile swaps.

use chrono::Utc;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::app_paths::AppPaths;
use crate::commands::CommandError;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaintenanceStage {
    Inactive,
    Preparing,
    CancellingWork,
    Flushing,
    SafetyBackup,
    Staging,
    Validating,
    AwaitingConfirmation,
    Swapping,
    Reopening,
    Rehydrating,
    RollingBack,
    Completed,
    Failed,
    RecoveryRequired,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceStatus {
    pub active: bool,
    pub operation_id: Option<String>,
    pub operation_type: Option<String>,
    pub stage: MaintenanceStage,
    pub started_at: Option<String>,
    pub irreversible: bool,
    pub safe_error: Option<String>,
}

#[derive(Debug, Clone)]
struct ActiveOperation {
    id: String,
    operation_type: String,
    stage: MaintenanceStage,
    started_at: String,
    irreversible: bool,
    safe_error: Option<String>,
}

/// Exactly one maintenance operation may be active.
#[derive(Default)]
pub struct MaintenanceMode {
    active: Option<ActiveOperation>,
}

impl MaintenanceMode {
    pub fn status(&self) -> MaintenanceStatus {
        match &self.active {
            None => MaintenanceStatus {
                active: false,
                operation_id: None,
                operation_type: None,
                stage: MaintenanceStage::Inactive,
                started_at: None,
                irreversible: false,
                safe_error: None,
            },
            Some(op) => MaintenanceStatus {
                active: true,
                operation_id: Some(op.id.clone()),
                operation_type: Some(op.operation_type.clone()),
                stage: op.stage,
                started_at: Some(op.started_at.clone()),
                irreversible: op.irreversible,
                safe_error: op.safe_error.clone(),
            },
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn begin(&mut self, operation_type: &str) -> Result<String, CommandError> {
        if self.active.is_some() {
            return Err(CommandError::new(
                "maintenance_in_progress",
                "Coreside is already running a maintenance operation.",
            ));
        }
        let id = Uuid::new_v4().to_string();
        self.active = Some(ActiveOperation {
            id: id.clone(),
            operation_type: operation_type.to_string(),
            stage: MaintenanceStage::Preparing,
            started_at: Utc::now().to_rfc3339(),
            irreversible: false,
            safe_error: None,
        });
        Ok(id)
    }

    pub fn set_stage(
        &mut self,
        operation_id: &str,
        stage: MaintenanceStage,
    ) -> Result<(), CommandError> {
        let op = self
            .active
            .as_mut()
            .ok_or_else(|| CommandError::new("invalid", "No maintenance operation is active."))?;
        if op.id != operation_id {
            return Err(CommandError::new(
                "invalid",
                "Maintenance operation id mismatch.",
            ));
        }
        if matches!(
            stage,
            MaintenanceStage::Swapping
                | MaintenanceStage::Reopening
                | MaintenanceStage::Rehydrating
        ) {
            op.irreversible = true;
        }
        op.stage = stage;
        Ok(())
    }

    pub fn fail(
        &mut self,
        operation_id: &str,
        safe_error: impl Into<String>,
    ) -> Result<(), CommandError> {
        let op = self
            .active
            .as_mut()
            .ok_or_else(|| CommandError::new("invalid", "No maintenance operation is active."))?;
        if op.id != operation_id {
            return Err(CommandError::new(
                "invalid",
                "Maintenance operation id mismatch.",
            ));
        }
        op.stage = MaintenanceStage::Failed;
        op.safe_error = Some(safe_error.into());
        Ok(())
    }

    pub fn clear(&mut self, operation_id: &str) -> Result<(), CommandError> {
        match &self.active {
            Some(op) if op.id == operation_id => {
                self.active = None;
                Ok(())
            }
            Some(_) => Err(CommandError::new(
                "invalid",
                "Maintenance operation id mismatch.",
            )),
            None => Ok(()),
        }
    }

    pub fn require_inactive(&self) -> Result<(), CommandError> {
        if self.is_active() {
            Err(CommandError::new(
                "maintenance_in_progress",
                "Coreside is updating your profile. Try again when maintenance finishes.",
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeferredMaintenanceOutcome {
    Completed {
        journal_compacted: bool,
        cache_cleaned: bool,
        attachments_swept: bool,
    },
    SkippedProfileNotReady,
    SkippedQuiescence,
    SkippedMaintenanceActive,
    SkippedRecoveryRequired,
    SkippedAlreadyRunning,
    SkippedGenerationChanged,
    Cancelled,
}

/// Execute non-critical startup maintenance (journal compaction, cache cleaning,
/// attachment sweep) with strict authority gates and bounded locks.
///
/// Designed to execute on a blocking thread pool (`spawn_blocking`) after the initial
/// shell hydration delay so Tokio worker threads and native event loops are not blocked.
pub fn execute_deferred_startup_maintenance(
    state: &AppState,
    paths: &AppPaths,
    cancel: &CancellationToken,
) -> DeferredMaintenanceOutcome {
    if cancel.is_cancelled() {
        return DeferredMaintenanceOutcome::Cancelled;
    }
    if !state.profile_ready() {
        return DeferredMaintenanceOutcome::SkippedProfileNotReady;
    }
    if state.maintenance_status().active {
        return DeferredMaintenanceOutcome::SkippedMaintenanceActive;
    }
    if state.quiescence.is_paused() {
        return DeferredMaintenanceOutcome::SkippedQuiescence;
    }
    if !state.try_begin_deferred_maintenance() {
        return DeferredMaintenanceOutcome::SkippedAlreadyRunning;
    }
    struct WorkerGuard<'a>(&'a AppState);
    impl Drop for WorkerGuard<'_> {
        fn drop(&mut self) {
            self.0.end_deferred_maintenance();
        }
    }
    let _guard = WorkerGuard(state);

    let initial_gen = state.quiescence.generation();

    match crate::maintenance_journal::load_journal(paths) {
        Ok(Some(journal)) => {
            let action = crate::maintenance_journal::classify_unfinished_journal(&journal);
            if action != crate::maintenance_journal::JournalStartupAction::None {
                return DeferredMaintenanceOutcome::SkippedRecoveryRequired;
            }
        }
        Err(_) => {
            return DeferredMaintenanceOutcome::SkippedRecoveryRequired;
        }
        Ok(None) => {}
    }

    if cancel.is_cancelled() {
        return DeferredMaintenanceOutcome::Cancelled;
    }
    if state.quiescence.generation() != initial_gen {
        return DeferredMaintenanceOutcome::SkippedGenerationChanged;
    }

    // 1. Compact turn journal with strictly scoped database mutex lock
    let journal_compacted = {
        let db = state.db.lock();
        if cancel.is_cancelled() {
            return DeferredMaintenanceOutcome::Cancelled;
        }
        if state.quiescence.generation() != initial_gen {
            return DeferredMaintenanceOutcome::SkippedGenerationChanged;
        }
        match crate::runtime_v2::compact_turn_journal(&db, 30, 14) {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(error = %e, "failed to compact turn journal");
                false
            }
        }
    };

    if cancel.is_cancelled() {
        return DeferredMaintenanceOutcome::Cancelled;
    }
    if state.quiescence.generation() != initial_gen {
        return DeferredMaintenanceOutcome::SkippedGenerationChanged;
    }
    if state.maintenance_status().active {
        return DeferredMaintenanceOutcome::SkippedMaintenanceActive;
    }
    if state.quiescence.is_paused() {
        return DeferredMaintenanceOutcome::SkippedQuiescence;
    }

    // 2. Cache cleaner — directory walks and removals
    let cache_cleaned = {
        let report = crate::cache_cleaner::clean_stale_caches(paths);
        let total_removed = report.stale_restore_staging_removed
            + report.stale_attachment_staging_removed
            + report.orphaned_temp_files_removed;
        if total_removed > 0 || report.crawler_bytes_freed > 0 || !report.errors.is_empty() {
            tracing::info!(
                restore_staging = report.stale_restore_staging_removed,
                attachment_staging = report.stale_attachment_staging_removed,
                orphaned_temps = report.orphaned_temp_files_removed,
                crawler_freed = %crate::cache_cleaner::format_bytes(report.crawler_bytes_freed),
                errors = report.errors.len(),
                "background cache cleanup completed"
            );
        }
        report.errors.is_empty()
    };

    if cancel.is_cancelled() {
        return DeferredMaintenanceOutcome::Cancelled;
    }
    if state.quiescence.generation() != initial_gen {
        return DeferredMaintenanceOutcome::SkippedGenerationChanged;
    }
    if state.maintenance_status().active {
        return DeferredMaintenanceOutcome::SkippedMaintenanceActive;
    }
    if state.quiescence.is_paused() {
        return DeferredMaintenanceOutcome::SkippedQuiescence;
    }

    // 3. Attachment reconciliation and sweep
    let attachments_swept = match crate::commands::reconcile_and_sweep_attachments(state) {
        Ok(report) => {
            if report.expired > 0 || report.promoted_orphans > 0 || report.missing_durable > 0 {
                tracing::info!(
                    expired = report.expired,
                    promoted = report.promoted_orphans,
                    missing = report.missing_durable,
                    "attachment sweep completed"
                );
            }
            true
        }
        Err(err) => {
            tracing::warn!(error = %err.message, "attachment sweep failed");
            false
        }
    };

    DeferredMaintenanceOutcome::Completed {
        journal_compacted,
        cache_cleaned,
        attachments_swept,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{BootstrapStatus, Database};
    use tempfile::TempDir;

    fn test_fixture() -> (TempDir, AppPaths, AppState) {
        let dir = TempDir::new().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();
        let db = Database::open_path(&paths.database).unwrap();
        let state = AppState::new_for_test(db);
        (dir, paths, state)
    }

    #[test]
    fn only_one_operation_at_a_time() {
        let mut m = MaintenanceMode::default();
        let id = m.begin("restore").unwrap();
        assert!(m.begin("backup").is_err());
        m.set_stage(&id, MaintenanceStage::Swapping).unwrap();
        assert!(m.status().irreversible);
        m.clear(&id).unwrap();
        assert!(!m.is_active());
    }

    #[test]
    fn deferred_maintenance_skips_when_cancelled() {
        let (_dir, paths, state) = test_fixture();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let outcome = execute_deferred_startup_maintenance(&state, &paths, &cancel);
        assert_eq!(outcome, DeferredMaintenanceOutcome::Cancelled);
    }

    #[test]
    fn deferred_maintenance_skips_when_profile_not_ready() {
        let (_dir, paths, state) = test_fixture();
        *state.bootstrap.lock() = BootstrapStatus::recovery("test", "test", None, false);
        let cancel = CancellationToken::new();
        let outcome = execute_deferred_startup_maintenance(&state, &paths, &cancel);
        assert_eq!(outcome, DeferredMaintenanceOutcome::SkippedProfileNotReady);
    }

    #[test]
    fn deferred_maintenance_skips_when_quiescence_paused() {
        let (_dir, paths, state) = test_fixture();
        let _token = state.quiescence.pause();
        let cancel = CancellationToken::new();
        let outcome = execute_deferred_startup_maintenance(&state, &paths, &cancel);
        assert_eq!(outcome, DeferredMaintenanceOutcome::SkippedQuiescence);
    }

    #[test]
    fn deferred_maintenance_skips_when_maintenance_active() {
        let (_dir, paths, state) = test_fixture();
        let _id = state.begin_maintenance("test_backup").unwrap();
        let cancel = CancellationToken::new();
        let outcome = execute_deferred_startup_maintenance(&state, &paths, &cancel);
        assert_eq!(
            outcome,
            DeferredMaintenanceOutcome::SkippedMaintenanceActive
        );
    }

    #[test]
    fn deferred_maintenance_skips_when_journal_requires_recovery() {
        let (_dir, paths, state) = test_fixture();
        let mut journal = crate::maintenance_journal::MaintenanceJournal::new("restore");
        journal.touch_stage(MaintenanceStage::Swapping);
        crate::maintenance_journal::persist_journal(&paths, &journal).unwrap();

        let cancel = CancellationToken::new();
        let outcome = execute_deferred_startup_maintenance(&state, &paths, &cancel);
        assert_eq!(outcome, DeferredMaintenanceOutcome::SkippedRecoveryRequired);
    }

    #[test]
    fn deferred_maintenance_completes_when_healthy() {
        let (_dir, paths, state) = test_fixture();
        let cancel = CancellationToken::new();
        let outcome = execute_deferred_startup_maintenance(&state, &paths, &cancel);
        assert!(matches!(
            outcome,
            DeferredMaintenanceOutcome::Completed {
                journal_compacted: true,
                cache_cleaned: true,
                ..
            }
        ));
    }

    #[test]
    fn deferred_maintenance_cancelled_via_state_cancel_all() {
        let (_dir, paths, state) = test_fixture();
        let cancel = CancellationToken::new();
        state.register_maintenance_cancel(cancel.clone());

        let state = std::sync::Arc::new(state);
        let paths = std::sync::Arc::new(paths);
        let state_clone = state.clone();
        let paths_clone = paths.clone();
        let cancel_clone = cancel.clone();

        let db_guard = state.db.lock();
        let handle = std::thread::spawn(move || {
            execute_deferred_startup_maintenance(&state_clone, &paths_clone, &cancel_clone)
        });
        std::thread::sleep(std::time::Duration::from_millis(50));

        state.cancel_all();
        drop(db_guard);

        let outcome = handle.join().unwrap();
        assert_eq!(outcome, DeferredMaintenanceOutcome::Cancelled);
    }

    #[test]
    fn deferred_maintenance_cancelled_via_replace_profile_database() {
        let (dir, paths, state) = test_fixture();
        let cancel = CancellationToken::new();
        state.register_maintenance_cancel(cancel.clone());

        let new_db = Database::open_path(&dir.path().join("replaced.db")).unwrap();
        state.replace_profile_database(new_db);

        assert!(
            cancel.is_cancelled(),
            "replace_profile_database must cancel in-flight maintenance request"
        );
        let outcome = execute_deferred_startup_maintenance(&state, &paths, &cancel);
        assert_eq!(outcome, DeferredMaintenanceOutcome::Cancelled);
    }

    #[test]
    fn deferred_maintenance_skips_when_generation_changes_after_authorization() {
        let (_dir, paths, state) = test_fixture();
        let cancel = CancellationToken::new();

        let state = std::sync::Arc::new(state);
        let paths = std::sync::Arc::new(paths);
        let state_clone = state.clone();
        let paths_clone = paths.clone();
        let cancel_clone = cancel.clone();

        // Hold db lock to pause execute_deferred_startup_maintenance right after authorization checks
        let db_guard = state.db.lock();

        let handle = std::thread::spawn(move || {
            execute_deferred_startup_maintenance(&state_clone, &paths_clone, &cancel_clone)
        });
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Profile generation bumped mid-execution (after authorization)
        state.quiescence.bump_generation();
        drop(db_guard);

        let outcome = handle.join().unwrap();
        assert!(
            matches!(
                outcome,
                DeferredMaintenanceOutcome::SkippedGenerationChanged | DeferredMaintenanceOutcome::Cancelled
            ),
            "expected SkippedGenerationChanged or Cancelled when generation bumps after authorization, got {outcome:?}"
        );
    }

    #[test]
    fn deferred_maintenance_mutual_exclusion_second_call_skipped() {
        let (_dir, paths, state) = test_fixture();
        let cancel1 = CancellationToken::new();
        let cancel2 = CancellationToken::new();

        let state = std::sync::Arc::new(state);
        let paths = std::sync::Arc::new(paths);
        let state_clone = state.clone();
        let paths_clone = paths.clone();
        let cancel1_clone = cancel1.clone();

        // Hold db lock so first call remains running in background
        let db_guard = state.db.lock();

        let handle = std::thread::spawn(move || {
            execute_deferred_startup_maintenance(&state_clone, &paths_clone, &cancel1_clone)
        });
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Second concurrent invocation must not overlap and return SkippedAlreadyRunning
        let outcome2 = execute_deferred_startup_maintenance(&state, &paths, &cancel2);

        drop(db_guard);
        let outcome1 = handle.join().unwrap();

        assert_eq!(
            outcome2,
            DeferredMaintenanceOutcome::SkippedAlreadyRunning,
            "concurrent call to execute_deferred_startup_maintenance must return SkippedAlreadyRunning"
        );
        assert!(matches!(
            outcome1,
            DeferredMaintenanceOutcome::Completed { .. }
        ));
    }

    #[test]
    fn deferred_maintenance_cancelled_via_begin_maintenance() {
        let (_dir, paths, state) = test_fixture();
        let cancel = CancellationToken::new();
        state.register_maintenance_cancel(cancel.clone());

        let maint_id = state.begin_maintenance("backup").expect("begin backup");
        assert!(
            cancel.is_cancelled(),
            "begin_maintenance must cancel in-flight maintenance token"
        );
        let outcome = execute_deferred_startup_maintenance(&state, &paths, &cancel);
        assert_eq!(outcome, DeferredMaintenanceOutcome::Cancelled);

        state.clear_maintenance(&maint_id).expect("clear backup");
    }

    #[test]
    fn try_begin_deferred_maintenance_respects_quiescence_pause_and_release() {
        let (_dir, _paths, state) = test_fixture();

        // When quiescence is paused, cannot begin
        let pause_token = state.quiescence.pause();
        assert!(!state.try_begin_deferred_maintenance());

        // Once resumed with matching token, can begin
        assert!(state.quiescence.resume(pause_token));
        assert!(state.try_begin_deferred_maintenance());

        // Cannot begin again while already active
        assert!(!state.try_begin_deferred_maintenance());

        // Releasing allows next worker to begin
        state.end_deferred_maintenance();
        assert!(state.try_begin_deferred_maintenance());
        state.end_deferred_maintenance();
    }
}
