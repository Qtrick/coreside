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

/// Lifetime of a pending (undecided) approval from creation.
pub const APPROVAL_TTL_MINUTES: i64 = 15;
/// Lifetime of an approved-but-unused approval from the decision instant.
/// Prevents an indefinitely valid consume token after the user walks away.
pub const APPROVED_UNUSED_TTL_MINUTES: i64 = 15;
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
    // Pending and approved-but-unused both honor expires_at. Denied/consumed/
    // already-expired rows are left alone.
    let n = db.conn().execute(
        "UPDATE runtime_approvals SET status = 'expired'
         WHERE status IN ('pending', 'approved') AND expires_at <= ?1",
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
        let decided_now = chrono::Utc::now();
        let decided_at = decided_now.to_rfc3339();
        // Approving starts a fresh unused window from the decision instant so a
        // user who decides near the end of the pending TTL still has time to
        // finish the trusted re-run. Denied rows keep their original expires_at.
        let n = if approve {
            let unused_expires = (decided_now
                + chrono::Duration::minutes(APPROVED_UNUSED_TTL_MINUTES))
            .to_rfc3339();
            db.conn().execute(
                "UPDATE runtime_approvals
                 SET status = ?2, decided_at = ?3, expires_at = ?4
                 WHERE id = ?1 AND status = 'pending'",
                params![id, status, decided_at, unused_expires],
            )?
        } else {
            db.conn().execute(
                "UPDATE runtime_approvals SET status = ?2, decided_at = ?3
                 WHERE id = ?1 AND status = 'pending'",
                params![id, status, decided_at],
            )?
        };
        if n != 1 {
            return Err(DbError::Invalid(
                "approval is no longer pending and cannot be decided".into(),
            ));
        }

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
///
/// `expire_stale` runs first so approved-but-unused past `expires_at` cannot be
/// consumed (status flips to `expired` before the approved check). The row
/// UPDATE also requires `expires_at > now` so a TTL that elapses between the
/// sweep and the CAS still loses.
///
/// Claim receipt + status update are one atomic transition: a failed update
/// must not leave `consumed:<id>` claimed while the row stays `approved`.
pub fn consume(db: &mut Database, id: &str, expected_call_hash: &str) -> DbResult<bool> {
    expire_stale(db)?;
    db.conn().execute_batch("SAVEPOINT approval_consume")?;
    let consumed = (|| -> DbResult<bool> {
        let approval = get_approval(db, id)?;
        if approval.status != "approved" || approval.call_hash != expected_call_hash {
            return Ok(false);
        }
        if !claim(db, &format!("consumed:{id}"))? {
            return Ok(false);
        }
        let now = now_rfc3339();
        let n = db.conn().execute(
            "UPDATE runtime_approvals SET status = 'consumed', consumed_at = ?2
             WHERE id = ?1 AND status = 'approved' AND call_hash = ?3 AND expires_at > ?4",
            params![id, now, expected_call_hash, now],
        )?;
        if n != 1 {
            // Lost the row-level CAS after winning the claim receipt. Roll back via
            // Err so the claim does not stick; mapped to Ok(false) below.
            return Err(DbError::Invalid(
                "approval consume cas conflict: approved row changed under claim".into(),
            ));
        }
        Ok(true)
    })();
    match consumed {
        Ok(result) => {
            db.conn().execute_batch("RELEASE approval_consume")?;
            Ok(result)
        }
        Err(err) => {
            let _ = db
                .conn()
                .execute_batch("ROLLBACK TO approval_consume; RELEASE approval_consume");
            // CAS lose must block the action without looking like storage failure
            // (gateway maps Ok(false) → approval_invalid).
            if matches!(&err, DbError::Invalid(msg) if msg.contains("cas conflict")) {
                Ok(false)
            } else {
                Err(err)
            }
        }
    }
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
        "SELECT COUNT(*) FROM runtime_approvals
         WHERE status IN ('pending','approved') AND expires_at > ?1",
        [now_rfc3339()],
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
    fn decide_rejects_when_already_expired_before_claim() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        db.conn()
            .execute(
                "UPDATE runtime_approvals SET status = 'expired' WHERE id = ?1",
                [&a.id],
            )
            .unwrap();
        let err = decide(&mut db, &a.id, true, None, "user").unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "expired");
        let claims: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM runtime_approval_claims WHERE id = ?1",
                [format!("decided:{}", a.id)],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(claims, 0, "early reject must not leave a decide claim");
    }

    #[test]
    fn decide_cas_rejects_when_status_flips_after_pending_read() {
        // Real CAS window: status is still pending at get_approval, then a
        // concurrent expire wins between claim INSERT and the status UPDATE.
        // Mirror consume_cas_conflict — flip under the claim INSERT trigger.
        //
        // Same-connection TEMP TRIGGER UPDATE is rolled back with the decide
        // savepoint (as with consume_cas_conflict); assert the decision did
        // not stick and the claim receipt did not leak.
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();

        db.conn()
            .execute_batch(
                "CREATE TEMP TRIGGER steal_pending_on_decide_claim
                 BEFORE INSERT ON runtime_approval_claims
                 WHEN NEW.id LIKE 'decided:%'
                 BEGIN
                   UPDATE runtime_approvals SET status = 'expired'
                   WHERE status = 'pending';
                 END;",
            )
            .unwrap();

        let err = decide(&mut db, &a.id, true, None, "user").unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
        let after = get_approval(&db, &a.id).unwrap();
        assert_ne!(
            after.status, "approved",
            "CAS lose must not leave the approval approved"
        );
        assert!(after.decided_at.is_none());
        let claims: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM runtime_approval_claims WHERE id = ?1",
                [format!("decided:{}", a.id)],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            claims, 0,
            "CAS lose must roll back the decided: claim receipt"
        );

        db.conn()
            .execute_batch("DROP TRIGGER steal_pending_on_decide_claim")
            .unwrap();
    }

    #[test]
    fn decide_rejects_when_a_concurrent_expire_stale_sweep_wins_the_race() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();

        db.conn()
            .execute(
                "UPDATE runtime_approvals SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
                [&a.id],
            )
            .unwrap();

        let swept = expire_stale(&mut db).unwrap();
        assert_eq!(swept, 1);
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "expired");

        let err = decide(&mut db, &a.id, true, None, "user").unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "expired");
        assert!(get_approval(&db, &a.id).unwrap().decided_at.is_none());
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

    #[test]
    fn consume_rolls_back_claim_when_status_update_aborts() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        decide(&mut db, &a.id, true, None, "user").unwrap();

        // Force the status UPDATE to abort after the claim INSERT would succeed.
        db.conn()
            .execute_batch(
                "CREATE TEMP TRIGGER abort_consume_update
                 BEFORE UPDATE OF status ON runtime_approvals
                 WHEN NEW.status = 'consumed'
                 BEGIN
                   SELECT RAISE(ABORT, 'forced consume failure');
                 END;",
            )
            .unwrap();

        let err = consume(&mut db, &a.id, &a.call_hash);
        assert!(err.is_err(), "forced update abort must surface as Err");

        // Approval must remain consumable — claim receipt must not outlive the row.
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "approved");
        let claim_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM runtime_approval_claims WHERE id = ?1",
                [format!("consumed:{}", a.id)],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(claim_count, 0, "failed consume must roll back claim");

        db.conn()
            .execute_batch("DROP TRIGGER abort_consume_update")
            .unwrap();
        assert!(consume(&mut db, &a.id, &a.call_hash).unwrap());
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "consumed");
    }

    #[test]
    fn consume_cas_conflict_returns_false_and_keeps_approval_consumable() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        decide(&mut db, &a.id, true, None, "user").unwrap();

        // Simulate a lost row-level CAS: claim would succeed, but the approved
        // row is flipped under us so UPDATE matches 0 rows.
        db.conn()
            .execute_batch(
                "CREATE TEMP TRIGGER steal_approved_row
                 BEFORE INSERT ON runtime_approval_claims
                 WHEN NEW.id LIKE 'consumed:%'
                 BEGIN
                   UPDATE runtime_approvals SET status = 'expired'
                   WHERE status = 'approved';
                 END;",
            )
            .unwrap();

        assert!(
            !consume(&mut db, &a.id, &a.call_hash).unwrap(),
            "CAS lose must be Ok(false), not storage Err"
        );
        let claim_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM runtime_approval_claims WHERE id = ?1",
                [format!("consumed:{}", a.id)],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(claim_count, 0, "CAS lose must roll back claim receipt");
    }

    #[test]
    fn consume_rejects_after_descriptor_row_still_approved_hash_mismatch() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        decide(&mut db, &a.id, true, None, "user").unwrap();
        assert!(!consume(&mut db, &a.id, "wrong-hash").unwrap());
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "approved");
        assert!(consume(&mut db, &a.id, &a.call_hash).unwrap());
    }

    #[test]
    fn concurrent_consume_race_only_allows_single_winner() {
        let (mut db, dir) = test_db();
        let db_path = dir.path().join("registered-actions.db");
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        decide(&mut db, &a.id, true, None, "user").unwrap();

        let approval_id = a.id.clone();
        let call_hash = a.call_hash.clone();
        let num_threads = 8;
        let mut handles = Vec::new();

        for _ in 0..num_threads {
            let path = db_path.clone();
            let aid = approval_id.clone();
            let chash = call_hash.clone();
            handles.push(std::thread::spawn(move || {
                let mut thread_db = Database::open_path(&path).expect("open db in thread");
                consume(&mut thread_db, &aid, &chash).unwrap_or(false)
            }));
        }

        let mut winners = 0;
        for handle in handles {
            if handle.join().expect("thread join") {
                winners += 1;
            }
        }

        assert_eq!(winners, 1, "Exactly one thread must win the consume race");
        assert_eq!(get_approval(&db, &approval_id).unwrap().status, "consumed");
    }

    #[test]
    fn concurrent_decide_race_only_allows_single_winner() {
        let (mut db, dir) = test_db();
        let db_path = dir.path().join("registered-actions.db");
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();

        let approval_id = a.id.clone();
        let num_threads = 8;
        let mut handles = Vec::new();

        for i in 0..num_threads {
            let path = db_path.clone();
            let aid = approval_id.clone();
            let approve = i % 2 == 0;
            handles.push(std::thread::spawn(move || {
                let mut thread_db = Database::open_path(&path).expect("open db in thread");
                decide(&mut thread_db, &aid, approve, None, "user").is_ok()
            }));
        }

        let mut successes = 0;
        for handle in handles {
            if handle.join().expect("thread join") {
                successes += 1;
            }
        }

        assert_eq!(successes, 1, "Exactly one thread must successfully decide");
        let final_status = get_approval(&db, &approval_id).unwrap().status;
        assert!(final_status == "approved" || final_status == "denied");
    }

    #[test]
    fn approved_but_unused_expires_and_cannot_be_consumed() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        decide(&mut db, &a.id, true, None, "user").unwrap();
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "approved");

        db.conn()
            .execute(
                "UPDATE runtime_approvals SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
                [&a.id],
            )
            .unwrap();

        let swept = expire_stale(&mut db).unwrap();
        assert!(swept >= 1);
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "expired");
        assert!(
            !consume(&mut db, &a.id, &a.call_hash).unwrap(),
            "expired approved approval must not be consumable"
        );
    }

    #[test]
    fn approving_refreshes_unused_expires_at_window() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        // Force pending expiry into the past without sweeping yet.
        db.conn()
            .execute(
                "UPDATE runtime_approvals SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
                [&a.id],
            )
            .unwrap();
        // decide calls expire_stale first — pending past TTL cannot be decided.
        assert!(decide(&mut db, &a.id, true, None, "user").is_err());

        let b = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m2"}), None).unwrap();
        decide(&mut db, &b.id, true, None, "user").unwrap();
        let after = get_approval(&db, &b.id).unwrap();
        assert_eq!(after.status, "approved");
        let decided = after.decided_at.as_deref().expect("decided_at set");
        assert!(
            after.expires_at.as_str() > decided,
            "approve must set expires_at after decided_at for the unused window"
        );
        assert!(consume(&mut db, &b.id, &b.call_hash).unwrap());
    }

    #[test]
    fn has_live_approvals_ignores_rows_past_expires_at() {
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let pending = create_pending(&mut db, &ctx(), d, &json!({"modelId": "p"}), None).unwrap();
        assert!(has_live_approvals(&db).unwrap());

        db.conn()
            .execute(
                "UPDATE runtime_approvals SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
                [&pending.id],
            )
            .unwrap();
        // Status still pending — filter is expires_at, not a prior expire_stale sweep.
        assert_eq!(get_approval(&db, &pending.id).unwrap().status, "pending");
        assert!(
            !has_live_approvals(&db).unwrap(),
            "pending past expires_at must not count as live"
        );

        let approved = create_pending(&mut db, &ctx(), d, &json!({"modelId": "a"}), None).unwrap();
        decide(&mut db, &approved.id, true, None, "user").unwrap();
        assert!(has_live_approvals(&db).unwrap());

        db.conn()
            .execute(
                "UPDATE runtime_approvals SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
                [&approved.id],
            )
            .unwrap();
        assert_eq!(get_approval(&db, &approved.id).unwrap().status, "approved");
        assert!(
            !has_live_approvals(&db).unwrap(),
            "approved-but-unused past expires_at must not count as live"
        );
    }

    #[test]
    fn consume_rejects_when_expires_at_elapses_between_sweep_and_cas() {
        // expire_stale saw a still-valid approved row; TTL then elapses before the
        // row UPDATE. The expires_at > now CAS must lose without leaving a claim.
        let (mut db, _dir) = test_db();
        let d = find_action("local_data.write").unwrap();
        let a = create_pending(&mut db, &ctx(), d, &json!({"modelId": "m"}), None).unwrap();
        decide(&mut db, &a.id, true, None, "user").unwrap();

        db.conn()
            .execute_batch(
                "CREATE TEMP TRIGGER elapse_approved_ttl_on_consume_claim
                 BEFORE INSERT ON runtime_approval_claims
                 WHEN NEW.id LIKE 'consumed:%'
                 BEGIN
                   UPDATE runtime_approvals
                   SET expires_at = '2000-01-01T00:00:00Z'
                   WHERE status = 'approved';
                 END;",
            )
            .unwrap();

        assert!(
            !consume(&mut db, &a.id, &a.call_hash).unwrap(),
            "TTL race must be Ok(false), not a consumable win"
        );
        let claim_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM runtime_approval_claims WHERE id = ?1",
                [format!("consumed:{}", a.id)],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(claim_count, 0, "TTL CAS lose must roll back claim receipt");
        // Trigger UPDATE is rolled back with the consume savepoint; row stays approved.
        assert_eq!(get_approval(&db, &a.id).unwrap().status, "approved");

        db.conn()
            .execute_batch("DROP TRIGGER elapse_approved_ttl_on_consume_claim")
            .unwrap();
        assert!(consume(&mut db, &a.id, &a.call_hash).unwrap());
    }
}
