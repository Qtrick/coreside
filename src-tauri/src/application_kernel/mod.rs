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

use std::collections::HashMap;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
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

impl Default for ChangeRequest {
    fn default() -> Self {
        Self {
            proposal_id: None,
            conversation_id: None,
            project_id: None,
            turn_id: None,
            summary: String::new(),
            operations: Vec::new(),
            silent: false,
            source_type: "user".into(),
            provider: None,
            model: None,
            require_approval: false,
            approval_granted: false,
        }
    }
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
    mut req: ChangeRequest,
) -> Result<ChangeResult, KernelError> {
    if let Some(proposal_id) = req.proposal_id.as_deref() {
        return decide_proposal(db, bus, proposal_id, req.approval_granted);
    }

    req.operations = crate::runtime_v2::normalize_operations_for_validation(&req.operations);

    validate_operations(&req.operations).map_err(KernelError::Validation)?;
    validate_dependency_refs(&req.operations).map_err(KernelError::Validation)?;
    assert_ops_not_protected(&req.operations)?;

    // Recovery Mode: block agent UI mutations while surfaces are disabled.
    // Fail-closed: if recovery state cannot be read, block agent mutations
    // rather than silently allowing them through.
    let rec = recovery::get_recovery_state(db)
        .map_err(|e| KernelError::Validation(format!("unable to read recovery state: {e}")))?;
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
        let mut base_revisions = HashMap::new();
        let mut newly_created_surfaces = std::collections::HashSet::new();
        for op in &req.operations {
            if matches!(
                op.op_type.as_str(),
                "surface.create" | "chat.inline_surface_create"
            ) {
                if let Some(sid) = op.target.surface_id.clone().or_else(|| {
                    op.target
                        .tool_id
                        .as_deref()
                        .map(crate::runtime_v2::surfaces::surface_id_for_tool)
                }) {
                    newly_created_surfaces.insert(sid);
                }
                if let Some(sid) = op.payload.get("surfaceId").and_then(|v| v.as_str()) {
                    newly_created_surfaces.insert(sid.to_string());
                }
                if let Some(tid) = op.payload.get("toolId").and_then(|v| v.as_str()) {
                    newly_created_surfaces
                        .insert(crate::runtime_v2::surfaces::surface_id_for_tool(tid));
                }
                if let Some(tool) = op.payload.get("tool").and_then(|t| t.as_object()) {
                    if let Some(tid) = tool.get("id").and_then(|id| id.as_str()) {
                        newly_created_surfaces
                            .insert(crate::runtime_v2::surfaces::surface_id_for_tool(tid));
                    }
                }
            }
        }

        for op in &req.operations {
            let sid_opt = op.target.surface_id.clone().or_else(|| {
                op.target
                    .tool_id
                    .as_deref()
                    .map(crate::runtime_v2::surfaces::surface_id_for_tool)
            });
            if let Some(sid) = sid_opt {
                if newly_created_surfaces.contains(&sid) {
                    base_revisions.entry(sid).or_insert(0);
                    continue;
                }
                if !base_revisions.contains_key(&sid) {
                    let rev: i64 = db
                        .conn()
                        .query_row(
                            "SELECT current_revision FROM surfaces WHERE id = ?",
                            rusqlite::params![sid],
                            |r| r.get(0),
                        )
                        .map_err(|e| match e {
                            rusqlite::Error::QueryReturnedNoRows => {
                                KernelError::Validation(format!("Target surface {sid} not found"))
                            }
                            other => KernelError::Db(crate::db::DbError::Sqlite(other)),
                        })?;
                    base_revisions.insert(sid, rev);
                }
            }
        }

        let hash_payload = serde_json::json!({
            "operations": &req.operations,
            "base_revisions": &base_revisions,
            "summary": &req.summary,
            "conversation_id": &req.conversation_id,
        });
        let operations_hash = registered_actions::canonical::hash_value(&hash_payload);
        let proposal_id = format!("proposal-{}", Uuid::new_v4());
        let now = chrono::Utc::now().to_rfc3339();
        let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
        let ops_json = serde_json::to_string(&req.operations).unwrap_or_else(|_| "[]".into());
        let base_revs_json = serde_json::to_string(&base_revisions).unwrap_or_else(|_| "{}".into());

        db.conn()
            .execute(
                "INSERT INTO kernel_change_proposals (
                id, conversation_id, turn_id, summary, risk, impact_summary,
                exact_operations_json, exact_operations_hash, base_revisions_json, source_type,
                model, provider, status, created_at, expires_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)",
                rusqlite::params![
                    proposal_id,
                    req.conversation_id,
                    req.turn_id,
                    req.summary,
                    risk,
                    impact,
                    ops_json,
                    operations_hash,
                    base_revs_json,
                    req.source_type,
                    req.model,
                    req.provider,
                    now,
                    expires_at,
                ],
            )
            .map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?;

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
            if let Err(e) = db.conn().execute_batch("ROLLBACK") {
                tracing::error!(error = %e, "ROLLBACK failed after non-success outcome");
            }
            Ok(result)
        }
        Err(e) => {
            if let Err(rb_err) = db.conn().execute_batch("ROLLBACK") {
                tracing::error!(error = %rb_err, original_error = %e, "ROLLBACK failed after error");
            }
            Err(e)
        }
    }
}

