//! Application Kernel — trusted gateway for generated application changes.
//!
//! All durable mutations to manifests, generated data, permissions, tests,
//! and recovery state must enter through this module. Runtime V2 surface
//! operations continue to use `runtime_v2::transactions`, but agent-facing
//! apply paths should prefer `kernel::apply_change`.

pub mod compiler;
pub mod context;
pub mod data;
pub mod errors;
pub mod impact;
pub mod lifecycle;
pub mod manifest;
pub mod packages;
pub mod permissions;
pub mod policy;
pub mod provenance;
pub mod recovery;
pub mod testing;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{Database, DbResult};
use crate::runtime_v2::operations::{validate_operations, AppOperation};
use crate::runtime_v2::transactions::{create_transaction, ApplyResult};
use crate::security::assert_not_protected;

use self::errors::KernelError;
use self::permissions::FORBIDDEN_PERMISSIONS;
use self::policy::{evaluate_policy, PolicyAction, PolicyDecision};
use self::provenance::record_provenance;

pub const COMPILER_VERSION: &str = "1";
pub const MANIFEST_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequest {
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub turn_id: Option<String>,
    pub summary: String,
    pub operations: Vec<AppOperation>,
    pub silent: bool,
    pub source_type: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub require_approval: bool,
    pub approval_granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeResult {
    pub apply: Option<ApplyResult>,
    pub proposal_id: Option<String>,
    pub risk: String,
    pub impact_summary: String,
    pub policy: PolicyDecision,
    pub verification: Option<testing::VerificationResult>,
    /// Present when a proposal is returned so the UI can re-apply with approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operations: Option<Vec<AppOperation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

fn risk_rank(level: &str) -> u8 {
    match level {
        "strong" => 3,
        "lightweight" => 2,
        _ => 1,
    }
}

fn op_risk(op_type: &str) -> &'static str {
    if op_type.contains("delete")
        || op_type.starts_with("data.migrate")
        || op_type.starts_with("data.model")
        || op_type == "export.prepare"
        || op_type.starts_with("permission.")
        || op_type.starts_with("package.")
    {
        return "strong";
    }
    if op_type.starts_with("surface.")
        || op_type.starts_with("component.")
        || op_type.starts_with("setting.")
        || op_type.starts_with("manifest.")
        || op_type.starts_with("tool.")
    {
        return "lightweight";
    }
    "automatic"
}

/// Classify risk for risk-based approval. Trusted code only — never the agent.
/// Returns the highest risk across the whole batch (order-independent).
pub fn classify_risk(operations: &[AppOperation]) -> &'static str {
    let mut worst = "automatic";
    for op in operations {
        let level = op_risk(&op.op_type);
        if risk_rank(level) > risk_rank(worst) {
            worst = level;
        }
    }
    worst
}

fn assert_ops_not_protected(operations: &[AppOperation]) -> Result<(), KernelError> {
    for op in operations {
        recovery::assert_agent_cannot_disable_recovery(&op.op_type, &op.payload)
            .map_err(KernelError::Protected)?;
        if let Some(tid) = op.target.tool_id.as_ref() {
            assert_not_protected(tid).map_err(KernelError::Protected)?;
        }
        if let Some(sid) = op.target.surface_id.as_ref() {
            assert_not_protected(sid).map_err(KernelError::Protected)?;
        }
        if let Some(app) = op.target.application_id.as_ref() {
            assert_not_protected(app).map_err(KernelError::Protected)?;
        }
        // Reject adversarial permission grants / forbidden perms in payload
        if op.op_type.starts_with("permission.") {
            if let Some(perm) = op.payload.get("permission").and_then(|v| v.as_str()) {
                if FORBIDDEN_PERMISSIONS.contains(&perm) || perm.starts_with("unrestricted.") {
                    return Err(KernelError::PermissionDenied(format!(
                        "forbidden permission: {perm}"
                    )));
                }
            }
            if op.op_type == "permission.grant" {
                return Err(KernelError::PermissionDenied(
                    "agent cannot grant permissions".into(),
                ));
            }
        }
        if op.op_type == "data.execute_sql" || op.payload.get("sql").is_some() {
            return Err(KernelError::Validation(
                "arbitrary SQL is not allowed".into(),
            ));
        }
    }
    Ok(())
}

