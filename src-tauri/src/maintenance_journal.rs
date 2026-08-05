//! Durable maintenance transaction journal (outside the active profile SQLite DB).

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_paths::AppPaths;
use crate::commands::CommandError;
use crate::maintenance::MaintenanceStage;
use crate::security::sanitize_error;

pub const JOURNAL_SCHEMA_VERSION: u32 = 1;
const JOURNAL_FILE_NAME: &str = "maintenance-journal.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceJournal {
    pub schema_version: u32,
    pub operation_id: String,
    pub operation_type: String,
    pub stage: String,
    pub irreversible: bool,
    pub started_at: String,
    pub updated_at: String,
    pub profile_generation: Option<u64>,
    pub source_profile: Option<String>,
    pub staged_profile: Option<String>,
    pub quarantine_profile: Option<String>,
    pub safety_backup_id: Option<String>,
    pub safe_error_code: Option<String>,
    pub rollback_state: Option<String>,
    pub rehydration_state: Option<String>,
    pub completion_state: Option<String>,
}

impl MaintenanceJournal {
    pub fn new(operation_type: &str) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            schema_version: JOURNAL_SCHEMA_VERSION,
            operation_id: Uuid::new_v4().to_string(),
            operation_type: operation_type.to_string(),
            stage: stage_name(MaintenanceStage::Preparing).to_string(),
            irreversible: false,
            started_at: now.clone(),
            updated_at: now,
            profile_generation: None,
            source_profile: None,
            staged_profile: None,
            quarantine_profile: None,
            safety_backup_id: None,
            safe_error_code: None,
            rollback_state: None,
            rehydration_state: None,
            completion_state: None,
        }
    }

    pub fn touch_stage(&mut self, stage: MaintenanceStage) {
        if matches!(
            stage,
            MaintenanceStage::Swapping
                | MaintenanceStage::Reopening
                | MaintenanceStage::Rehydrating
        ) {
            self.irreversible = true;
        }
        self.stage = stage_name(stage).to_string();
        self.updated_at = Utc::now().to_rfc3339();
    }
}

fn stage_name(stage: MaintenanceStage) -> &'static str {
    match stage {
        MaintenanceStage::Inactive => "inactive",
        MaintenanceStage::Preparing => "preparing",
        MaintenanceStage::CancellingWork => "cancelling-work",
        MaintenanceStage::Flushing => "flushing",
        MaintenanceStage::SafetyBackup => "safety-backup",
        MaintenanceStage::Staging => "staging",
        MaintenanceStage::Validating => "validating",
        MaintenanceStage::AwaitingConfirmation => "awaiting-confirmation",
        MaintenanceStage::Swapping => "swapping",
        MaintenanceStage::Reopening => "reopening",
        MaintenanceStage::Rehydrating => "rehydrating",
        MaintenanceStage::RollingBack => "rolling-back",
        MaintenanceStage::Completed => "completed",
        MaintenanceStage::Failed => "failed",
        MaintenanceStage::RecoveryRequired => "recovery-required",
    }
}

fn journal_path(paths: &AppPaths) -> PathBuf {
    // Lives beside the profile tree roots, not inside coreside.db.
    paths.recovery.join(JOURNAL_FILE_NAME)
}

/// Lexically normalize an absolute path (`..` / `.`) without touching the filesystem.
/// Returns `None` if the path is relative or `..` escapes the filesystem root.
fn normalize_lexically(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(comp),
            Component::CurDir => {}
            Component::ParentDir => {
                // Refuse escape above the root directory.
                if !out.pop() {
                    return None;
                }
            }
            Component::Normal(c) => out.push(c),
        }
    }
    Some(out)
}

fn path_is_under_managed_root(candidate: &Path, root: &Path) -> bool {
    let Ok(r) = root.canonicalize() else {
        return false;
    };
    if let Ok(c) = candidate.canonicalize() {
        return c.starts_with(&r);
    }
    // Non-existent paths: fail closed on relative refs and `..` traversal.
    // Plain `Path::starts_with` is unsafe — `/root/../../etc` still starts_with `/root`.
    let Some(normalized) = normalize_lexically(candidate) else {
        return false;
    };
    // Resolve the longest existing prefix (handles macOS /var → /private/var symlinks),
    // then re-append missing leaf components that were already lexically normalized.
    let mut probe = normalized;
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    while !probe.exists() {
        match probe.file_name() {
            Some(name) => {
                missing.push(name.to_os_string());
                if !probe.pop() {
                    return false;
                }
            }
            None => return false,
        }
    }
    let Ok(mut resolved) = probe.canonicalize() else {
        return false;
    };
    for part in missing.into_iter().rev() {
        resolved.push(part);
    }
    resolved.starts_with(&r)
}

