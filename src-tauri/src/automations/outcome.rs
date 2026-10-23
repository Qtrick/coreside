//! Typed automation execution outcomes (never invent success from IPC transport alone).

use serde::{Deserialize, Serialize};

/// Authoritative run result for scheduler persistence and `run_now` IPC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRunOutcome {
    /// success | awaiting_approval | error | blocked | skipped | cancelled | busy
    pub status: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
}

impl AutomationRunOutcome {
    pub fn success(summary: impl Into<String>) -> Self {
        Self {
            status: "success".into(),
            summary: summary.into(),
            error_category: None,
        }
    }

    pub fn awaiting_approval(summary: impl Into<String>) -> Self {
        Self {
            status: "awaiting_approval".into(),
            summary: summary.into(),
            error_category: None,
        }
    }

    pub fn error(summary: impl Into<String>, category: impl Into<String>) -> Self {
        Self {
            status: "error".into(),
            summary: summary.into(),
            error_category: Some(category.into()),
        }
    }

    pub fn blocked(summary: impl Into<String>) -> Self {
        Self {
            status: "blocked".into(),
            summary: summary.into(),
            error_category: Some("permission_blocked".into()),
        }
    }

    pub fn skipped(summary: impl Into<String>) -> Self {
        Self {
            status: "skipped".into(),
            summary: summary.into(),
            error_category: None,
        }
    }

    pub fn busy() -> Self {
        Self {
            status: "busy".into(),
            summary: "Automation is already running".into(),
            error_category: Some("already_running".into()),
        }
    }

    pub fn is_failure(&self) -> bool {
        matches!(self.status.as_str(), "error" | "blocked")
    }

    /// Counts toward consecutive-failure pause thresholds.
    pub fn counts_as_failure(&self) -> bool {
        self.is_failure()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awaiting_approval_does_not_count_as_failure() {
        let parked = AutomationRunOutcome::awaiting_approval("Waiting for approval");
        assert!(!parked.counts_as_failure());
        assert!(!parked.is_failure());
        assert_eq!(parked.status, "awaiting_approval");
    }

    #[test]
    fn skipped_and_busy_do_not_count_as_failure() {
        assert!(!AutomationRunOutcome::skipped("missed").counts_as_failure());
        assert!(!AutomationRunOutcome::busy().counts_as_failure());
        assert!(!AutomationRunOutcome::success("ok").counts_as_failure());
    }

    #[test]
    fn error_and_blocked_count_as_failure() {
        assert!(AutomationRunOutcome::error("boom", "execution_error").counts_as_failure());
        assert!(AutomationRunOutcome::blocked("no grant").counts_as_failure());
    }
}
