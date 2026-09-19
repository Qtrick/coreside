//! Durable atomic turn journal — provider work stays outside SQLite transactions.
//!
//! Legal transitions (attempt-scoped):
//!   created → claimed → reserved → provider_started → streaming
//!     → typed_terminal → finalizing → committed → published
//!   any pre-committed → failed | interrupted_recoverable
//!
//! Delayed callbacks from an older attempt_id must not settle a newer attempt.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    Created,
    Claimed,
    Reserved,
    ProviderStarted,
    Streaming,
    TypedTerminal,
    Finalizing,
    Committed,
    Published,
    Failed,
    InterruptedRecoverable,
}

impl TurnState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Claimed => "claimed",
            Self::Reserved => "reserved",
            Self::ProviderStarted => "provider_started",
            Self::Streaming => "streaming",
            Self::TypedTerminal => "typed_terminal",
            Self::Finalizing => "finalizing",
            Self::Committed => "committed",
            Self::Published => "published",
            Self::Failed => "failed",
            Self::InterruptedRecoverable => "interrupted_recoverable",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "created" => Self::Created,
            "claimed" => Self::Claimed,
            "reserved" => Self::Reserved,
            "provider_started" => Self::ProviderStarted,
            "streaming" => Self::Streaming,
            "typed_terminal" => Self::TypedTerminal,
            "finalizing" => Self::Finalizing,
            "committed" => Self::Committed,
            "published" => Self::Published,
            "failed" => Self::Failed,
            "interrupted_recoverable" => Self::InterruptedRecoverable,
            _ => return None,
        })
    }

    fn can_transition(self, next: Self) -> bool {
        use TurnState::*;
        matches!(
            (self, next),
            (Created, Claimed)
                | (Claimed, Reserved)
                | (Claimed, ProviderStarted) // non-hosted
                | (Reserved, ProviderStarted)
                | (ProviderStarted, Streaming)
                | (Streaming, TypedTerminal)
                | (TypedTerminal, Finalizing)
                | (Finalizing, Committed)
                | (Committed, Published)
                | (
                    Created | Claimed | Reserved | ProviderStarted | Streaming | TypedTerminal
                        | Finalizing,
                    Failed
                )
                | (
                    Created | Claimed | Reserved | ProviderStarted | Streaming | TypedTerminal
                        | Finalizing,
                    InterruptedRecoverable
                ) // Failed|InterruptedRecoverable → Claimed requires begin_retry_attempt
                  // (rotates attempt_id). Do not allow transition_turn to reclaim without rotation.
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnJournalRecord {
    pub id: String,
    pub conversation_id: String,
    pub project_id: Option<String>,
    pub attempt_id: String,
    pub idempotency_key: String,
    pub state: TurnState,
    pub route: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub reservation_id: Option<String>,
    pub provisional_text: Option<String>,
    pub operations_json: Option<String>,
    pub error_category: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub finalized_at: Option<String>,
}

pub fn create_turn(
    db: &Database,
    conversation_id: &str,
    project_id: Option<&str>,
    idempotency_key: &str,
    route: Option<&str>,
) -> DbResult<TurnJournalRecord> {
    // Idempotent create: scoped to conversation.
    if let Ok(existing) = get_turn_by_idempotency(db, conversation_id, idempotency_key) {
        return Ok(existing);
    }
    let id = format!("turn-{}", Uuid::new_v4());
    let attempt_id = format!("attempt-{}", Uuid::new_v4());
    let now = now_rfc3339();
    match db.conn().execute(
        "INSERT INTO turn_journal (
            id, conversation_id, project_id, attempt_id, idempotency_key, state,
            route, created_at, updated_at
         ) VALUES (?1,?2,?3,?4,?5,'created',?6,?7,?7)",
        params![
            id,
            conversation_id,
            project_id,
            attempt_id,
            idempotency_key,
            route,
            now
        ],
    ) {
        Ok(_) => get_turn(db, &id),
        Err(rusqlite::Error::SqliteFailure(err, _))
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            // Concurrent create with the same conversation-scoped key.
            get_turn_by_idempotency(db, conversation_id, idempotency_key)
        }
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

pub fn get_turn(db: &Database, id: &str) -> DbResult<TurnJournalRecord> {
    db.conn()
        .query_row(
            "SELECT id, conversation_id, project_id, attempt_id, idempotency_key, state,
                    route, provider, model, reservation_id, provisional_text, operations_json,
                    error_category, error_message, created_at, updated_at, finalized_at
             FROM turn_journal WHERE id = ?1",
            [id],
            map_turn_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("turn {id}")),
            other => DbError::Sqlite(other),
        })
}