/// Authoritative mutation entry for operation transactions.
pub fn apply_change(
    db: &mut Database,
    bus: Option<&mut crate::runtime_v2::EventBus>,
    req: ChangeRequest,
) -> Result<ChangeResult, KernelError> {
    validate_operations(&req.operations).map_err(KernelError::Validation)?;
    assert_ops_not_protected(&req.operations)?;

    // Recovery Mode: block agent UI mutations while surfaces are disabled.
    if let Ok(rec) = recovery::get_recovery_state(db) {
        if (rec.recovery_mode || rec.disable_user_surfaces) && req.source_type == "agent" {
            let touches_ui = req.operations.iter().any(|op| {
                let t = op.op_type.as_str();
                t.starts_with("surface.")
                    || t.starts_with("component.")
                    || t.starts_with("layout.")
                    || t.starts_with("manifest.")
            });
            if touches_ui {
                return Err(KernelError::RecoveryRequired(
                    "Recovery Mode is active; user surfaces are disabled".into(),
                ));
            }
        }
    }

    let risk = classify_risk(&req.operations).to_string();
    let impact = impact::summarize_operations(db, &req.operations);
    let policy = evaluate_policy(db, PolicyAction::ApplyOperations, &req)?;

    if policy.decision == "deny" {
        return Err(KernelError::PolicyDenied(policy.message.clone()));
    }

    // Agent cannot self-approve lightweight/strong changes; only automatic-risk ops apply inline.
    let approval_granted = if req.source_type == "agent" {
        false
    } else {
        req.approval_granted
    };

    let needs_approval = matches!(risk.as_str(), "lightweight" | "strong")
        || policy.decision == "require_user_approval"
        || req.require_approval;

    if needs_approval && !approval_granted {
        let proposal_id = format!("proposal-{}", Uuid::new_v4());
        return Ok(ChangeResult {
            apply: None,
            proposal_id: Some(proposal_id),
            risk,
            impact_summary: impact,
            policy,
            verification: None,
            operations: Some(req.operations.clone()),
            summary: Some(req.summary.clone()),
        });
    }

    let txn = create_transaction(
        db,
        req.conversation_id.as_deref(),
        req.project_id.as_deref(),
        req.turn_id.as_deref(),
        &req.summary,
        &req.operations,
        req.silent,
    )
    .map_err(KernelError::Db)?;

    data::apply_kernel_operations(db, &req.operations).map_err(KernelError::Db)?;
    manifest::apply_manifest_operations(db, &req.operations).map_err(KernelError::Db)?;

    let apply =
        crate::runtime_v2::transactions::apply_transaction_with_bus(db, &txn.id, bus)
            .map_err(KernelError::Db)?;

    record_provenance(
        db,
        &txn.id,
        &req,
        "applied",
        Some("passed"),
        None,
    )
    .map_err(KernelError::Db)?;

    let verification = testing::verify_after_change(db, &req.operations).ok();

    if let Some(ref v) = verification {
        if v.status == "verified" {
            for op in &req.operations {
                if let Some(app_id) = op
                    .target
                    .application_id
                    .as_deref()
                    .or_else(|| op.payload.get("applicationId").and_then(|x| x.as_str()))
                {
                    let _ = manifest::mark_last_known_good(db, app_id);
                }
            }
        }
    }

    Ok(ChangeResult {
        apply: Some(apply),
        proposal_id: None,
        risk,
        impact_summary: impact,
        policy,
        verification,
        operations: None,
        summary: Some(req.summary),
    })
}

/// Compact capability introspection for the agent (bounded).
pub fn capability_catalog() -> Value {
    let packs = crate::runtime_v2::packs::bundled_packs();
    serde_json::json!({
        "compilerVersion": COMPILER_VERSION,
        "manifestSchemaVersion": MANIFEST_SCHEMA_VERSION,
        "packs": packs.iter().map(|p| serde_json::json!({
            "id": p.id,
            "version": p.version,
            "components": p.component_types,
            "enabled": p.enabled,
        })).collect::<Vec<_>>(),
        "permissions": permissions::ALLOWED_PERMISSIONS,
        "forbiddenPermissions": FORBIDDEN_PERMISSIONS,
        "operationFamilies": [
            "surface", "component", "state", "setting", "layout",
            "event", "subscription", "chat", "automation", "wallpaper",
            "export", "project", "tool", "manifest", "data", "test"
        ],
        "limits": crate::runtime_v2::limits::limits_json(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_v2::operations::{AppOperation, OperationTarget};

    fn op(op_type: &str) -> AppOperation {
        AppOperation {
            id: format!("op-{}", Uuid::new_v4()),
            op_type: op_type.into(),
            target: OperationTarget::default(),
            base_revision: None,
            payload: serde_json::json!({}),
            transaction_group: None,
            idempotency_key: None,
            depends_on: None,
            requires_approval: None,
            destructive: None,
            audience: None,
        }
    }

    #[test]
    fn risk_classification() {
        assert_eq!(classify_risk(&[op("state.set")]), "automatic");
        assert_eq!(classify_risk(&[op("component.insert")]), "lightweight");
        assert_eq!(classify_risk(&[op("surface.delete")]), "strong");
        // Highest risk wins regardless of order
        assert_eq!(
            classify_risk(&[op("component.insert"), op("surface.delete")]),
            "strong"
        );
        assert_eq!(
            classify_risk(&[op("surface.delete"), op("component.insert")]),
            "strong"
        );
    }

    #[test]
    fn rejects_sql_payload() {
        let mut o = op("state.set");
        o.payload = serde_json::json!({ "sql": "DROP TABLE tools" });
        let err = assert_ops_not_protected(&[o]).unwrap_err();
        assert!(matches!(err, KernelError::Validation(_)));
    }

    #[test]
    fn rejects_permission_self_grant() {
        let mut o = op("permission.grant");
        o.payload = serde_json::json!({ "permission": "local_data.write" });
        assert!(assert_ops_not_protected(&[o]).is_err());
    }

    #[test]
    fn rejects_forbidden_permission() {
        let mut o = op("permission.request");
        o.payload = serde_json::json!({ "permission": "credential.read" });
        assert!(assert_ops_not_protected(&[o]).is_err());
    }

    #[test]
    fn catalog_has_no_unrestricted() {
        let c = capability_catalog();
        let perms = c.get("permissions").and_then(|v| v.as_array()).unwrap();
        assert!(perms.iter().any(|p| p.as_str() == Some("local_data.write")));
        assert!(!perms.iter().any(|p| p.as_str() == Some("unrestricted.shell")));
        let forbidden = c
            .get("forbiddenPermissions")
            .and_then(|v| v.as_array())
            .unwrap();
        assert!(forbidden
            .iter()
            .any(|p| p.as_str() == Some("unrestricted.shell")));
    }
}
