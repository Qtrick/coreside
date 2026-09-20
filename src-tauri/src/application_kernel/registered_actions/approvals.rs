//! Pending approvals for registered actions.
//!
//! Only the user decides an approval. The agent has no path to `decide`, and a
//! decided approval can be consumed exactly once — enforced by a compare-and-set
//! receipt row rather than a read-then-write check.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

use super::canonical::hash_value;
use super::context::ActionRunContext;
use super::descriptor::ActionDescriptor;
use super::grants::{mint_grant, GrantDuration, GrantScope, RuntimeGrant};

pub const APPROVAL_TTL_MINUTES: i64 = 15;
const MAX_INPUT_PREVIEW_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: String,
    pub application_id: Option<String>,
    pub action_name: String,
    pub action_title: String,
    pub risk: String,
    pub critical: bool,
    pub input_preview: String,
    pub call_hash: String,
    pub descriptor_hash: String,
    pub venue: String,
    pub presence: String,
    pub session_id: Option<String>,
    pub run_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub expires_at: String,
    pub decided_at: Option<String>,
    pub consumed_at: Option<String>,
    pub explanation: Option<String>,
    pub surface_id: Option<String>,
    pub component_id: Option<String>,
}

/// Identity of one concrete call: application + action + authority + input.
pub fn call_hash(ctx: &ActionRunContext, descriptor: &ActionDescriptor, input: &Value) -> String {
    hash_value(&serde_json::json!({
        "applicationId": ctx.application_id,
        "action": descriptor.name,
        "descriptorHash": descriptor.descriptor_hash(),
        "input": input,
    }))
}

pub fn input_preview(input: &Value) -> String {
    let raw = crate::security::redact_secrets(&input.to_string(), None);
    raw.chars().take(MAX_INPUT_PREVIEW_CHARS).collect()
}

pub fn create_pending(
    db: &mut Database,
    ctx: &ActionRunContext,
    descriptor: &ActionDescriptor,
    input: &Value,
    explanation: Option<&str>,
) -> DbResult<ApprovalRequest> {
    expire_stale(db)?;
    let hash = call_hash(ctx, descriptor, input);

    // Reuse an existing live pending request for the same call so repeated
    // renders do not pile up duplicate prompts.
    if let Some(existing) = find_live_pending_by_call(db, &hash)? {
        return Ok(existing);
    }

    let id = format!("approval-{}", Uuid::new_v4());
    let now = chrono::Utc::now();
    let expires = now + chrono::Duration::minutes(APPROVAL_TTL_MINUTES);
    let input_json = serde_json::to_string(input)?;
    db.conn().execute(
        "INSERT INTO runtime_approvals (
            id, application_id, action_name, action_title, risk, critical, input_preview,
            input_json, call_hash, descriptor_hash, venue, presence, session_id, run_id, status,
            created_at, expires_at, explanation, surface_id, component_id
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'pending',?15,?16,?17,?18,?19)",
        params![
            id,
            ctx.application_id,
            descriptor.name,
            descriptor.title,
            descriptor.risk.as_str(),
            if descriptor.critical { 1 } else { 0 },
            input_preview(input),
            input_json,
            hash,
            descriptor.descriptor_hash(),
            ctx.venue.as_str(),
            ctx.presence.as_str(),
            ctx.session_id,
            ctx.run_id,
            now.to_rfc3339(),
            expires.to_rfc3339(),
            explanation,
            ctx.surface_id,
            ctx.component_id
        ],
    )?;
    get_approval(db, &id)
}

const APPROVAL_COLS: &str = "id, application_id, action_name, action_title, risk, critical,
    input_preview, call_hash, descriptor_hash, venue, presence, session_id, run_id, status,
    created_at, expires_at, decided_at, consumed_at, explanation, surface_id, component_id";