pub fn get_turn_by_idempotency(
    db: &Database,
    conversation_id: &str,
    key: &str,
) -> DbResult<TurnJournalRecord> {
    db.conn()
        .query_row(
            "SELECT id, conversation_id, project_id, attempt_id, idempotency_key, state,
                    route, provider, model, reservation_id, provisional_text, operations_json,
                    error_category, error_message, created_at, updated_at, finalized_at
             FROM turn_journal WHERE conversation_id = ?1 AND idempotency_key = ?2",
            params![conversation_id, key],
            map_turn_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("turn idempotency {conversation_id}/{key}"))
            }
            other => DbError::Sqlite(other),
        })
}

fn map_turn_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TurnJournalRecord> {
    let state_s: String = row.get(5)?;
    Ok(TurnJournalRecord {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        project_id: row.get(2)?,
        attempt_id: row.get(3)?,
        idempotency_key: row.get(4)?,
        state: TurnState::parse(&state_s).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown turn_journal state: {state_s}"),
                )),
            )
        })?,
        route: row.get(6)?,
        provider: row.get(7)?,
        model: row.get(8)?,
        reservation_id: row.get(9)?,
        provisional_text: row.get(10)?,
        operations_json: row.get(11)?,
        error_category: row.get(12)?,
        error_message: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        finalized_at: row.get(16)?,
    })
}

/// Transition only if the caller owns `attempt_id`, expected prior state matches,
/// and the transition is legal (compare-and-swap).
pub fn transition_turn(
    db: &Database,
    turn_id: &str,
    attempt_id: &str,
    next: TurnState,
    patch: TurnPatch,
) -> DbResult<TurnJournalRecord> {
    let current = get_turn(db, turn_id)?;
    if current.attempt_id != attempt_id {
        return Err(DbError::Invalid(
            "stale_attempt: delayed callback cannot settle a newer attempt".into(),
        ));
    }
    if !current.state.can_transition(next) {
        return Err(DbError::Invalid(format!(
            "illegal turn transition {:?} -> {:?}",
            current.state, next
        )));
    }
    let expected_state = current.state;
    let now = now_rfc3339();
    let is_terminal = matches!(
        next,
        TurnState::Committed | TurnState::Published | TurnState::Failed
    );
    let n = db.conn().execute(
        "UPDATE turn_journal SET
            state = ?1,
            updated_at = ?2,
            finalized_at = CASE
              WHEN ?3 THEN COALESCE(?4, finalized_at)
              ELSE finalized_at
            END,
            reservation_id = COALESCE(?5, reservation_id),
            provisional_text = COALESCE(?6, provisional_text),
            operations_json = COALESCE(?7, operations_json),
            provider = COALESCE(?8, provider),
            model = COALESCE(?9, model),
            error_category = COALESCE(?10, error_category),
            error_message = COALESCE(?11, error_message)
         WHERE id = ?12 AND attempt_id = ?13 AND state = ?14",
        params![
            next.as_str(),
            now,
            is_terminal,
            if is_terminal {
                Some(now.as_str())
            } else {
                None
            },
            patch.reservation_id,
            patch.provisional_text,
            patch.operations_json,
            patch.provider,
            patch.model,
            patch.error_category,
            patch.error_message,
            turn_id,
            attempt_id,
            expected_state.as_str(),
        ],
    )?;
    if n != 1 {
        return Err(DbError::Invalid(
            "turn_cas_conflict: expected prior state/attempt no longer matches".into(),
        ));
    }
    get_turn(db, turn_id)
}