/// Decide (approve or reject) a frozen kernel change proposal with single-use CAS semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelChangeProposalRecord {
    pub id: String,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub summary: String,
    pub risk: String,
    pub impact_summary: String,
    pub exact_operations: Vec<AppOperation>,
    pub exact_operations_hash: String,
    pub base_revisions: HashMap<String, i64>,
    pub source_type: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub decided_at: Option<String>,
    pub applied_at: Option<String>,
    pub transaction_id: Option<String>,
}

fn parse_proposal_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<KernelChangeProposalRecord> {
    let ops_json: String = r.get(6)?;
    let base_revs_json: String = r.get(8)?;
    // SECURITY: fail closed — a malformed ops JSON must not silently become an empty
    // operation list. An empty list would either do nothing (corrupted proposal applied
    // as a no-op) or, in the worst case, allow the caller to believe a proposal is valid
    // when its operations are unreadable. Return an error to propagate to the caller.
    let ops: Vec<AppOperation> = serde_json::from_str(&ops_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("proposal exact_operations_json is malformed: {e}"),
            )),
        )
    })?;
    let base_revisions: HashMap<String, i64> =
        serde_json::from_str(&base_revs_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                8,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("proposal base_revisions_json is malformed: {e}"),
                )),
            )
        })?;
    Ok(KernelChangeProposalRecord {
        id: r.get(0)?,
        conversation_id: r.get(1)?,
        turn_id: r.get(2)?,
        summary: r.get(3)?,
        risk: r.get(4)?,
        impact_summary: r.get(5)?,
        exact_operations: ops,
        exact_operations_hash: r.get(7)?,
        base_revisions,
        source_type: r.get(9)?,
        model: r.get(10)?,
        provider: r.get(11)?,
        status: r.get(12)?,
        error: r.get(13)?,
        created_at: r.get(14)?,
        expires_at: r.get(15)?,
        decided_at: r.get(16)?,
        applied_at: r.get(17)?,
        transaction_id: r.get(18)?,
    })
}

pub fn get_proposal(
    db: &Database,
    proposal_id: &str,
) -> Result<KernelChangeProposalRecord, KernelError> {
    db.conn()
        .query_row(
            "SELECT id, conversation_id, turn_id, summary, risk, impact_summary, \
                    exact_operations_json, exact_operations_hash, base_revisions_json, \
                    source_type, model, provider, status, error, created_at, expires_at, \
                    decided_at, applied_at, transaction_id \
             FROM kernel_change_proposals WHERE id = ?",
            rusqlite::params![proposal_id],
            parse_proposal_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                KernelError::Validation(format!("Proposal {proposal_id} not found"))
            }
            other => KernelError::Db(crate::db::DbError::Sqlite(other)),
        })
}

pub fn list_pending_proposals(
    db: &Database,
    conversation_id: Option<&str>,
) -> Result<Vec<KernelChangeProposalRecord>, KernelError> {
    let mut sql = "SELECT id, conversation_id, turn_id, summary, risk, impact_summary, \
                          exact_operations_json, exact_operations_hash, base_revisions_json, \
                          source_type, model, provider, status, error, created_at, expires_at, \
                          decided_at, applied_at, transaction_id \
                   FROM kernel_change_proposals WHERE status = 'pending'"
        .to_string();
    if conversation_id.is_some() {
        sql.push_str(" AND conversation_id = ? ORDER BY created_at DESC");
    } else {
        sql.push_str(" ORDER BY created_at DESC");
    }

    let mut stmt = db
        .conn()
        .prepare(&sql)
        .map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?;
    let rows = if let Some(cid) = conversation_id {
        stmt.query_map(rusqlite::params![cid], parse_proposal_row)
    } else {
        stmt.query_map([], parse_proposal_row)
    }
    .map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?);
    }
    Ok(out)
}

