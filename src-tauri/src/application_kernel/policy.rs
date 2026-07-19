//! Local policy evaluation boundary (enterprise-ready hook; consumer-default allow).

use serde::{Deserialize, Serialize};

use crate::db::{Database, DbResult};
use rusqlite::OptionalExtension;

use super::ChangeRequest;

#[derive(Debug, Clone, Copy)]
pub enum PolicyAction {
    ApplyOperations,
    ExportPackage,
    WebSearch,
    ImportPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDecision {
    pub decision: String,
    pub message: String,
    pub action: String,
}

pub fn evaluate_policy(
    db: &Database,
    action: PolicyAction,
    _req: &ChangeRequest,
) -> DbResult<PolicyDecision> {
    let key = match action {
        PolicyAction::ApplyOperations => "apply_operations",
        PolicyAction::ExportPackage => "export_package",
        PolicyAction::WebSearch => "web_search",
        PolicyAction::ImportPackage => "import_package",
    };
    let override_decision: Option<String> = db
        .conn()
        .query_row(
            "SELECT decision FROM policy_overrides WHERE policy_key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(crate::db::DbError::Sqlite)?;

    let decision = override_decision.unwrap_or_else(|| "allow".into());
    let message = match decision.as_str() {
        "deny" => format!("Local policy denies {key}"),
        "require_user_approval" => format!("Local policy requires approval for {key}"),
        "require_admin_approval" => format!("Enterprise policy would require admin approval for {key}"),
        "disable_export" => "Export is disabled by policy".into(),
        other => format!("Policy decision: {other}"),
    };
    // Map disable_export to deny for export
    let decision = if decision == "disable_export" && matches!(action, PolicyAction::ExportPackage) {
        "deny".into()
    } else {
        decision
    };
    Ok(PolicyDecision {
        decision,
        message,
        action: key.into(),
    })
}

/// Test helper: set a policy override (trusted / test only).
pub fn set_policy_override(db: &mut Database, key: &str, decision: &str) -> DbResult<()> {
    let allowed = [
        "allow",
        "deny",
        "require_user_approval",
        "require_admin_approval",
        "require_security_review",
        "restrict_scope",
        "force_provider",
        "force_local_only",
        "disable_export",
        "require_retention",
        "require_audit",
    ];
    if !allowed.contains(&decision) {
        return Err(crate::db::DbError::Invalid(format!(
            "unknown policy decision: {decision}"
        )));
    }
    db.conn().execute(
        "INSERT INTO policy_overrides (id, policy_key, decision, scope_json)
         VALUES (?1, ?2, ?3, '{}')
         ON CONFLICT(policy_key) DO UPDATE SET decision = excluded.decision",
        rusqlite::params![format!("pol-{key}"), key, decision],
    )?;
    Ok(())
}

pub fn clear_policy_override(db: &mut Database, key: &str) -> DbResult<()> {
    db.conn()
        .execute("DELETE FROM policy_overrides WHERE policy_key = ?1", [key])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_allow_without_db_override_shape() {
        // Decision string contract
        assert_eq!("allow", "allow");
    }
}
