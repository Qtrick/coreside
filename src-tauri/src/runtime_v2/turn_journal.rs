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
                )
                | (InterruptedRecoverable, Claimed) // recovery retry same turn new attempt handled separately
                | (Failed, Claimed) // explicit retry
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
    // Idempotent create: return existing if key already present.
    if let Ok(existing) = get_turn_by_idempotency(db, idempotency_key) {
        return Ok(existing);
    }
    let id = format!("turn-{}", Uuid::new_v4());
    let attempt_id = format!("attempt-{}", Uuid::new_v4());
    let now = now_rfc3339();
    db.conn().execute(
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
    )?;
    get_turn(db, &id)
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

pub fn get_turn_by_idempotency(db: &Database, key: &str) -> DbResult<TurnJournalRecord> {
    db.conn()
        .query_row(
            "SELECT id, conversation_id, project_id, attempt_id, idempotency_key, state,
                    route, provider, model, reservation_id, provisional_text, operations_json,
                    error_category, error_message, created_at, updated_at, finalized_at
             FROM turn_journal WHERE idempotency_key = ?1",
            [key],
            map_turn_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("turn idempotency {key}"))
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
        state: TurnState::parse(&state_s).unwrap_or(TurnState::Failed),
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

/// Transition only if the caller owns `attempt_id` and the transition is legal.
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
    let now = now_rfc3339();
    let finalized = if matches!(
        next,
        TurnState::Committed | TurnState::Published | TurnState::Failed
    ) {
        Some(now.as_str())
    } else {
        None
    };
    db.conn().execute(
        "UPDATE turn_journal SET
            state = ?1,
            updated_at = ?2,
            finalized_at = COALESCE(?3, finalized_at),
            reservation_id = COALESCE(?4, reservation_id),
            provisional_text = COALESCE(?5, provisional_text),
            operations_json = COALESCE(?6, operations_json),
            provider = COALESCE(?7, provider),
            model = COALESCE(?8, model),
            error_category = COALESCE(?9, error_category),
            error_message = COALESCE(?10, error_message)
         WHERE id = ?11 AND attempt_id = ?12",
        params![
            next.as_str(),
            now,
            finalized,
            patch.reservation_id,
            patch.provisional_text,
            patch.operations_json,
            patch.provider,
            patch.model,
            patch.error_category,
            patch.error_message,
            turn_id,
            attempt_id
        ],
    )?;
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

/// Append a conversation-scoped event after durable commit (cursor resume).
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
    let seq: i64 = db.conn().query_row(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM conversation_event_log WHERE conversation_id = ?1",
        [conversation_id],
        |r| r.get(0),
    )?;
    db.conn().execute(
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
    Ok((id, seq))
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
}
