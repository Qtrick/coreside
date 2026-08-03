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
            MaintenanceStage::Swapping | MaintenanceStage::Reopening | MaintenanceStage::Rehydrating
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
pub fn validate_journal_paths(journal: &MaintenanceJournal, paths: &AppPaths) -> Result<(), CommandError> {
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
    let bytes = fs::read(&path).map_err(|e| {
        CommandError::new("storage", sanitize_error(&e.to_string(), None))
    })?;
    let journal: MaintenanceJournal = serde_json::from_slice(&bytes).map_err(|e| {
        CommandError::new(
            "recovery_required",
            format!("Unreadable maintenance journal: {}", sanitize_error(&e.to_string(), None)),
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
    paths.ensure_dirs().map_err(|e| {
        CommandError::new("storage", sanitize_error(&e.to_string(), None))
    })?;
    validate_journal_paths(journal, paths)?;
    let path = journal_path(paths);
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(journal).map_err(|e| {
        CommandError::new("storage", sanitize_error(&e.to_string(), None))
    })?;
    {
        let mut f = fs::File::create(&tmp).map_err(|e| {
            CommandError::new("storage", sanitize_error(&e.to_string(), None))
        })?;
        f.write_all(&json).map_err(|e| {
            CommandError::new("storage", sanitize_error(&e.to_string(), None))
        })?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &path).map_err(|e| {
        CommandError::new("storage", sanitize_error(&e.to_string(), None))
    })?;
    Ok(())
}

pub fn clear_journal(paths: &AppPaths) -> Result<(), CommandError> {
    let path = journal_path(paths);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| {
            CommandError::new("storage", sanitize_error(&e.to_string(), None))
        })?;
    }
    let tmp = path.with_extension("json.tmp");
    let _ = fs::remove_file(tmp);
    Ok(())
}

/// Classify an unfinished journal without deleting profile trees.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JournalStartupAction {
    None,
    Resume,
    RollBack,
    EnterRecovery,
}

pub fn classify_unfinished_journal(journal: &MaintenanceJournal) -> JournalStartupAction {
    match journal.stage.as_str() {
        "completed" | "inactive" => JournalStartupAction::None,
        "failed" | "recovery-required" => JournalStartupAction::EnterRecovery,
        "swapping" | "reopening" | "rehydrating" if journal.irreversible => {
            JournalStartupAction::EnterRecovery
        }
        "rolling-back" => JournalStartupAction::RollBack,
        "preparing" | "cancelling-work" | "flushing" | "safety-backup" | "staging"
        | "validating" | "awaiting-confirmation" => JournalStartupAction::RollBack,
        _ => JournalStartupAction::EnterRecovery,
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
    fn classify_pre_swap_as_rollback() {
        let j = MaintenanceJournal::new("restore");
        assert_eq!(
            classify_unfinished_journal(&j),
            JournalStartupAction::RollBack
        );
    }
}