/// Validate journal path references stay inside AppPaths-managed roots.
pub fn validate_journal_paths(
    journal: &MaintenanceJournal,
    paths: &AppPaths,
) -> Result<(), CommandError> {
    for (label, value) in [
        ("source_profile", journal.source_profile.as_deref()),
        ("staged_profile", journal.staged_profile.as_deref()),
        ("quarantine_profile", journal.quarantine_profile.as_deref()),
    ] {
        let Some(raw) = value else { continue };
        let p = PathBuf::from(raw);
        // Relative paths are never trusted journal references.
        if !p.is_absolute()
            || !(path_is_under_managed_root(&p, &paths.product_root)
                || path_is_under_managed_root(&p, &paths.restore_staging)
                || path_is_under_managed_root(&p, &paths.quarantine)
                || path_is_under_managed_root(&p, &paths.backups)
                || path_is_under_managed_root(&p, &paths.recovery))
        {
            return Err(CommandError::new(
                "recovery_required",
                format!("Maintenance journal {label} is outside managed AppPaths roots"),
            ));
        }
    }
    Ok(())
}

pub fn load_journal(paths: &AppPaths) -> Result<Option<MaintenanceJournal>, CommandError> {
    let path = journal_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path)
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    let journal: MaintenanceJournal = serde_json::from_slice(&bytes).map_err(|e| {
        CommandError::new(
            "recovery_required",
            format!(
                "Unreadable maintenance journal: {}",
                sanitize_error(&e.to_string(), None)
            ),
        )
    })?;
    if journal.schema_version == 0 || journal.schema_version > JOURNAL_SCHEMA_VERSION {
        return Err(CommandError::new(
            "recovery_required",
            "Unsupported maintenance journal schema version",
        ));
    }
    validate_journal_paths(&journal, paths)?;
    Ok(Some(journal))
}

pub fn persist_journal(paths: &AppPaths, journal: &MaintenanceJournal) -> Result<(), CommandError> {
    paths
        .ensure_dirs()
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    validate_journal_paths(journal, paths)?;
    let path = journal_path(paths);
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(journal)
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
        f.write_all(&json)
            .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &path)
        .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    Ok(())
}

pub fn clear_journal(paths: &AppPaths) -> Result<(), CommandError> {
    let path = journal_path(paths);
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| CommandError::new("storage", sanitize_error(&e.to_string(), None)))?;
    }
    let tmp = path.with_extension("json.tmp");
    let _ = fs::remove_file(tmp);
    Ok(())
}

/// Classify an unfinished journal without deleting profile trees.
///
/// Startup applies these decisions:
/// - `None` / completed → clear stale completed journal
/// - `RollBack` + `!irreversible` → clear journal (staging trees preserved for inspection)
/// - `EnterRecovery` / irreversible rollback / unreadable → `enter_safe_startup` + skip scheduler
///
/// `Resume` is reserved and not returned by classify today.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JournalStartupAction {
    None,
    /// Reserved — not emitted by classify today.
    Resume,
    RollBack,
    EnterRecovery,
}

pub fn classify_unfinished_journal(journal: &MaintenanceJournal) -> JournalStartupAction {
    match journal.stage.as_str() {
        "completed" | "inactive" => JournalStartupAction::None,
        "failed" | "recovery-required" => JournalStartupAction::EnterRecovery,
        // Irreversible mid-swap: never auto-resume from detect-only startup.
        "swapping" | "reopening" | "rehydrating" => JournalStartupAction::EnterRecovery,
        "rolling-back" => JournalStartupAction::RollBack,
        "preparing"
        | "cancelling-work"
        | "flushing"
        | "safety-backup"
        | "staging"
        | "validating"
        | "awaiting-confirmation" => JournalStartupAction::RollBack,
        _ => JournalStartupAction::EnterRecovery,
    }
}

/// Deterministic outcome of applying a startup classification (no profile-tree deletes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalStartupOutcome {
    /// Stale completed/inactive or safe pre-swap journal removed.
    Cleared,
    /// Ordinary services must not start; caller enters Recovery.
    EnterRecovery { reason: &'static str },
    /// Classification was None for a non-stale stage (should not occur).
    NoOp,
}

