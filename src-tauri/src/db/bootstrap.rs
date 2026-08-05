//! Startup bootstrap status when the primary profile database cannot open.

use serde::Serialize;
use std::path::{Path, PathBuf};

use super::{product_data_dir, Database, DbError, DbResult};

/// Consumer-safe startup states for the protected shell.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum BootstrapStatus {
    Ready,
    /// Field rename_all must be on the variant — enum-level rename_all only
    /// affects variant tag names, not struct-variant fields.
    #[serde(rename_all = "camelCase")]
    RecoveryRequired {
        reason_code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        database_path: Option<String>,
        using_shell_database: bool,
    },
}

impl BootstrapStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn recovery(
        reason_code: impl Into<String>,
        message: impl Into<String>,
        database_path: Option<PathBuf>,
        using_shell_database: bool,
    ) -> Self {
        Self::RecoveryRequired {
            reason_code: reason_code.into(),
            message: message.into(),
            database_path: database_path.map(|p| p.display().to_string()),
            using_shell_database,
        }
    }
}

pub(crate) fn classify_open_error(err: &DbError) -> (&'static str, String) {
    let raw = err.to_string();
    let lower = raw.to_lowercase();
    if lower.contains("locked") || lower.contains("busy") {
        (
            "database_locked",
            "The local database is busy or locked. Close other Coreside windows and try again."
                .into(),
        )
    } else if lower.contains("corrupt") || lower.contains("malformed") {
        (
            "integrity_failed",
            "The local database could not be verified. Your original file was preserved when possible."
                .into(),
        )
    } else if lower.contains("migration") || lower.contains("migrate") {
        (
            "migration_failed",
            "Coreside could not finish updating local data. Your previous database was preserved when possible."
                .into(),
        )
    } else if lower.contains("permission")
        || lower.contains("read-only")
        || lower.contains("writable")
    {
        (
            "storage_unavailable",
            "Coreside could not write to its application data folder.".into(),
        )
    } else {
        (
            "database_unavailable",
            "Coreside could not open local data. Recovery tools are available.".into(),
        )
    }
}

/// Path for the ephemeral shell database used only when the profile DB fails.
pub fn shell_database_path() -> DbResult<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| DbError::Invalid("Could not resolve application data directory".into()))?;
    let dir = product_data_dir(&base).join("recovery");
    std::fs::create_dir_all(&dir)
        .map_err(|e| DbError::Invalid(format!("Failed to create recovery directory: {e}")))?;
    Ok(dir.join("shell.db"))
}

pub fn open_shell_database() -> DbResult<Database> {
    let path = shell_database_path()?;
    // Fresh shell DB each recovery session so stale shell state never looks like a profile.
    if path.exists() {
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }
    Database::open_path(&path)
}

/// Open the profile database, or a recovery shell database when that fails.
pub fn open_profile_or_shell() -> (Database, BootstrapStatus) {
    match Database::open_default() {
        Ok(db) => (db, BootstrapStatus::Ready),
        Err(err) => {
            let (code, message) = classify_open_error(&err);
            tracing::error!(error = %err, reason = code, "profile database unavailable");
            let profile_path = super::default_db_path_for_diagnostics().ok();
            match open_shell_database() {
                Ok(shell) => (
                    shell,
                    BootstrapStatus::recovery(code, message, profile_path, true),
                ),
                Err(shell_err) => {
                    tracing::error!(error = %shell_err, "recovery shell database also failed");
                    // Last resort: in-memory shell so the protected UI can still launch.
                    match Database::open_path(Path::new(":memory:")) {
                        Ok(mem) => (
                            mem,
                            BootstrapStatus::recovery(
                                "storage_unavailable",
                                "Coreside could not open local storage. Recovery options are limited until disk access is restored.",
                                profile_path,
                                true,
                            ),
                        ),
                        Err(mem_err) => {
                            // Should be unreachable with bundled sqlite; re-panic only here.
                            panic!(
                                "Coreside could not open a recovery database: {mem_err} (profile: {err})"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_locked_errors() {
        let (code, _) = classify_open_error(&DbError::Invalid("database is locked".into()));
        assert_eq!(code, "database_locked");
    }

    #[test]
    fn ready_status_is_ready() {
        assert!(BootstrapStatus::Ready.is_ready());
        assert!(!BootstrapStatus::recovery("x", "y", None, true).is_ready());
    }

    #[test]
    fn bootstrap_status_serde_matches_frontend_camel_case() {
        let ready = serde_json::to_value(BootstrapStatus::Ready).unwrap();
        assert_eq!(ready, serde_json::json!({ "status": "ready" }));

        let recovery = BootstrapStatus::recovery(
            "integrity_failed",
            "Could not verify.",
            Some(PathBuf::from("/tmp/coreside.db")),
            true,
        );
        let value = serde_json::to_value(&recovery).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "status": "recoveryRequired",
                "reasonCode": "integrity_failed",
                "message": "Could not verify.",
                "databasePath": "/tmp/coreside.db",
                "usingShellDatabase": true,
            })
        );
    }
}