/// Rotate attempt identity for a genuine retry. Clears terminal timestamps.
pub fn begin_retry_attempt(
    db: &Database,
    turn_id: &str,
    prior_attempt_id: &str,
) -> DbResult<TurnJournalRecord> {
    let current = get_turn(db, turn_id)?;
    if current.attempt_id != prior_attempt_id {
        return Err(DbError::Invalid(
            "stale_attempt: cannot retry from an older attempt".into(),
        ));
    }
    if !matches!(
        current.state,
        TurnState::Failed | TurnState::InterruptedRecoverable
    ) {
        return Err(DbError::Invalid(format!(
            "retry only from failed|interrupted_recoverable, got {:?}",
            current.state
        )));
    }
    let new_attempt = format!("attempt-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE turn_journal SET
            attempt_id = ?1,
            state = 'claimed',
            updated_at = ?2,
            finalized_at = NULL,
            error_category = NULL,
            error_message = NULL
         WHERE id = ?3 AND attempt_id = ?4 AND state = ?5",
        params![
            new_attempt,
            now,
            turn_id,
            prior_attempt_id,
            current.state.as_str()
        ],
    )?;
    if n != 1 {
        return Err(DbError::Invalid(
            "turn_cas_conflict: retry attempt rotation failed".into(),
        ));
    }
    get_turn(db, turn_id)
}

#[derive(Debug, Default, Clone)]
pub struct TurnPatch {
    pub reservation_id: Option<String>,
    pub provisional_text: Option<String>,
    pub operations_json: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub error_category: Option<String>,
    pub error_message: Option<String>,
}

/// Mark interrupted in-flight turns after restart for recovery.
pub fn mark_interrupted_in_flight(db: &Database) -> DbResult<usize> {
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE turn_journal SET state = 'interrupted_recoverable', updated_at = ?1
         WHERE state IN (
            'created','claimed','reserved','provider_started','streaming',
            'typed_terminal','finalizing'
         )",
        params![now],
    )?;
    Ok(n)
}

pub fn list_recoverable_turns(db: &Database, limit: usize) -> DbResult<Vec<TurnJournalRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id FROM turn_journal WHERE state = 'interrupted_recoverable'
         ORDER BY updated_at ASC LIMIT ?1",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![limit as i64], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter().map(|id| get_turn(db, &id)).collect()
}

pub fn list_conversation_recoverable_turns(
    db: &Database,
    conversation_id: &str,
    limit: usize,
) -> DbResult<Vec<TurnJournalRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id FROM turn_journal WHERE conversation_id = ?1 AND state = 'interrupted_recoverable'
         ORDER BY updated_at ASC LIMIT ?2",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![conversation_id, limit as i64], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter().map(|id| get_turn(db, &id)).collect()
}

/// Compact old finalized or failed turns in the journal.
///
/// Retention & compaction eligibility rules:
/// - Active/in-flight states (`created`, `claimed`, `reserved`, `provider_started`, `streaming`, `typed_terminal`, `finalizing`) are NEVER deleted.
/// - `interrupted_recoverable` states are NEVER deleted (they await recovery/retry).
/// - Finalized successful states (`committed`, `published`) are pruned only if `finalized_at` is older than `finalized_retention_days`.
/// - Terminal failed states (`failed`) are pruned only if `finalized_at` is older than `failed_retention_days`.
///
/// Executes inside an unchecked transaction to ensure atomicity.
pub fn compact_turn_journal(
    db: &Database,
    finalized_retention_days: u32,
    failed_retention_days: u32,
) -> DbResult<usize> {
    let tx = db.conn().unchecked_transaction()?;
    let deleted = tx.execute(
        "DELETE FROM turn_journal
         WHERE (
             state IN ('committed', 'published')
             AND finalized_at IS NOT NULL
             AND datetime(finalized_at) <= datetime('now', '-' || ?1 || ' days')
         ) OR (
             state = 'failed'
             AND finalized_at IS NOT NULL
             AND datetime(finalized_at) <= datetime('now', '-' || ?2 || ' days')
         )",
        params![
            finalized_retention_days as i64,
            failed_retention_days as i64
        ],
    )?;
    tx.commit()?;
    Ok(deleted)
}

