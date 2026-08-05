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
pub mod registered_actions;
pub mod testing;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::Database;
use crate::runtime_v2::operations::{validate_operations, AppOperation};
use crate::runtime_v2::outbox::{
    enqueue_outbox, flush_pending_outbox, idempotency_keys_for_batch, idempotency_scope_key,
    lookup_idempotency_outcome, store_idempotency_outcome, validate_dependency_refs, CommitOutcome,
};
use crate::runtime_v2::transactions::{
    apply_transaction_deferred, create_transaction, ApplyResult,
};
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
    /// Authoritative commit outcome — never treat `apply: Some` alone as success.
    #[serde(default = "default_failed_outcome")]
    pub outcome: CommitOutcome,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<String>,
}

fn default_failed_outcome() -> CommitOutcome {
    CommitOutcome::FailedBeforeCommit
}

impl ChangeResult {
    pub fn is_committed(&self) -> bool {
        self.outcome.is_success()
            && self
                .apply
                .as_ref()
                .map(|a| a.transaction.status == "applied" && a.conflicts.is_empty())
                .unwrap_or(false)
    }
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
    validate_dependency_refs(&req.operations).map_err(KernelError::Validation)?;
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
            outcome: CommitOutcome::RejectedValidation,
            conflicts: vec![],
        });
    }

    // Durable idempotency: only when every op is keyed (partial keys would skip unkeyed ops).
    let idem_keys = idempotency_keys_for_batch(&req.operations);
    if let Some(scope) = idempotency_scope_key(
        req.conversation_id.as_deref(),
        req.turn_id.as_deref(),
        &idem_keys,
    ) {
        if let Some((outcome, prior)) =
            lookup_idempotency_outcome(db, &scope).map_err(KernelError::Db)?
        {
            if outcome == "committed" {
                let apply: Option<ApplyResult> =
                    serde_json::from_value(prior.get("apply").cloned().unwrap_or(Value::Null)).ok();
                // Crash between COMMIT and flush leaves pending outbox — drain on recover.
                let _ = flush_pending_outbox(db, bus);
                let recovered_ok = apply
                    .as_ref()
                    .is_some_and(|a| a.transaction.status == "applied" && a.conflicts.is_empty());
                if recovered_ok {
                    return Ok(ChangeResult {
                        apply,
                        proposal_id: None,
                        risk,
                        impact_summary: impact,
                        policy,
                        verification: None,
                        operations: None,
                        summary: Some(req.summary.clone()),
                        outcome: CommitOutcome::RecoveredPriorCommitted,
                        conflicts: vec![],
                    });
                }
                // Corrupt prior record — never claim recovered success.
                return Ok(ChangeResult {
                    apply: None,
                    proposal_id: None,
                    risk,
                    impact_summary: impact,
                    policy,
                    verification: None,
                    operations: None,
                    summary: Some(req.summary.clone()),
                    outcome: CommitOutcome::FailedBeforeCommit,
                    conflicts: vec!["corrupt idempotency outcome record".into()],
                });
            }
        }
    }

    // One authoritative SQLite transaction covering data, manifest, surfaces,
    // provenance, and outbox inserts. EventBus is only mutated after COMMIT.
    db.conn()
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?;

    let mut deferred = Vec::new();
    let inner = (|| -> Result<ChangeResult, KernelError> {
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
            apply_transaction_deferred(db, &txn.id, &mut deferred).map_err(KernelError::Db)?;

        if apply.transaction.status != "applied" || !apply.conflicts.is_empty() {
            return Ok(ChangeResult {
                apply: None,
                proposal_id: None,
                risk: risk.clone(),
                impact_summary: impact.clone(),
                policy: policy.clone(),
                verification: None,
                operations: None,
                summary: Some(req.summary.clone()),
                outcome: CommitOutcome::Conflicted,
                conflicts: apply.conflicts,
            });
        }

        record_provenance(db, &txn.id, &req, "applied", Some("passed"), None)
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

        for (i, effect) in deferred.iter().enumerate() {
            enqueue_outbox(
                db,
                Some(&txn.id),
                req.conversation_id.as_deref(),
                req.turn_id.as_deref(),
                None,
                i as i64,
                effect,
            )
            .map_err(KernelError::Db)?;
        }

        if let Some(scope) = idempotency_scope_key(
            req.conversation_id.as_deref(),
            req.turn_id.as_deref(),
            &idem_keys,
        ) {
            let stored = serde_json::json!({ "apply": apply });
            store_idempotency_outcome(
                db,
                &scope,
                None,
                req.conversation_id.as_deref(),
                req.turn_id.as_deref(),
                Some(&txn.id),
                "committed",
                &stored,
            )
            .map_err(KernelError::Db)?;
        }

        Ok(ChangeResult {
            apply: Some(apply),
            proposal_id: None,
            risk: risk.clone(),
            impact_summary: impact.clone(),
            policy: policy.clone(),
            verification,
            operations: None,
            summary: Some(req.summary.clone()),
            outcome: CommitOutcome::Committed,
            conflicts: vec![],
        })
    })();

    match inner {
        Ok(result) if result.outcome.is_success() => {
            db.conn()
                .execute_batch("COMMIT")
                .map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?;
            let _ = flush_pending_outbox(db, bus);
            Ok(result)
        }
        Ok(result) => {
            let _ = db.conn().execute_batch("ROLLBACK");
            Ok(result)
        }
        Err(e) => {
            let _ = db.conn().execute_batch("ROLLBACK");
            Err(e)
        }
    }
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
        "registeredActions": registered_actions::catalog_json(),
        "registeredActionLimits": {
            "maxCallsPerApplicationPerMinute":
                registered_actions::breakers::MAX_CALLS_PER_APPLICATION_PER_MINUTE,
            "maxWritesPerRun": registered_actions::breakers::MAX_WRITES_PER_RUN,
            "maxActionDepth": registered_actions::breakers::MAX_ACTION_DEPTH,
            "maxInputBytes": registered_actions::breakers::MAX_INPUT_BYTES,
            "maxOutputBytes": registered_actions::breakers::MAX_OUTPUT_BYTES,
            "approvalTtlMinutes": registered_actions::approvals::APPROVAL_TTL_MINUTES,
        },
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
    use crate::db::{create_conversation, Database, DEFAULT_WORKSPACE_ID};
    use crate::runtime_v2::operations::{AppOperation, OperationTarget};
    use crate::runtime_v2::surfaces::{create_inline_surface, get_surface};
    use serde_json::json;
    use tempfile::tempdir;

    fn test_db() -> Database {
        let dir = tempdir().unwrap();
        Database::open_path(&dir.path().join("t.db")).unwrap()
    }

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
        assert!(!perms
            .iter()
            .any(|p| p.as_str() == Some("unrestricted.shell")));
        let forbidden = c
            .get("forbiddenPermissions")
            .and_then(|v| v.as_array())
            .unwrap();
        assert!(forbidden
            .iter()
            .any(|p| p.as_str() == Some("unrestricted.shell")));
    }

    #[test]
    fn catalog_lists_registered_actions_with_risk() {
        let c = capability_catalog();
        let actions = c
            .get("registeredActions")
            .and_then(|v| v.as_array())
            .expect("registeredActions array");
        assert!(!actions.is_empty());
        for action in actions {
            assert!(action.get("name").and_then(|v| v.as_str()).is_some());
            let risk = action.get("risk").and_then(|v| v.as_str()).unwrap();
            assert!(matches!(risk, "read" | "write" | "destructive"));
            assert!(action.get("descriptorHash").is_some());
        }
        assert!(c.get("registeredActionLimits").is_some());
    }

    #[test]
    fn third_op_failure_rolls_back_earlier_surface_ops() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Atomic", None).unwrap();
        let def = json!({
            "id": "d",
            "name": "Inline",
            "layout": "stack",
            "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Inline", &def, &[]).unwrap();
        let rev = surface.current_revision;

        let mut op1 = op("state.set");
        op1.target.surface_id = Some(surface.id.clone());
        op1.payload = json!({ "state": { "a": 1 } });

        let mut op2 = op("state.set");
        op2.target.surface_id = Some(surface.id.clone());
        op2.payload = json!({ "state": { "b": 2 } });

        let mut op3 = op("component.update_props");
        op3.target.surface_id = Some(surface.id.clone());
        op3.target.component_id = Some("t".into());
        op3.base_revision = Some(rev - 1); // stale → conflict
        op3.payload = json!({ "props": { "text": "nope" } });

        let result = apply_change(
            &mut db,
            None,
            ChangeRequest {
                conversation_id: Some(conv.id.clone()),
                project_id: None,
                turn_id: Some("turn-atomic-1".into()),
                summary: "batch fail".into(),
                operations: vec![op1, op2, op3],
                silent: false,
                source_type: "user".into(),
                provider: None,
                model: None,
                require_approval: false,
                approval_granted: true,
            },
        )
        .unwrap();

        assert!(!result.is_committed());
        assert_eq!(result.outcome, CommitOutcome::Conflicted);
        assert!(result.apply.is_none());

        // Surface definition unchanged (component update rolled back with batch).
        let after = get_surface(&db, &surface.id).unwrap();
        assert_eq!(after.current_revision, rev);
        let text = after
            .definition
            .pointer("/components/0/props/text")
            .and_then(|v| v.as_str());
        assert_eq!(text, Some("hi"));

        // No applied provenance for a rolled-back batch.
        let prov: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM operation_provenance WHERE validation_status = 'applied'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(prov, 0);
    }

    #[test]
    fn missing_depends_on_fails_closed() {
        let mut db = test_db();
        let mut a = op("state.set");
        a.depends_on = Some(vec!["does-not-exist".into()]);
        let err = apply_change(
            &mut db,
            None,
            ChangeRequest {
                conversation_id: None,
                project_id: None,
                turn_id: None,
                summary: "bad deps".into(),
                operations: vec![a],
                silent: false,
                source_type: "user".into(),
                provider: None,
                model: None,
                require_approval: false,
                approval_granted: true,
            },
        )
        .unwrap_err();
        assert!(matches!(err, KernelError::Validation(_)));
    }

    #[test]
    fn idempotent_retry_returns_prior_committed_outcome() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Idem", None).unwrap();
        let def = json!({
            "id": "d",
            "name": "Inline",
            "layout": "stack",
            "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Inline", &def, &[]).unwrap();

        let mut o = op("state.set");
        o.target.surface_id = Some(surface.id.clone());
        o.idempotency_key = Some("idem-key-1".into());
        o.payload = json!({ "state": { "x": 1 } });

        let req = ChangeRequest {
            conversation_id: Some(conv.id.clone()),
            project_id: None,
            turn_id: Some("turn-idem".into()),
            summary: "idem".into(),
            operations: vec![o.clone()],
            silent: false,
            source_type: "user".into(),
            provider: None,
            model: None,
            require_approval: false,
            approval_granted: true,
        };

        let first = apply_change(&mut db, None, req.clone()).unwrap();
        assert!(first.is_committed());
        let second = apply_change(&mut db, None, req).unwrap();
        assert_eq!(second.outcome, CommitOutcome::RecoveredPriorCommitted);
        assert_eq!(
            first.apply.as_ref().map(|a| a.transaction.id.clone()),
            second.apply.as_ref().map(|a| a.transaction.id.clone())
        );
    }

    #[test]
    fn failed_batch_does_not_persist_subscription_or_bus_effects() {
        use crate::runtime_v2::EventBus;
        let mut db = test_db();
        let mut bus = EventBus::new();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Bus", None).unwrap();
        let def = json!({
            "id": "d",
            "name": "Inline",
            "layout": "stack",
            "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
        });
        let surface = create_inline_surface(&mut db, &conv.id, None, None, "S", &def, &[]).unwrap();

        let mut sub = op("subscription.create");
        sub.target.surface_id = Some(surface.id.clone());
        sub.payload = json!({
            "id": "sub-should-not-live",
            "eventTypes": ["custom"],
            "sourceFilter": {},
            "target": {}
        });

        let mut bad = op("component.update_props");
        bad.target.surface_id = Some(surface.id.clone());
        bad.target.component_id = Some("t".into());
        bad.base_revision = Some(0); // stale
        bad.payload = json!({ "props": { "text": "x" } });

        let result = apply_change(
            &mut db,
            Some(&mut bus),
            ChangeRequest {
                conversation_id: Some(conv.id),
                project_id: None,
                turn_id: Some("turn-bus".into()),
                summary: "bus rollback".into(),
                operations: vec![sub, bad],
                silent: false,
                source_type: "user".into(),
                provider: None,
                model: None,
                require_approval: false,
                approval_granted: true,
            },
        )
        .unwrap();
        assert!(!result.is_committed());
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM surface_subscriptions WHERE id = 'sub-should-not-live'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        let pending_outbox: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM commit_event_outbox WHERE status = 'pending'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pending_outbox, 0);
    }
}