fn map_approval(row: &rusqlite::Row<'_>) -> rusqlite::Result<ApprovalRequest> {
    Ok(ApprovalRequest {
        id: row.get(0)?,
        application_id: row.get(1)?,
        action_name: row.get(2)?,
        action_title: row.get(3)?,
        risk: row.get(4)?,
        critical: row.get::<_, i64>(5)? != 0,
        input_preview: row.get(6)?,
        call_hash: row.get(7)?,
        descriptor_hash: row.get(8)?,
        venue: row.get(9)?,
        presence: row.get(10)?,
        session_id: row.get(11)?,
        run_id: row.get(12)?,
        status: row.get(13)?,
        created_at: row.get(14)?,
        expires_at: row.get(15)?,
        decided_at: row.get(16)?,
        consumed_at: row.get(17)?,
        explanation: row.get(18)?,
        surface_id: row.get(19)?,
        component_id: row.get(20)?,
    })
}

pub fn get_approval(db: &Database, id: &str) -> DbResult<ApprovalRequest> {
    db.conn()
        .query_row(
            &format!("SELECT {APPROVAL_COLS} FROM runtime_approvals WHERE id = ?1"),
            [id],
            map_approval,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("approval {id}")),
            other => DbError::Sqlite(other),
        })
}

fn find_live_pending_by_call(db: &Database, hash: &str) -> DbResult<Option<ApprovalRequest>> {
    let mut stmt = db.conn().prepare(&format!(
        "SELECT {APPROVAL_COLS} FROM runtime_approvals
         WHERE call_hash = ?1 AND status = 'pending' AND expires_at > ?2
         ORDER BY created_at DESC LIMIT 1"
    ))?;
    let mut rows = stmt.query_map(params![hash, now_rfc3339()], map_approval)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn list_pending(db: &mut Database) -> DbResult<Vec<ApprovalRequest>> {
    expire_stale(db)?;
    let mut stmt = db.conn().prepare(&format!(
        "SELECT {APPROVAL_COLS} FROM runtime_approvals
         WHERE status = 'pending' ORDER BY created_at DESC LIMIT 100"
    ))?;
    let rows = stmt.query_map([], map_approval)?;
    let mut approvals = Vec::new();
    for row in rows {
        approvals.push(row?);
    }
    Ok(approvals)
}

pub fn expire_stale(db: &mut Database) -> DbResult<u64> {
    let n = db.conn().execute(
        "UPDATE runtime_approvals SET status = 'expired'
         WHERE status = 'pending' AND expires_at <= ?1",
        [now_rfc3339()],
    )?;
    Ok(n as u64)
}

#[derive(Debug, Clone)]
pub struct RememberChoice {
    pub scope: GrantScope,
    pub duration: GrantDuration,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalDecisionResult {
    pub approval: ApprovalRequest,
    pub grant: Option<RuntimeGrant>,
    /// Present when the user approved and the trusted path re-ran the frozen call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<super::gateway::ActionOutcome>,
}

/// Frozen input for a pending/approved approval. Trusted callers only.
pub fn frozen_input(db: &Database, id: &str) -> DbResult<Value> {
    let raw: String = db
        .conn()
        .query_row(
            "SELECT input_json FROM runtime_approvals WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("approval {id}")),
            other => DbError::Sqlite(other),
        })?;
    serde_json::from_str(&raw).map_err(DbError::Serde)
}

/// Decide an approval. `actor` must be `user`; the agent can never decide.
/// The decision itself is claimed once via `runtime_approval_claims`.
pub fn decide(
    db: &mut Database,
    id: &str,
    approve: bool,
    remember: Option<RememberChoice>,
    actor: &str,
) -> DbResult<ApprovalDecisionResult> {
    if actor != "user" {
        return Err(DbError::Invalid(
            "only the user can decide an approval".into(),
        ));
    }
    expire_stale(db)?;
    let approval = get_approval(db, id)?;
    if approval.status != "pending" {
        return Err(DbError::Invalid(format!(
            "approval is already {}",
            approval.status
        )));
    }

    // Validate remember/mint inputs *before* claiming the decision so a failed
    // grant does not leave the approval approved-but-unusable.
    let mut prepared_grant: Option<(
        &'static super::descriptor::ActionDescriptor,
        GrantScope,
        GrantDuration,
        Option<String>,
    )> = None;
    if approve {
        if let Some(choice) = remember {
            if choice.duration != GrantDuration::Once {
                let descriptor = super::descriptor::find_action(&approval.action_name)
                    .ok_or_else(|| DbError::Invalid("unknown action".into()))?;
                if descriptor.descriptor_hash() != approval.descriptor_hash {
                    return Err(DbError::Invalid(
                        "this action changed since it was requested; approve it again".into(),
                    ));
                }
                if descriptor.risk == super::descriptor::ActionRisk::Destructive
                    || descriptor.critical
                {
                    return Err(DbError::Invalid(
                        "this action cannot be remembered; approve it once instead".into(),
                    ));
                }
                let input_hash = if choice.scope == GrantScope::Exact {
                    Some(approval.call_hash.clone())
                } else {
                    None
                };
                prepared_grant = Some((descriptor, choice.scope, choice.duration, input_hash));
            }
        }
    }

    // Claim + status + optional grant mint must be atomic: a failed remember
    // must not leave the approval approved-but-unusable.
    db.conn().execute_batch("SAVEPOINT approval_decide")?;
    let decided = (|| -> DbResult<ApprovalDecisionResult> {
        if !claim(db, &format!("decided:{id}"))? {
            return Err(DbError::Invalid("approval was already decided".into()));
        }
        let status = if approve { "approved" } else { "denied" };
        db.conn().execute(
            "UPDATE runtime_approvals SET status = ?2, decided_at = ?3 WHERE id = ?1",
            params![id, status, now_rfc3339()],
        )?;

        let mut grant = None;
        if let Some((descriptor, scope, duration, input_hash)) = prepared_grant {
            let ctx = approval_context(&approval);
            grant = Some(mint_grant(
                db,
                &ctx,
                descriptor,
                scope,
                duration,
                input_hash.as_deref(),
                "user",
            )?);
        }
        Ok(ApprovalDecisionResult {
            approval: get_approval(db, id)?,
            grant,
            outcome: None,
        })
    })();
    match decided {
        Ok(result) => {
            db.conn().execute_batch("RELEASE approval_decide")?;
            Ok(result)
        }
        Err(err) => {
            let _ = db
                .conn()
                .execute_batch("ROLLBACK TO approval_decide; RELEASE approval_decide");
            Err(err)
        }
    }
}

fn approval_context(approval: &ApprovalRequest) -> ActionRunContext {
    use super::context::{Presence, Venue};
    ActionRunContext {
        actor: "user".into(),
        venue: Venue::parse(&approval.venue).unwrap_or(Venue::Application),
        presence: Presence::Present,
        application_id: approval.application_id.clone(),
        project_id: None,
        conversation_id: None,
        session_id: approval
            .session_id
            .clone()
            .unwrap_or_else(|| super::context::session_id().to_string()),
        run_id: approval.run_id.clone().unwrap_or_default(),
        trigger: None,
        surface_id: approval.surface_id.clone(),
        component_id: approval.component_id.clone(),
        depth: 0,
    }
}

/// Consume an approved approval for a specific call. Returns `Ok(true)` for the
/// single caller that wins the compare-and-set; every later attempt gets false.
pub fn consume(db: &mut Database, id: &str, expected_call_hash: &str) -> DbResult<bool> {
    expire_stale(db)?;
    let approval = get_approval(db, id)?;
    if approval.status != "approved" || approval.call_hash != expected_call_hash {
        return Ok(false);
    }
    if !claim(db, &format!("consumed:{id}"))? {
        return Ok(false);
    }
    db.conn().execute(
        "UPDATE runtime_approvals SET status = 'consumed', consumed_at = ?2 WHERE id = ?1",
        params![id, now_rfc3339()],
    )?;
    Ok(true)
}

/// Insert a receipt. `true` means this caller claimed it first.
fn claim(db: &Database, receipt: &str) -> DbResult<bool> {
    let n = db.conn().execute(
        "INSERT OR IGNORE INTO runtime_approval_claims (id) VALUES (?1)",
        [receipt],
    )?;
    Ok(n == 1)
}

pub fn has_live_approvals(db: &Database) -> DbResult<bool> {
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM runtime_approvals WHERE status IN ('pending','approved')",
        [],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_kernel::registered_actions::context::{Presence, Venue};
    use crate::application_kernel::registered_actions::descriptor::find_action;
    use crate::application_kernel::registered_actions::testing::test_db;
    use serde_json::json;

    fn ctx() -> ActionRunContext {
        ActionRunContext {
            actor: "user".into(),
            venue: Venue::Application,
            presence: Presence::Present,
            application_id: Some("app-1".into()),
            project_id: None,
            conversation_id: None,
            session_id: "session-test".into(),
            run_id: "run-test".into(),
            trigger: None,
            surface_id: None,
            component_id: None,
            depth: 0,
        }
    }

    #[test]
    fn agent_cannot_decide() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        assert!(decide(&mut db, &a.id, true, None, "agent").is_err());
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "pending");
    }

    #[test]
    fn approval_consumes_exactly_once() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let input = json!({"modelId": "m"});
        let a = create_pending(&mut db, &ctx(), d, &input, None).unwrap();
        decide(&mut db, &a.id, true, None, "user").unwrap();
        assert!(consume(&mut db, &a.id, &a.call_hash).unwrap());
        assert!(!consume(&mut db, &a.id, &a.call_hash).unwrap());
    }

    #[test]
    fn approval_does_not_authorize_a_different_call() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        decide(&mut db, &a.id, true, None, "user").unwrap();
        assert!(!consume(&mut db, &a.id, "some-other-hash").unwrap());
    }

    #[test]
    fn denied_approval_cannot_be_consumed() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        decide(&mut db, &a.id, false, None, "user").unwrap();
        assert!(!consume(&mut db, &a.id, &a.call_hash).unwrap());
    }

    #[test]
    fn decision_happens_once() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        decide(&mut db, &a.id, true, None, "user").unwrap();
        assert!(decide(&mut db, &a.id, false, None, "user").is_err());
    }

    #[test]
    fn duplicate_pending_is_reused() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let input = json!({"modelId": "m"});
        let a = create_pending(&mut db, &ctx(), d, &input, None).unwrap();
        let b = create_pending(&mut db, &ctx(), d, &input, None).unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(list_pending(&mut db).unwrap().len(), 1);
    }

    #[test]
    fn expired_approval_is_not_pending() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        db.conn()
            .execute(
                "UPDATE runtime_approvals SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
                [&a.id],
            )
            .unwrap();
        assert!(list_pending(&mut db).unwrap().is_empty());
        assert!(decide(&mut db, &a.id, true, None, "user").is_err());
    }

    #[test]
    fn remember_mints_grant() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        let result = decide(
            &mut db,
            &a.id,
            true,
            Some(RememberChoice {
                scope: GrantScope::Action,
                duration: GrantDuration::Session,
            }),
            "user",
        )
        .unwrap();
        assert!(result.grant.is_some());
    }

    #[test]
    fn remember_destructive_is_refused_without_deciding() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.delete").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"recordId": "r1"}), None).unwrap();
        let err = decide(
            &mut db,
            &a.id,
            true,
            Some(RememberChoice {
                scope: GrantScope::Action,
                duration: GrantDuration::Session,
            }),
            "user",
        );
        assert!(err.is_err());
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "pending");
    }

    #[test]
    fn input_preview_is_redacted_and_bounded() {
        let preview = input_preview(&json!({ "note": "x".repeat(1000) }));
        assert!(preview.chars().count() <= MAX_INPUT_PREVIEW_CHARS);
        let secret = input_preview(&json!({ "key": "AIzaSyA1234567890abcdefghijklmno" }));
        assert!(!secret.contains("AIzaSyA1234567890abcdefghijklmno"));
    }
}
