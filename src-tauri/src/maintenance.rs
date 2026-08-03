//! Global maintenance-mode boundary for restore / migration / profile swaps.

use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::commands::CommandError;

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

    pub fn set_stage(&mut self, operation_id: &str, stage: MaintenanceStage) -> Result<(), CommandError> {
        let op = self
            .active
            .as_mut()
            .ok_or_else(|| CommandError::new("invalid", "No maintenance operation is active."))?;
        if op.id != operation_id {
            return Err(CommandError::new("invalid", "Maintenance operation id mismatch."));
        }
        if matches!(
            stage,
            MaintenanceStage::Swapping | MaintenanceStage::Reopening | MaintenanceStage::Rehydrating
        ) {
            op.irreversible = true;
        }
        op.stage = stage;
        Ok(())
    }

    pub fn fail(&mut self, operation_id: &str, safe_error: impl Into<String>) -> Result<(), CommandError> {
        let op = self
            .active
            .as_mut()
            .ok_or_else(|| CommandError::new("invalid", "No maintenance operation is active."))?;
        if op.id != operation_id {
            return Err(CommandError::new("invalid", "Maintenance operation id mismatch."));
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
            Some(_) => Err(CommandError::new("invalid", "Maintenance operation id mismatch.")),
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