/// Apply startup classification with side effects limited to journal clear + outcome.
/// Never deletes profile trees. Clear failures surface as `Err` so callers enter Recovery
/// instead of logging-only.
pub fn apply_startup_decision(
    paths: &AppPaths,
    journal: &MaintenanceJournal,
    action: JournalStartupAction,
) -> Result<JournalStartupOutcome, CommandError> {
    match action {
        JournalStartupAction::None => {
            if journal.stage == "completed" || journal.stage == "inactive" {
                clear_journal(paths)?;
                Ok(JournalStartupOutcome::Cleared)
            } else {
                Ok(JournalStartupOutcome::NoOp)
            }
        }
        JournalStartupAction::RollBack if !journal.irreversible => {
            // Staged trees are left for Recovery inspection — never auto-deleted.
            clear_journal(paths)?;
            Ok(JournalStartupOutcome::Cleared)
        }
        JournalStartupAction::EnterRecovery
        | JournalStartupAction::RollBack
        | JournalStartupAction::Resume => Ok(JournalStartupOutcome::EnterRecovery {
            reason: "unfinished maintenance journal",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn persist_load_clear_round_trip() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();
        let mut j = MaintenanceJournal::new("restore");
        j.touch_stage(MaintenanceStage::Staging);
        persist_journal(&paths, &j).unwrap();
        let loaded = load_journal(&paths).unwrap().expect("journal");
        assert_eq!(loaded.operation_id, j.operation_id);
        assert_eq!(loaded.stage, "staging");
        clear_journal(&paths).unwrap();
        assert!(load_journal(&paths).unwrap().is_none());
    }

    #[test]
    fn rejects_path_outside_managed_roots() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();
        let mut j = MaintenanceJournal::new("restore");
        j.source_profile = Some("/tmp/evil-profile".into());
        assert!(validate_journal_paths(&j, &paths).is_err());
    }

    #[test]
    fn rejects_relative_and_dotdot_traversal_paths() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();
        let mut j = MaintenanceJournal::new("restore");
        j.source_profile = Some("relative/profile".into());
        assert!(validate_journal_paths(&j, &paths).is_err());

        let mut j2 = MaintenanceJournal::new("restore");
        j2.staged_profile = Some(
            paths
                .product_root
                .join("..")
                .join("..")
                .join("etc")
                .join("passwd")
                .to_string_lossy()
                .into_owned(),
        );
        assert!(validate_journal_paths(&j2, &paths).is_err());
    }

    #[test]
    fn accepts_nonexistent_leaf_under_product_root() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();
        let mut j = MaintenanceJournal::new("restore");
        j.source_profile = Some(
            paths
                .product_root
                .join("profiles")
                .join("staged-new")
                .to_string_lossy()
                .into_owned(),
        );
        assert!(validate_journal_paths(&j, &paths).is_ok());
    }

    #[test]
    fn classify_mid_swap_enters_recovery_even_without_irreversible_flag() {
        let mut j = MaintenanceJournal::new("restore");
        j.stage = "swapping".into();
        j.irreversible = false;
        assert_eq!(
            classify_unfinished_journal(&j),
            JournalStartupAction::EnterRecovery
        );
    }

    #[test]
    fn classify_rolling_back_remains_rollback_when_irreversible() {
        // Startup match arms (not classify) send irreversible RollBack to Recovery.
        let mut j = MaintenanceJournal::new("restore");
        j.stage = "rolling-back".into();
        j.irreversible = true;
        assert_eq!(
            classify_unfinished_journal(&j),
            JournalStartupAction::RollBack
        );
    }

    #[test]
    fn classify_pre_swap_as_rollback() {
        let j = MaintenanceJournal::new("restore");
        assert_eq!(
            classify_unfinished_journal(&j),
            JournalStartupAction::RollBack
        );
    }

    #[test]
    fn apply_clears_completed_and_inactive_journals() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();

        let mut completed = MaintenanceJournal::new("restore");
        completed.stage = "completed".into();
        persist_journal(&paths, &completed).unwrap();
        let action = classify_unfinished_journal(&completed);
        assert_eq!(
            apply_startup_decision(&paths, &completed, action).unwrap(),
            JournalStartupOutcome::Cleared
        );
        assert!(load_journal(&paths).unwrap().is_none());

        let mut inactive = MaintenanceJournal::new("restore");
        inactive.stage = "inactive".into();
        persist_journal(&paths, &inactive).unwrap();
        let action = classify_unfinished_journal(&inactive);
        assert_eq!(
            apply_startup_decision(&paths, &inactive, action).unwrap(),
            JournalStartupOutcome::Cleared
        );
        assert!(load_journal(&paths).unwrap().is_none());
    }

    #[test]
    fn apply_safe_rollback_clears_journal_without_deleting_staged_tree() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();
        let staged = paths.product_root.join("profiles").join("staged-op");
        fs::create_dir_all(&staged).unwrap();
        fs::write(staged.join("marker.txt"), b"keep").unwrap();

        let mut j = MaintenanceJournal::new("restore");
        j.touch_stage(MaintenanceStage::Staging);
        j.staged_profile = Some(staged.to_string_lossy().into_owned());
        j.irreversible = false;
        persist_journal(&paths, &j).unwrap();

        let action = classify_unfinished_journal(&j);
        assert_eq!(action, JournalStartupAction::RollBack);
        assert_eq!(
            apply_startup_decision(&paths, &j, action).unwrap(),
            JournalStartupOutcome::Cleared
        );
        assert!(load_journal(&paths).unwrap().is_none());
        assert!(staged.join("marker.txt").exists());
    }

    #[test]
    fn apply_mid_swap_enters_recovery_without_clearing() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();
        let mut j = MaintenanceJournal::new("restore");
        j.stage = "swapping".into();
        j.irreversible = true;
        persist_journal(&paths, &j).unwrap();

        let action = classify_unfinished_journal(&j);
        assert_eq!(action, JournalStartupAction::EnterRecovery);
        assert_eq!(
            apply_startup_decision(&paths, &j, action).unwrap(),
            JournalStartupOutcome::EnterRecovery {
                reason: "unfinished maintenance journal",
            }
        );
        assert!(load_journal(&paths).unwrap().is_some());
    }

    #[test]
    fn restore_stage_ladder_pre_swap_clears_mid_swap_recovers() {
        // Staging / Validating → RollBack + clear (safe).
        for stage in [
            MaintenanceStage::Staging,
            MaintenanceStage::Validating,
            MaintenanceStage::CancellingWork,
        ] {
            let mut j = MaintenanceJournal::new("restore");
            j.touch_stage(stage);
            assert!(!j.irreversible, "{stage:?} must remain reversible");
            assert_eq!(
                classify_unfinished_journal(&j),
                JournalStartupAction::RollBack,
                "{stage:?}"
            );
        }

        // Swapping / Reopening / Rehydrating → EnterRecovery (irreversible).
        for stage in [
            MaintenanceStage::Swapping,
            MaintenanceStage::Reopening,
            MaintenanceStage::Rehydrating,
        ] {
            let mut j = MaintenanceJournal::new("restore");
            j.touch_stage(stage);
            assert!(j.irreversible, "{stage:?} must be irreversible");
            assert_eq!(
                classify_unfinished_journal(&j),
                JournalStartupAction::EnterRecovery,
                "{stage:?}"
            );
        }

        // Explicit RecoveryRequired after mid-swap failure.
        let mut failed = MaintenanceJournal::new("restore");
        failed.touch_stage(MaintenanceStage::Swapping);
        failed.touch_stage(MaintenanceStage::RecoveryRequired);
        assert_eq!(
            classify_unfinished_journal(&failed),
            JournalStartupAction::EnterRecovery
        );

        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();
        persist_journal(&paths, &failed).unwrap();
        let action = classify_unfinished_journal(&failed);
        assert_eq!(
            apply_startup_decision(&paths, &failed, action).unwrap(),
            JournalStartupOutcome::EnterRecovery {
                reason: "unfinished maintenance journal",
            }
        );
        assert!(load_journal(&paths).unwrap().is_some());

        // Pre-swap Validating: apply clears journal.
        let mut pre = MaintenanceJournal::new("restore");
        pre.touch_stage(MaintenanceStage::Validating);
        persist_journal(&paths, &pre).unwrap();
        let action = classify_unfinished_journal(&pre);
        assert_eq!(
            apply_startup_decision(&paths, &pre, action).unwrap(),
            JournalStartupOutcome::Cleared
        );
        assert!(load_journal(&paths).unwrap().is_none());
    }

    #[test]
    fn restore_completed_stage_clears_on_startup() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_base(dir.path());
        paths.ensure_dirs().unwrap();
        let mut j = MaintenanceJournal::new("restore");
        j.touch_stage(MaintenanceStage::Completed);
        persist_journal(&paths, &j).unwrap();
        let action = classify_unfinished_journal(&j);
        assert_eq!(action, JournalStartupAction::None);
        assert_eq!(
            apply_startup_decision(&paths, &j, action).unwrap(),
            JournalStartupOutcome::Cleared
        );
        assert!(load_journal(&paths).unwrap().is_none());
    }
}
