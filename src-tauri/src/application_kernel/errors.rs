//! Kernel error categories (user-safe messages; no secrets).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("validation_failed: {0}")]
    Validation(String),
    #[error("permission_denied: {0}")]
    PermissionDenied(String),
    #[error("policy_denied: {0}")]
    PolicyDenied(String),
    #[error("protected_resource: {0}")]
    Protected(String),
    #[error("revision_conflict: {0}")]
    RevisionConflict(String),
    #[error("capability_unavailable: {0}")]
    CapabilityUnavailable(String),
    #[error("resource_limit_exceeded: {0}")]
    ResourceLimit(String),
    #[error("migration_failed: {0}")]
    MigrationFailed(String),
    #[error("package_invalid: {0}")]
    PackageInvalid(String),
    #[error("recovery_required: {0}")]
    RecoveryRequired(String),
    #[error("db: {0}")]
    Db(#[from] crate::db::DbError),
}

impl KernelError {
    pub fn category(&self) -> &'static str {
        match self {
            Self::Validation(_) => "validation_failed",
            Self::PermissionDenied(_) => "permission_denied",
            Self::PolicyDenied(_) => "policy_denied",
            Self::Protected(_) => "permission_denied",
            Self::RevisionConflict(_) => "revision_conflict",
            Self::CapabilityUnavailable(_) => "capability_unavailable",
            Self::ResourceLimit(_) => "resource_limit_exceeded",
            Self::MigrationFailed(_) => "migration_failed",
            Self::PackageInvalid(_) => "package_invalid",
            Self::RecoveryRequired(_) => "recovery_required",
            // DbError::Conflict is Coreside-authored (idempotency / CAS); map it
            // like other conflicts so IPC does not collapse it to unknown_error.
            Self::Db(crate::db::DbError::Conflict(_)) => "conflict",
            Self::Db(_) => "unknown_error",
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Validation(m) => format!("This change could not be validated: {m}"),
            Self::PermissionDenied(m) => format!("Permission denied: {m}"),
            Self::PolicyDenied(m) => format!("This action is not allowed: {m}"),
            Self::Protected(m) => format!("Protected Coreside resources cannot be changed: {m}"),
            Self::RevisionConflict(_) => {
                "This application was updated elsewhere. Reload and try again.".into()
            }
            Self::CapabilityUnavailable(m) => {
                format!("A required capability is unavailable: {m}")
            }
            Self::ResourceLimit(m) => format!("This change exceeds a safety limit: {m}"),
            Self::MigrationFailed(m) => format!("Data migration failed: {m}"),
            Self::PackageInvalid(m) => format!("Package rejected: {m}"),
            Self::RecoveryRequired(m) => format!("Recovery is required: {m}"),
            Self::Db(crate::db::DbError::Conflict(m)) => m.clone(),
            Self::Db(_) => "A local storage error occurred.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_conflict_surfaces_as_conflict_category() {
        let err = KernelError::Db(crate::db::DbError::Conflict(
            "idempotency key was already used for a different request body".into(),
        ));
        assert_eq!(err.category(), "conflict");
        assert!(err.user_message().contains("idempotency key"));
    }

    #[test]
    fn non_conflict_db_error_stays_unknown_category() {
        // Only Conflict is remapped — other Db errors must not leak as conflict.
        let err = KernelError::Db(crate::db::DbError::NotFound("row".into()));
        assert_eq!(err.category(), "unknown_error");
        assert_eq!(err.user_message(), "A local storage error occurred.");
    }
}