/// Decide (approve or reject) a frozen kernel change proposal with atomic single-use CAS semantics.
pub fn decide_proposal(
    db: &mut Database,
    bus: Option<&mut crate::runtime_v2::EventBus>,
    proposal_id: &str,
    approve: bool,
) -> Result<ChangeResult, KernelError> {
    let now = chrono::Utc::now();
    let proposal = get_proposal(db, proposal_id)?;

    if proposal.status != "pending" {
        return Err(KernelError::Validation(format!(
            "Proposal {proposal_id} is already {} (cannot decide)",
            proposal.status
        )));
    }

    if let Some(exp_str) = &proposal.expires_at {
        if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(exp_str) {
            if now > exp {
                let _ = db.conn().execute(
                    "UPDATE kernel_change_proposals SET status = 'expired', error = 'Proposal has expired' WHERE id = ? AND status = 'pending'",
                    rusqlite::params![proposal_id],
                );
                return Err(KernelError::Validation("Proposal has expired".into()));
            }
        }
    }

    if !approve {
        let updated = db.conn().execute(
            "UPDATE kernel_change_proposals SET status = 'rejected', decided_at = ? WHERE id = ? AND status = 'pending'",
            rusqlite::params![now.to_rfc3339(), proposal_id],
        ).map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?;
        if updated == 0 {
            return Err(KernelError::Validation(
                "Proposal was already claimed or decided concurrently".into(),
            ));
        }
        return Ok(ChangeResult {
            apply: None,
            proposal_id: Some(proposal_id.to_string()),
            risk: proposal.risk,
            impact_summary: proposal.impact_summary,
            policy: PolicyDecision {
                decision: "denied_by_user".into(),
                message: "Proposal rejected by user".into(),
                action: "apply_operations".into(),
            },
            verification: None,
            operations: None,
            summary: Some(proposal.summary),
            outcome: CommitOutcome::RejectedValidation,
            conflicts: vec![],
        });
    }

    // Atomic BEGIN IMMEDIATE covering CAS claim, staleness verification, operations apply, and proposal status commit
    db.conn()
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?;

    // 1. Single-use CAS claim inside transaction
    let updated = db.conn().execute(
        "UPDATE kernel_change_proposals SET status = 'applying', decided_at = ? WHERE id = ? AND status = 'pending'",
        rusqlite::params![now.to_rfc3339(), proposal_id],
    ).map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?;

    if updated == 0 {
        if let Err(e) = db.conn().execute_batch("ROLLBACK") {
            tracing::error!(error = %e, "ROLLBACK failed after proposal already claimed");
        }
        return Err(KernelError::Validation(
            "Proposal was already claimed or decided concurrently".into(),
        ));
    }

    // 2. Check base revisions against surfaces using `current_revision` column!
    for (sid, expected_rev) in &proposal.base_revisions {
        let current_rev_res: Result<i64, _> = db.conn().query_row(
            "SELECT current_revision FROM surfaces WHERE id = ?",
            rusqlite::params![sid],
            |r| r.get(0),
        );
        match current_rev_res {
            Ok(actual) if actual == *expected_rev => {
                // Revision matches
            }
            Ok(actual) => {
                let stale_msg = format!("Proposal is stale: surface {sid} revision moved from {expected_rev} to {actual}");
                let _ = db.conn().execute(
                    "UPDATE kernel_change_proposals SET status = 'stale', error = ? WHERE id = ?",
                    rusqlite::params![stale_msg, proposal_id],
                );
                let _ = db.conn().execute_batch("COMMIT");
                return Err(KernelError::RevisionConflict(stale_msg));
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                if *expected_rev == 0 {
                    // Valid: this surface is newly created by this proposal and does not exist in DB yet.
                } else {
                    let stale_msg =
                        format!("Proposal is stale: target surface {sid} no longer exists");
                    let _ = db.conn().execute(
                        "UPDATE kernel_change_proposals SET status = 'stale', error = ? WHERE id = ?",
                        rusqlite::params![stale_msg, proposal_id],
                    );
                    let _ = db.conn().execute_batch("COMMIT");
                    return Err(KernelError::RevisionConflict(stale_msg));
                }
            }
            Err(e) => {
                if let Err(rb_err) = db.conn().execute_batch("ROLLBACK") {
                    tracing::error!(error = %rb_err, "ROLLBACK failed during revision check");
                }
                return Err(KernelError::Db(crate::db::DbError::Sqlite(e)));
            }
        }
    }

    // 3. Verify operations hash
    let hash_payload = serde_json::json!({
        "operations": &proposal.exact_operations,
        "base_revisions": &proposal.base_revisions,
        "summary": &proposal.summary,
        "conversation_id": &proposal.conversation_id,
    });
    let expected_hash = registered_actions::canonical::hash_value(&hash_payload);
    if expected_hash != proposal.exact_operations_hash {
        let _ = db.conn().execute(
            "UPDATE kernel_change_proposals SET status = 'failed', error = 'Proposal operations hash mismatch (tampered payload)' WHERE id = ?",
            rusqlite::params![proposal_id],
        );
        let _ = db.conn().execute_batch("COMMIT");
        return Err(KernelError::Validation(
            "Proposal operations hash mismatch (tampered payload)".into(),
        ));
    }

    // 4. Apply frozen operations
    let ops = crate::runtime_v2::normalize_operations_for_validation(&proposal.exact_operations);
    let mut deferred = Vec::new();
    let txn_res = create_transaction(
        db,
        proposal.conversation_id.as_deref(),
        None,
        proposal.turn_id.as_deref(),
        &proposal.summary,
        &ops,
        false,
    );
    let txn = match txn_res {
        Ok(t) => t,
        Err(e) => {
            if let Err(rb_err) = db.conn().execute_batch("ROLLBACK") {
                tracing::error!(error = %rb_err, original_error = %e, "ROLLBACK failed during transaction creation");
            }
            let _ = db.conn().execute(
                "UPDATE kernel_change_proposals SET status = 'failed', error = ? WHERE id = ?",
                rusqlite::params![e.to_string(), proposal_id],
            );
            return Err(KernelError::Db(e));
        }
    };

    if let Err(e) = data::apply_kernel_operations(db, &ops) {
        if let Err(rb_err) = db.conn().execute_batch("ROLLBACK") {
            tracing::error!(error = %rb_err, original_error = %e, "ROLLBACK failed during data operations");
        }
        let _ = db.conn().execute(
            "UPDATE kernel_change_proposals SET status = 'failed', error = ? WHERE id = ?",
            rusqlite::params![e.to_string(), proposal_id],
        );
        return Err(KernelError::Db(e));
    }

    if let Err(e) = manifest::apply_manifest_operations(db, &ops) {
        if let Err(rb_err) = db.conn().execute_batch("ROLLBACK") {
            tracing::error!(error = %rb_err, original_error = %e, "ROLLBACK failed during manifest operations");
        }
        let _ = db.conn().execute(
            "UPDATE kernel_change_proposals SET status = 'failed', error = ? WHERE id = ?",
            rusqlite::params![e.to_string(), proposal_id],
        );
        return Err(KernelError::Db(e));
    }

    let apply_res = apply_transaction_deferred(db, &txn.id, &mut deferred);
    let apply = match apply_res {
        Ok(a) => a,
        Err(e) => {
            if let Err(rb_err) = db.conn().execute_batch("ROLLBACK") {
                tracing::error!(error = %rb_err, original_error = %e, "ROLLBACK failed during deferred apply");
            }
            let _ = db.conn().execute(
                "UPDATE kernel_change_proposals SET status = 'failed', error = ? WHERE id = ?",
                rusqlite::params![e.to_string(), proposal_id],
            );
            return Err(KernelError::Db(e));
        }
    };

    if apply.transaction.status != "applied" || !apply.conflicts.is_empty() {
        let err_msg = format!("Application conflicts: {:?}", apply.conflicts);
        if let Err(rb_err) = db.conn().execute_batch("ROLLBACK") {
            tracing::error!(error = %rb_err, "ROLLBACK failed during application conflicts");
        }
        let _ = db.conn().execute(
            "UPDATE kernel_change_proposals SET status = 'failed', error = ? WHERE id = ?",
            rusqlite::params![err_msg, proposal_id],
        );
        return Ok(ChangeResult {
            apply: None,
            proposal_id: Some(proposal_id.to_string()),
            risk: proposal.risk,
            impact_summary: proposal.impact_summary,
            policy: PolicyDecision {
                decision: "conflicted".into(),
                message: "Conflicts during proposal application".into(),
                action: "apply_operations".into(),
            },
            verification: None,
            operations: None,
            summary: Some(proposal.summary),
            outcome: CommitOutcome::Conflicted,
            conflicts: apply.conflicts,
        });
    }

    // Record provenance and outbox
    let change_req = ChangeRequest {
        proposal_id: Some(proposal_id.to_string()),
        conversation_id: proposal.conversation_id.clone(),
        project_id: None,
        turn_id: proposal.turn_id.clone(),
        summary: proposal.summary.clone(),
        operations: ops.clone(),
        silent: false,
        source_type: "user".into(),
        provider: None,
        model: None,
        require_approval: false,
        approval_granted: true,
    };
    if let Err(e) = record_provenance(db, &txn.id, &change_req, "applied", Some("passed"), None) {
        tracing::warn!(
            transaction_id = %txn.id,
            error = %e,
            "provenance recording failed after proposal apply — audit trail gap"
        );
    }

    let verification = testing::verify_after_change(db, &ops).ok();

    for (i, effect) in deferred.iter().enumerate() {
        if let Err(e) = enqueue_outbox(
            db,
            Some(&txn.id),
            proposal.conversation_id.as_deref(),
            proposal.turn_id.as_deref(),
            None,
            i as i64,
            effect,
        ) {
            if let Err(rb_err) = db.conn().execute_batch("ROLLBACK") {
                tracing::error!(error = %rb_err, original_error = %e, "ROLLBACK failed during outbox enqueue");
            }
            let _ = db.conn().execute(
                "UPDATE kernel_change_proposals SET status = 'failed', error = ? WHERE id = ?",
                rusqlite::params![e.to_string(), proposal_id],
            );
            return Err(KernelError::Db(e));
        }
    }

    // 5. Atomic proposal status update to 'applied' with transaction_id and applied_at
    if let Err(e) = db.conn().execute(
        "UPDATE kernel_change_proposals SET status = 'applied', applied_at = ?, transaction_id = ? WHERE id = ?",
        rusqlite::params![chrono::Utc::now().to_rfc3339(), txn.id, proposal_id],
    ) {
        if let Err(rb_err) = db.conn().execute_batch("ROLLBACK") {
            tracing::error!(error = %rb_err, original_error = %e, "ROLLBACK failed during proposal status update");
        }
        return Err(KernelError::Db(crate::db::DbError::Sqlite(e)));
    }

    // 6. Single COMMIT for all changes!
    db.conn()
        .execute_batch("COMMIT")
        .map_err(|e| KernelError::Db(crate::db::DbError::Sqlite(e)))?;

    // 7. Flush outbox after commit
    if let Some(bus) = bus {
        let _ = flush_pending_outbox(db, Some(bus));
    }

    Ok(ChangeResult {
        apply: Some(apply),
        proposal_id: Some(proposal_id.to_string()),
        risk: proposal.risk,
        impact_summary: proposal.impact_summary,
        policy: PolicyDecision {
            decision: "approved".into(),
            message: "Proposal approved and applied".into(),
            action: "apply_operations".into(),
        },
        verification,
        operations: None,
        summary: Some(proposal.summary),
        outcome: CommitOutcome::Committed,
        conflicts: vec![],
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
                proposal_id: None,
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
                proposal_id: None,
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
            "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}],
            "stateContracts": [
                { "key": "x" }
            ]
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
            proposal_id: None,
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
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "bus-fail", None).unwrap();
        let def = json!({ "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}] });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Inline", &def, &[]).unwrap();

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
                proposal_id: None,
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

    #[test]
    fn proposal_revision_staleness_protection() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "stale-test", None).unwrap();
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Target Surface",
            &json!({ "components": [] }),
            &["coreside.core".into()],
        )
        .unwrap();

        assert_eq!(surface.current_revision, 1);

        // Create a strong operation targeting this surface that requires approval
        let mut delete_op = op("surface.delete");
        delete_op.target.surface_id = Some(surface.id.clone());
        delete_op.requires_approval = Some(true);

        let change_res = apply_change(
            &mut db,
            None,
            ChangeRequest {
                conversation_id: Some(conv.id.clone()),
                turn_id: Some("turn-stale-1".into()),
                summary: "delete proposal".into(),
                operations: vec![delete_op],
                require_approval: true,
                approval_granted: false,
                source_type: "agent".into(),
                ..Default::default()
            },
        )
        .unwrap();

        let proposal_id = change_res
            .proposal_id
            .expect("expected proposal for strong change");
        let proposal = get_proposal(&db, &proposal_id).unwrap();
        assert_eq!(proposal.status, "pending");

        // Now mutate surface definition, which increments its revision to 2
        let updated = crate::runtime_v2::surfaces::update_surface_definition(
            &mut db,
            &surface.id,
            &json!({ "components": [] }),
            "bump revision",
            Some(1),
        )
        .unwrap();
        assert_eq!(updated.current_revision, 2);

        // Attempting to decide/approve the proposal must fail due to revision conflict!
        let err = decide_proposal(&mut db, None, &proposal_id, true).unwrap_err();
        assert!(matches!(err, KernelError::RevisionConflict(_)));

        // Proposal status must authoritatively become stale
        let stale_proposal = get_proposal(&db, &proposal_id).unwrap();
        assert_eq!(stale_proposal.status, "stale");

        // Surface must NOT have been deleted
        let surf = get_surface(&db, &surface.id).unwrap();
        assert_eq!(surf.id, surface.id);
    }

    #[test]
    fn proposal_single_use_cas_and_tamper_protection() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "cas-test", None).unwrap();
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "CAS Surface",
            &json!({ "components": [] }),
            &["coreside.core".into()],
        )
        .unwrap();

        // Create an operation requiring approval
        let mut add_op = op("component.insert");
        add_op.target.surface_id = Some(surface.id.clone());
        add_op.payload = json!({
            "component": { "id": "c_cas", "type": "text", "props": { "text": "CAS approved" } }
        });
        add_op.requires_approval = Some(true);

        let change_res = apply_change(
            &mut db,
            None,
            ChangeRequest {
                conversation_id: Some(conv.id.clone()),
                turn_id: Some("turn-cas-1".into()),
                summary: "insert component".into(),
                operations: vec![add_op],
                require_approval: true,
                approval_granted: false,
                source_type: "agent".into(),
                ..Default::default()
            },
        )
        .unwrap();

        let proposal_id = change_res.proposal_id.expect("proposal expected");

        // Tamper test: modify exact_operations_json directly in the database
        db.conn()
            .execute(
                "UPDATE kernel_change_proposals SET exact_operations_json = '[]' WHERE id = ?1",
                [&proposal_id],
            )
            .unwrap();

        let tamper_err = decide_proposal(&mut db, None, &proposal_id, true).unwrap_err();
        assert!(
            matches!(tamper_err, KernelError::Validation(ref msg) if msg.contains("hash mismatch")),
            "Expected hash mismatch on tampered operations, got: {:?}",
            tamper_err
        );

        // Now create a valid proposal and test single-use CAS
        let mut valid_op = op("component.insert");
        valid_op.target.surface_id = Some(surface.id.clone());
        valid_op.payload = json!({
            "component": { "id": "c_valid", "type": "text", "props": { "text": "Valid insert" } }
        });
        valid_op.requires_approval = Some(true);

        let valid_change = apply_change(
            &mut db,
            None,
            ChangeRequest {
                conversation_id: Some(conv.id.clone()),
                turn_id: Some("turn-cas-2".into()),
                summary: "valid insert".into(),
                operations: vec![valid_op],
                require_approval: true,
                approval_granted: false,
                source_type: "agent".into(),
                ..Default::default()
            },
        )
        .unwrap();

        let valid_proposal_id = valid_change.proposal_id.expect("proposal expected");

        // First approval succeeds
        let applied_res = decide_proposal(&mut db, None, &valid_proposal_id, true).unwrap();
        assert!(applied_res.is_committed());

        let prop = get_proposal(&db, &valid_proposal_id).unwrap();
        assert_eq!(prop.status, "applied");

        // Second approval must fail closed via CAS
        let second_err = decide_proposal(&mut db, None, &valid_proposal_id, true).unwrap_err();
        assert!(matches!(second_err, KernelError::Validation(_)));
    }

    #[test]
    fn test_live_task_manager_proposal_approval_and_application() {
        let mut db = test_db();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Task Manager Conv", None).unwrap();

        let raw_json = r#"[{"id":"operation-1","type":"surface.create","target":{},"payload":{"components":[{"id":"header","props":{"text":"Task Manager"},"type":"heading"},{"id":"task-filter","props":{"label":"Search Tasks","valueKey":"searchQuery"},"type":"textInput"},{"id":"task-priority","props":{"label":"Filter by Priority","options":[{"label":"All","value":""},{"label":"High","value":"high"},{"label":"Medium","value":"medium"},{"label":"Low","value":"low"}],"valueKey":"priorityFilter"},"type":"select"},{"id":"task-status","props":{"label":"Filter by Status","options":[{"label":"All","value":""},{"label":"Completed","value":"completed"},{"label":"Pending","value":"pending"}],"valueKey":"statusFilter"},"type":"select"},{"id":"task-list","props":{"columns":[{"accessor":"title","header":"Task"},{"accessor":"priority","header":"Priority"},{"accessor":"status","header":"Status"},{"accessor":"actions","header":"Actions"}],"dataKey":"tasks","rowsKey":"tasks"},"type":"dataTable"},{"id":"add-task-container","props":{"actions":[{"target":"newTaskTitle","type":"setValue","value":""},{"target":"newTaskPriority","type":"setValue","value":"medium"},{"target":"newTaskStatus","type":"setValue","value":"pending"}],"label":"Add New Task"},"type":"container"},{"id":"newTaskTitle","props":{"label":"Task Title","valueKey":"newTaskTitle"},"type":"textInput"},{"id":"newTaskPriority","props":{"label":"Priority","options":[{"label":"High","value":"high"},{"label":"Medium","value":"medium"},{"label":"Low","value":"low"}],"valueKey":"newTaskPriority"},"type":"select"},{"id":"newTaskStatus","props":{"label":"Status","options":[{"label":"Pending","value":"pending"},{"label":"Completed","value":"completed"}],"valueKey":"newTaskStatus"},"type":"select"},{"id":"add-task-button","props":{"actions":[{"item":{"priority":"newTaskPriority","status":"newTaskStatus","title":"newTaskTitle"},"target":"tasks","type":"appendItem"},{"target":"newTaskTitle","type":"setValue","value":""},{"target":"newTaskPriority","type":"setValue","value":"medium"},{"target":"newTaskStatus","type":"setValue","value":"pending"}],"label":"Add Task"},"type":"button"},{"id":"delete-task-button","props":{"actions":[{"id":"selectedTaskId","target":"tasks","type":"removeItem"}],"label":"Delete Task"},"type":"button"},{"id":"edit-task-button","props":{"actions":[{"id":"selectedTaskId","patch":{"priority":"newTaskPriority","status":"newTaskStatus","title":"newTaskTitle"},"target":"tasks","type":"updateItem"}],"label":"Edit Task"},"type":"button"}],"description":"A tool for managing tasks with persistent storage, priority, status, and various functionalities.","id":"task-manager","layout":{"columns":2,"density":"compact","type":"dashboard"},"name":"Task Manager"}}]"#;

        let raw_ops: Vec<AppOperation> = serde_json::from_str(raw_json).unwrap();
        let normalized_ops = crate::runtime_v2::normalize_and_validate_model_operations(&raw_ops)
            .expect("should normalize and validate");

        let change_res = apply_change(
            &mut db,
            None,
            ChangeRequest {
                conversation_id: Some(conv.id.clone()),
                turn_id: Some("turn-tm-1".into()),
                summary: "Create Task Manager".into(),
                operations: normalized_ops,
                source_type: "agent".into(),
                ..Default::default()
            },
        )
        .expect("apply_change must succeed and create proposal");

        let proposal_id = change_res
            .proposal_id
            .expect("must produce proposal for agent surface creation");
        let apply_res = decide_proposal(&mut db, None, &proposal_id, true)
            .expect("decide_proposal approve must succeed without conflicts");

        assert!(
            apply_res.is_committed(),
            "proposal application must be committed, conflicts: {:?}",
            apply_res.conflicts
        );
        assert!(apply_res.conflicts.is_empty(), "conflicts must be empty");

        let prop = get_proposal(&db, &proposal_id).unwrap();
        assert_eq!(prop.status, "applied");

        let sid = crate::runtime_v2::surfaces::surface_id_for_tool("task-manager");
        let surface = crate::runtime_v2::surfaces::get_surface(&db, &sid).unwrap();
        assert_eq!(surface.name, "Task Manager");
    }
}
