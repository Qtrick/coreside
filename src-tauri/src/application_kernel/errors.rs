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
            Self::Db(_) => "A local storage error occurred.".into(),
        }
    }
}