/// Append a conversation-scoped event after durable commit (cursor resume).
/// Sequence allocation uses a dedicated counter row — not `MAX(sequence)+1`.
pub fn append_conversation_event(
    db: &Database,
    conversation_id: &str,
    turn_id: Option<&str>,
    attempt_id: Option<&str>,
    event_type: &str,
    payload: &Value,
) -> DbResult<(String, i64)> {
    let id = format!("cev-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let tx = db.conn().unchecked_transaction()?;
    // Ensure counter row exists, then atomically increment.
    tx.execute(
        "INSERT OR IGNORE INTO conversation_event_sequences (conversation_id, next_sequence)
         VALUES (?1, 1)",
        [conversation_id],
    )?;
    let seq: i64 = tx.query_row(
        "UPDATE conversation_event_sequences
         SET next_sequence = next_sequence + 1
         WHERE conversation_id = ?1
         RETURNING next_sequence - 1",
        [conversation_id],
        |r| r.get(0),
    )?;
    tx.execute(
        "INSERT INTO conversation_event_log (
            id, conversation_id, sequence, turn_id, attempt_id, event_type, payload_json, created_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            id,
            conversation_id,
            seq,
            turn_id,
            attempt_id,
            event_type,
            payload.to_string(),
            now
        ],
    )?;
    tx.commit()?;
    Ok((id, seq))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationEventRecord {
    pub id: String,
    pub conversation_id: String,
    pub sequence: i64,
    pub turn_id: Option<String>,
    pub attempt_id: Option<String>,
    pub event_type: String,
    pub payload: Value,
    pub created_at: String,
}

pub fn get_conversation_events(
    db: &Database,
    conversation_id: &str,
    after_sequence: Option<i64>,
    limit: Option<i64>,
) -> DbResult<Vec<ConversationEventRecord>> {
    let after_seq = after_sequence.unwrap_or(0);
    let lim = limit.unwrap_or(100).clamp(1, 500);
    let mut stmt = db.conn().prepare(
        "SELECT id, conversation_id, sequence, turn_id, attempt_id, event_type, payload_json, created_at
         FROM conversation_event_log
         WHERE conversation_id = ?1 AND sequence > ?2
         ORDER BY sequence ASC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![conversation_id, after_seq, lim], |r| {
        let payload_json: String = r.get(6)?;
        let payload = serde_json::from_str(&payload_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(ConversationEventRecord {
            id: r.get(0)?,
            conversation_id: r.get(1)?,
            sequence: r.get(2)?,
            turn_id: r.get(3)?,
            attempt_id: r.get(4)?,
            event_type: r.get(5)?,
            payload,
            created_at: r.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn db() -> Database {
        let dir = tempdir().unwrap();
        Database::open_path(&dir.path().join("t.db")).unwrap()
    }

    #[test]
    fn rejects_illegal_and_stale_attempt() {
        let db = db();
        let turn = create_turn(&db, "c1", None, "idem-1", Some("hosted")).unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        assert!(transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Committed,
            TurnPatch::default(),
        )
        .is_err());
        assert!(transition_turn(
            &db,
            &turn.id,
            "attempt-old",
            TurnState::Reserved,
            TurnPatch::default(),
        )
        .is_err());
    }

    #[test]
    fn cas_rejects_stale_prior_state() {
        let db = db();
        let turn = create_turn(&db, "c1", None, "idem-cas", Some("hosted")).unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        // Concurrent CAS: same attempt but state already moved — second Claimed fails.
        assert!(transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .is_err());
    }

    #[test]
    fn retry_rotates_attempt_and_clears_finalized() {
        let db = db();
        let turn = create_turn(&db, "c1", None, "idem-retry", None).unwrap();
        let prior = turn.attempt_id.clone();
        transition_turn(
            &db,
            &turn.id,
            &prior,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &prior,
            TurnState::Failed,
            TurnPatch {
                error_category: Some("provider".into()),
                error_message: Some("boom".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let failed = get_turn(&db, &turn.id).unwrap();
        assert!(failed.finalized_at.is_some());
        let retried = begin_retry_attempt(&db, &turn.id, &prior).unwrap();
        assert_ne!(retried.attempt_id, prior);
        assert_eq!(retried.state, TurnState::Claimed);
        assert!(retried.finalized_at.is_none());
        // Stale callback from prior attempt must not settle.
        assert!(transition_turn(
            &db,
            &turn.id,
            &prior,
            TurnState::Streaming,
            TurnPatch::default(),
        )
        .is_err());
        // Reclaim without attempt rotation is forbidden.
        let failed_again = {
            let t = create_turn(&db, "c1", None, "idem-retry-2", None).unwrap();
            let a = t.attempt_id.clone();
            transition_turn(&db, &t.id, &a, TurnState::Claimed, TurnPatch::default()).unwrap();
            transition_turn(
                &db,
                &t.id,
                &a,
                TurnState::Failed,
                TurnPatch {
                    error_category: Some("provider".into()),
                    error_message: Some("boom".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            (t.id, a)
        };
        assert!(transition_turn(
            &db,
            &failed_again.0,
            &failed_again.1,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .is_err());
    }

    #[test]
    fn idempotency_is_conversation_scoped() {
        let db = db();
        let a = create_turn(&db, "c-a", None, "same-key", None).unwrap();
        let b = create_turn(&db, "c-b", None, "same-key", None).unwrap();
        assert_ne!(a.id, b.id);
        let a2 = create_turn(&db, "c-a", None, "same-key", None).unwrap();
        assert_eq!(a.id, a2.id);
    }

    #[test]
    fn crash_markers_recoverable() {
        let db = db();
        let turn = create_turn(&db, "c1", None, "idem-2", None).unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        let n = mark_interrupted_in_flight(&db).unwrap();
        assert!(n >= 1);
        let list = list_recoverable_turns(&db, 10).unwrap();
        assert!(list.iter().any(|t| t.id == turn.id));
    }

    #[test]
    fn idempotent_create_returns_same_turn() {
        let db = db();
        let a = create_turn(&db, "c1", None, "same-key", None).unwrap();
        let b = create_turn(&db, "c1", None, "same-key", None).unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.attempt_id, b.attempt_id);
    }

    #[test]
    fn compact_turn_journal_retention_rules() {
        let db = db();

        // 1. In-flight turn (claimed) - should NEVER be deleted
        let inflight = create_turn(&db, "c1", None, "idem-inflight", None).unwrap();
        transition_turn(
            &db,
            &inflight.id,
            &inflight.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();

        // 2. Interrupted recoverable - should NEVER be deleted
        let interrupted = create_turn(&db, "c1", None, "idem-interrupted", None).unwrap();
        transition_turn(
            &db,
            &interrupted.id,
            &interrupted.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        mark_interrupted_in_flight(&db).unwrap();

        // 3. Recent committed turn - finalized now, retention 30 days -> preserved
        let recent_committed = create_turn(&db, "c1", None, "idem-recent-com", None).unwrap();
        let now_str = now_rfc3339();
        db.conn()
            .execute(
                "UPDATE turn_journal SET state = 'committed', finalized_at = ?1 WHERE id = ?2",
                [&now_str, &recent_committed.id],
            )
            .unwrap();

        // 4. Old committed turn - backdate finalized_at to 2020 -> pruned
        let old_committed = create_turn(&db, "c1", None, "idem-old-com", None).unwrap();
        db.conn()
            .execute(
                "UPDATE turn_journal SET state = 'committed', finalized_at = '2020-01-01T00:00:00Z' WHERE id = ?1",
                [&old_committed.id],
            )
            .unwrap();

        // 5. Old failed turn - backdate to 2020 -> pruned
        let old_failed = create_turn(&db, "c1", None, "idem-old-fail", None).unwrap();
        db.conn()
            .execute(
                "UPDATE turn_journal SET state = 'failed', finalized_at = '2020-01-01T00:00:00Z' WHERE id = ?1",
                [&old_failed.id],
            )
            .unwrap();

        // Run compaction: 30 days for committed, 14 days for failed
        let pruned = compact_turn_journal(&db, 30, 14).unwrap();
        assert_eq!(pruned, 2); // old_committed and old_failed pruned

        // Verify active and recoverable remain intact
        assert!(get_turn(&db, &inflight.id).is_ok());
        assert!(get_turn(&db, &interrupted.id).is_ok());
        assert!(get_turn(&db, &recent_committed.id).is_ok());
        assert!(get_turn(&db, &old_committed.id).is_err());
        assert!(get_turn(&db, &old_failed.id).is_err());
    }
}
