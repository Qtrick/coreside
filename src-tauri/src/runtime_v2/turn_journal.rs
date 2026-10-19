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
use rusqlite::{params, OptionalExtension};

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
                // Buffered provider responses never stream live deltas; no
                // production path records Streaming today, so allow them to
                // settle the typed response directly instead of breaking the
                // TypedTerminal → Finalizing → Committed → Published chain.
                | (ProviderStarted, TypedTerminal)
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
                // Explicit discard of recoverable staging (not a retry reclaim).
                | (InterruptedRecoverable, Failed) // Failed|InterruptedRecoverable → Claimed requires begin_retry_attempt
                                                   // (rotates attempt_id). Do not allow transition_turn to reclaim without rotation.
        )
    }
}

/// Durable progressive ops checkpoint schema (staging only — never Apply authority).
pub const PROGRESSIVE_CHECKPOINT_V: &str = "coreside.ops.checkpoint.v1";
/// Cap serialized checkpoint JSON so SQLite growth stays bounded.
pub const MAX_PROGRESSIVE_CHECKPOINT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressiveOpsCheckpoint {
    pub v: String,
    pub attempt_id: String,
    pub sequence: u32,
    pub complete: bool,
    pub ops: Vec<crate::runtime_v2::operations::AppOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

impl ProgressiveOpsCheckpoint {
    pub fn new(
        attempt_id: impl Into<String>,
        ops: Vec<crate::runtime_v2::operations::AppOperation>,
        complete: bool,
    ) -> Self {
        let sequence = ops.len() as u32;
        let mut cp = Self {
            v: PROGRESSIVE_CHECKPOINT_V.into(),
            attempt_id: attempt_id.into(),
            sequence,
            complete,
            ops,
            content_hash: None,
        };
        cp.content_hash = Some(cp.compute_hash());
        cp
    }

    fn compute_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.v.as_bytes());
        hasher.update(self.attempt_id.as_bytes());
        hasher.update(self.sequence.to_le_bytes());
        hasher.update([u8::from(self.complete)]);
        if let Ok(ops) = serde_json::to_vec(&self.ops) {
            hasher.update(&ops);
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn serialize_bounded(&self) -> DbResult<String> {
        let json = serde_json::to_string(self)
            .map_err(|e| DbError::Invalid(format!("checkpoint_serialize: {e}")))?;
        if json.len() > MAX_PROGRESSIVE_CHECKPOINT_BYTES {
            return Err(DbError::Invalid(format!(
                "checkpoint_too_large: {} bytes (max {MAX_PROGRESSIVE_CHECKPOINT_BYTES})",
                json.len()
            )));
        }
        Ok(json)
    }

    pub fn parse(raw: &str) -> DbResult<Self> {
        if raw.len() > MAX_PROGRESSIVE_CHECKPOINT_BYTES {
            return Err(DbError::Invalid(format!(
                "checkpoint_too_large: {} bytes (max {MAX_PROGRESSIVE_CHECKPOINT_BYTES})",
                raw.len()
            )));
        }
        let cp: Self = serde_json::from_str(raw)
            .map_err(|e| DbError::Invalid(format!("checkpoint_parse: {e}")))?;
        if cp.v != PROGRESSIVE_CHECKPOINT_V {
            return Err(DbError::Invalid(format!(
                "checkpoint_version: unsupported {}",
                cp.v
            )));
        }
        if cp.ops.len() as u32 != cp.sequence {
            return Err(DbError::Invalid(
                "checkpoint_sequence_mismatch: sequence must equal ops.len()".into(),
            ));
        }
        // v1 requires content_hash. Hash is staging integrity only — never Apply authority.
        let Some(ref stored) = cp.content_hash else {
            return Err(DbError::Invalid(
                "checkpoint_hash_required: contentHash missing".into(),
            ));
        };
        if stored.len() != 64 || !stored.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DbError::Invalid(
                "checkpoint_hash_malformed: expected 64 hex chars".into(),
            ));
        }
        let expected = cp.compute_hash();
        if stored != &expected {
            return Err(DbError::Invalid(
                "checkpoint_hash_mismatch: operations_json tampered or corrupt".into(),
            ));
        }
        Ok(cp)
    }
}

/// Persist a validated progressive ops checkpoint on the turn journal (staging only).
pub fn write_progressive_checkpoint(
    db: &Database,
    turn_id: &str,
    attempt_id: &str,
    ops: Vec<crate::runtime_v2::operations::AppOperation>,
    complete: bool,
) -> DbResult<TurnJournalRecord> {
    let current = get_turn(db, turn_id)?;
    if current.attempt_id != attempt_id {
        return Err(DbError::Invalid(
            "stale_attempt: checkpoint attempt_id mismatch".into(),
        ));
    }
    // Freeze staging once interrupted — Review/Retry/Discard must see a stable
    // checkpoint. Terminal states also deny further writes.
    if matches!(
        current.state,
        TurnState::Committed
            | TurnState::Published
            | TurnState::Failed
            | TurnState::InterruptedRecoverable
    ) {
        return Err(DbError::Invalid(format!(
            "checkpoint_denied: turn is {:?}",
            current.state
        )));
    }
    // Append-only prefix continuity for the same attempt.
    // Length alone is insufficient — a longer vec must preserve the prior prefix.
    // complete=true is terminal: no demotion, no late ops after completion.
    if let Some(raw) = current.operations_json.as_deref() {
        if let Ok(prior) = ProgressiveOpsCheckpoint::parse(raw) {
            if prior.attempt_id == attempt_id {
                if prior.complete && !complete {
                    return Err(DbError::Invalid(
                        "checkpoint_complete_regression: cannot demote complete=true".into(),
                    ));
                }
                if prior.complete && ops.len() > prior.ops.len() {
                    return Err(DbError::Invalid(
                        "checkpoint_late_ops: cannot append after complete=true".into(),
                    ));
                }
                if ops.len() < prior.ops.len() {
                    return Err(DbError::Invalid(
                        "checkpoint_out_of_order: cannot regress ops sequence".into(),
                    ));
                }
                if ops.len() > prior.ops.len() {
                    if prior.ops != ops[..prior.ops.len()] {
                        return Err(DbError::Invalid(
                            "checkpoint_prefix_mismatch: later checkpoint altered earlier ops"
                                .into(),
                        ));
                    }
                }
                if ops.len() == prior.ops.len() {
                    let promote_complete = !prior.complete && complete;
                    if !promote_complete {
                        // Idempotent only when ops+complete match; diverge → conflict.
                        let candidate =
                            ProgressiveOpsCheckpoint::new(attempt_id, ops.clone(), complete);
                        if prior.content_hash != candidate.content_hash {
                            return Err(DbError::Invalid(
                                "checkpoint_conflict: same sequence with different ops".into(),
                            ));
                        }
                        return Ok(current);
                    }
                    // Fall through to rewrite with complete=true.
                }
            }
        }
    }
    let checkpoint = ProgressiveOpsCheckpoint::new(attempt_id, ops, complete);
    let json = checkpoint.serialize_bounded()?;
    let next_state = match current.state {
        TurnState::ProviderStarted | TurnState::Streaming => TurnState::Streaming,
        other => other,
    };
    if next_state != current.state {
        transition_turn(
            db,
            turn_id,
            attempt_id,
            next_state,
            TurnPatch {
                operations_json: Some(json),
                ..Default::default()
            },
        )
    } else {
        // Same state: still patch operations_json via a no-op self-transition.
        // transition_turn requires a state change — update directly with CAS.
        let now = now_rfc3339();
        let n = db.conn().execute(
            "UPDATE turn_journal SET operations_json = ?1, updated_at = ?2
             WHERE id = ?3 AND attempt_id = ?4 AND state = ?5",
            params![json, now, turn_id, attempt_id, current.state.as_str()],
        )?;
        if n != 1 {
            return Err(DbError::Invalid(
                "turn_cas_conflict: checkpoint write failed".into(),
            ));
        }
        get_turn(db, turn_id)
    }
}

/// Discard recoverable staging without applying. Clears operations_json.
///
/// `conversation_id` must match the turn row so a client cannot discard another
/// conversation's recoverable staging by turn id alone.
///
/// When `expected_attempt_id` / `expected_sequence` are provided, they must match
/// the current row so a stale Review Preview cannot discard a newer attempt.
/// Sequence CAS also binds the exact `operations_json` blob in the UPDATE.
///
/// `error_category` / `error_message` distinguish user discard vs retry retirement.
pub fn discard_recoverable_turn(
    db: &Database,
    turn_id: &str,
    conversation_id: &str,
    expected_attempt_id: Option<&str>,
    expected_sequence: Option<u32>,
    error_category: &str,
    error_message: &str,
) -> DbResult<TurnJournalRecord> {
    let current = get_turn(db, turn_id)?;
    if current.conversation_id != conversation_id {
        return Err(DbError::Invalid(
            "turn does not belong to conversation".into(),
        ));
    }
    if current.state != TurnState::InterruptedRecoverable {
        return Err(DbError::Invalid(format!(
            "discard only from interrupted_recoverable, got {:?}",
            current.state
        )));
    }
    if let Some(expected) = expected_attempt_id {
        if current.attempt_id != expected {
            return Err(DbError::Invalid(
                "stale_attempt: discard attempt_id mismatch".into(),
            ));
        }
    }
    // When sequence is asserted, CAS on the exact checkpoint blob so a concurrent
    // mutation between parse and UPDATE cannot retire a different staging payload.
    let ops_cas: Option<String> = if let Some(expected_seq) = expected_sequence {
        let raw = current
            .operations_json
            .as_deref()
            .ok_or_else(|| DbError::Invalid("discard_sequence: no checkpoint on turn".into()))?;
        let cp = ProgressiveOpsCheckpoint::parse(raw)?;
        if cp.sequence != expected_seq {
            return Err(DbError::Invalid(
                "stale_checkpoint: discard sequence mismatch".into(),
            ));
        }
        Some(raw.to_string())
    } else {
        None
    };
    let attempt_id = expected_attempt_id
        .unwrap_or(current.attempt_id.as_str())
        .to_string();
    let category = if error_category.trim().is_empty() {
        "discarded"
    } else {
        error_category
    };
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE turn_journal SET
            state = 'failed',
            operations_json = NULL,
            provisional_text = NULL,
            error_category = ?1,
            error_message = ?2,
            updated_at = ?3,
            finalized_at = ?3
         WHERE id = ?4 AND attempt_id = ?5 AND conversation_id = ?6
           AND state = 'interrupted_recoverable'
           AND (?7 IS NULL OR operations_json = ?7)",
        params![
            category,
            error_message,
            now,
            turn_id,
            attempt_id,
            conversation_id,
            ops_cas
        ],
    )?;
    if n != 1 {
        return Err(DbError::Invalid(
            "turn_cas_conflict: discard recoverable turn failed".into(),
        ));
    }
    get_turn(db, turn_id)
}

/// Backend-validated recovery eligibility (never trust frontend checkpoint parsing).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryStatus {
    /// Valid staged checkpoint — read-only preview may be reconstructed.
    PreviewAvailable,
    /// Interrupted but no usable checkpoint (empty / missing ops).
    PreviewUnavailable,
    /// Checkpoint present but fails integrity / schema checks.
    CheckpointCorrupted,
    /// Turn is not in a recoverable interrupted state.
    NotRecoverable,
}

/// Compact list DTO — no raw operations_json (ops stay in Rust until preview rebuild).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecoverableGenerationSummary {
    pub turn_id: String,
    pub conversation_id: String,
    pub attempt_id: String,
    pub sequence: u32,
    pub complete: bool,
    pub status: RecoveryStatus,
    pub op_count: u32,
    pub content_hash: Option<String>,
    pub updated_at: String,
    pub can_retry: bool,
    pub can_discard: bool,
    pub can_review: bool,
}

/// Classify a turn journal row for the recovery banner (no Apply authority).
pub fn classify_recoverable_turn(turn: &TurnJournalRecord) -> RecoverableGenerationSummary {
    let base = |status: RecoveryStatus,
                sequence: u32,
                complete: bool,
                op_count: u32,
                content_hash: Option<String>,
                can_review: bool| RecoverableGenerationSummary {
        turn_id: turn.id.clone(),
        conversation_id: turn.conversation_id.clone(),
        attempt_id: turn.attempt_id.clone(),
        sequence,
        complete,
        status,
        op_count,
        content_hash,
        updated_at: turn.updated_at.clone(),
        // Corrupted staging still allows Retry (new attempt) and Discard — only
        // Review requires a parseable non-empty checkpoint. User cancel of a
        // progressive hold is recoverable (Journey 34) — do not hide it.
        can_retry: matches!(
            status,
            RecoveryStatus::PreviewAvailable
                | RecoveryStatus::PreviewUnavailable
                | RecoveryStatus::CheckpointCorrupted
        ) && turn.state == TurnState::InterruptedRecoverable,
        can_discard: turn.state == TurnState::InterruptedRecoverable,
        can_review,
    };

    if turn.state != TurnState::InterruptedRecoverable {
        return base(RecoveryStatus::NotRecoverable, 0, false, 0, None, false);
    }

    let Some(raw) = turn.operations_json.as_deref() else {
        return base(RecoveryStatus::PreviewUnavailable, 0, false, 0, None, false);
    };
    match ProgressiveOpsCheckpoint::parse(raw) {
        Ok(cp) if !cp.ops.is_empty() => base(
            RecoveryStatus::PreviewAvailable,
            cp.sequence,
            cp.complete,
            cp.ops.len() as u32,
            cp.content_hash,
            true,
        ),
        Ok(cp) => base(
            RecoveryStatus::PreviewUnavailable,
            cp.sequence,
            cp.complete,
            0,
            cp.content_hash,
            false,
        ),
        Err(_) => base(
            RecoveryStatus::CheckpointCorrupted,
            0,
            false,
            0,
            None,
            false,
        ),
    }
}

/// List conversation recoverable generations as typed summaries (newest first).
///
/// Includes interrupted turns with missing/corrupt checkpoints so the UI can
/// offer Retry/Discard with accurate status — not only valid staging.
pub fn list_recoverable_generations(
    db: &Database,
    conversation_id: &str,
    limit: usize,
) -> DbResult<Vec<RecoverableGenerationSummary>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let capped = limit.min(20);
    let mut stmt = db.conn().prepare(
        "SELECT id FROM turn_journal
         WHERE conversation_id = ?1
           AND state = 'interrupted_recoverable'
         ORDER BY updated_at DESC
         LIMIT ?2",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![conversation_id, capped as i64], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let turn = get_turn(db, &id)?;
        let summary = classify_recoverable_turn(&turn);
        if matches!(
            summary.status,
            RecoveryStatus::PreviewAvailable
                | RecoveryStatus::PreviewUnavailable
                | RecoveryStatus::CheckpointCorrupted
        ) {
            out.push(summary);
        }
    }
    Ok(out)
}

/// Resolve the original user message id encoded in a turn idempotency key.
pub fn resolve_turn_user_message_id(idempotency_key: &str) -> Option<String> {
    if let Some(rest) = idempotency_key.strip_prefix("turn-") {
        if !rest.is_empty() {
            return Some(rest.to_string());
        }
    }
    None
}

/// Mark other interrupted recoverable turns in a conversation as superseded.
///
/// Soft supersession: staging bytes are retained (`operations_json` kept) so a
/// failed admission / mistaken supersede cannot silently destroy the only copy.
/// Rows leave `interrupted_recoverable` and therefore leave the recovery banner.
/// Prefer [`prune_recoverable_turns_over_cap`] for ordinary chat retention.
pub fn supersede_conversation_recoverable_turns(
    db: &Database,
    conversation_id: &str,
    except_turn_id: Option<&str>,
) -> DbResult<usize> {
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE turn_journal SET
            state = 'failed',
            error_category = 'superseded',
            error_message = ?1,
            updated_at = ?2,
            finalized_at = ?2
         WHERE conversation_id = ?3
           AND state = 'interrupted_recoverable'
           AND (?4 IS NULL OR id != ?4)",
        params![
            "Superseded by a newer message in this conversation",
            now,
            conversation_id,
            except_turn_id
        ],
    )?;
    Ok(n)
}

/// Bound recoverable staging per conversation (newest kept).
///
/// Ordinary new messages must not wipe sibling recoveries; only excess beyond
/// `max_keep` is retired. `except_turn_id` is never pruned.
pub fn prune_recoverable_turns_over_cap(
    db: &Database,
    conversation_id: &str,
    max_keep: usize,
    except_turn_id: Option<&str>,
) -> DbResult<usize> {
    if max_keep == 0 {
        return Ok(0);
    }
    let mut stmt = db.conn().prepare(
        "SELECT id FROM turn_journal
         WHERE conversation_id = ?1
           AND state = 'interrupted_recoverable'
         ORDER BY updated_at DESC",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![conversation_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut kept = 0usize;
    let mut pruned = 0usize;
    let now = now_rfc3339();
    for id in ids {
        if except_turn_id == Some(id.as_str()) {
            kept += 1;
            continue;
        }
        if kept < max_keep {
            kept += 1;
            continue;
        }
        let n = db.conn().execute(
            "UPDATE turn_journal SET
                state = 'failed',
                operations_json = NULL,
                provisional_text = NULL,
                error_category = 'retention_capped',
                error_message = ?1,
                updated_at = ?2,
                finalized_at = ?2
             WHERE id = ?3 AND conversation_id = ?4 AND state = 'interrupted_recoverable'",
            params![
                "Recoverable staging retired by retention cap",
                now,
                id,
                conversation_id
            ],
        )?;
        pruned += n;
    }
    Ok(pruned)
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
    // Clear prior attempt staging so retries cannot Apply stale incomplete ops.
    let n = db.conn().execute(
        "UPDATE turn_journal SET
            attempt_id = ?1,
            state = 'claimed',
            updated_at = ?2,
            finalized_at = NULL,
            error_category = NULL,
            error_message = NULL,
            operations_json = NULL,
            provisional_text = NULL
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
    // Only checkpointed staging is recoverable in the UI. Newest first so the
    // banner surfaces the latest interrupted attempt. User cancel of a
    // progressive hold keeps error_category='cancelled' and MUST appear when
    // operations_json is present (Review / Retry / Discard).
    let mut stmt = db.conn().prepare(
        "SELECT id FROM turn_journal
         WHERE conversation_id = ?1
           AND state = 'interrupted_recoverable'
           AND operations_json IS NOT NULL
         ORDER BY updated_at DESC
         LIMIT ?2",
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
    let tx = db.conn().unchecked_transaction()?;
    let result = append_conversation_event_on_conn(
        &tx,
        conversation_id,
        turn_id,
        attempt_id,
        event_type,
        payload,
    )?;
    tx.commit()?;
    Ok(result)
}

/// Same as [`append_conversation_event`] but uses an existing connection/savepoint
/// so callers can keep state mutation + event log atomic.
pub fn append_conversation_event_on_conn(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    turn_id: Option<&str>,
    attempt_id: Option<&str>,
    event_type: &str,
    payload: &Value,
) -> DbResult<(String, i64)> {
    let id = format!("cev-{}", Uuid::new_v4());
    let now = now_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO conversation_event_sequences (conversation_id, next_sequence)
         VALUES (?1, 1)",
        [conversation_id],
    )?;
    let seq: i64 = conn.query_row(
        "UPDATE conversation_event_sequences
         SET next_sequence = next_sequence + 1
         WHERE conversation_id = ?1
         RETURNING next_sequence - 1",
        [conversation_id],
        |r| r.get(0),
    )?;
    conn.execute(
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

/// Durable catch-up watermark for one (conversation, client) pair (0 = never advanced).
///
/// `client_id` is the Tauri window label (or equivalent webview session id). Cursors
/// must not be shared across windows — advancing one client's watermark must not
/// cause another client to skip catch-up.
pub fn get_conversation_sync_cursor(
    db: &Database,
    conversation_id: &str,
    client_id: &str,
) -> DbResult<i64> {
    let client = normalize_sync_client_id(client_id)?;
    let seq: Option<i64> = db
        .conn()
        .query_row(
            "SELECT last_sequence FROM conversation_sync_cursors
             WHERE conversation_id = ?1 AND client_id = ?2",
            params![conversation_id, client],
            |r| r.get(0),
        )
        .optional()?;
    Ok(seq.unwrap_or(0).max(0))
}

fn normalize_sync_client_id(client_id: &str) -> DbResult<String> {
    let trimmed = client_id.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return Err(DbError::Invalid(
            "conversation sync client_id must be 1..=256 characters".into(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(DbError::Invalid(
            "conversation sync client_id contains invalid characters".into(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Monotonic advance of the durable catch-up watermark for one client.
///
/// Returns the stored cursor after the attempt. Never moves backwards: if
/// `sequence` is less than or equal to the current watermark, the existing
/// value is returned unchanged. Never advances past the highest sequence
/// currently present in `conversation_event_log` (a buggy client cannot
/// permanently skip catch-up by forging an inflated watermark). Requires the
/// conversation to exist so a forged id cannot create orphan cursor rows.
pub fn advance_conversation_sync_cursor(
    db: &Database,
    conversation_id: &str,
    client_id: &str,
    sequence: i64,
) -> DbResult<i64> {
    if sequence < 0 {
        return Err(DbError::Invalid(
            "conversation sync cursor sequence must be non-negative".into(),
        ));
    }
    let client = normalize_sync_client_id(client_id)?;
    let exists: bool = db
        .conn()
        .query_row(
            "SELECT 1 FROM conversations WHERE id = ?1",
            params![conversation_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !exists {
        return Err(DbError::NotFound(format!("conversation {conversation_id}")));
    }
    let max_logged: i64 = db
        .conn()
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM conversation_event_log
             WHERE conversation_id = ?1",
            params![conversation_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let capped = sequence.min(max_logged);
    let now = now_rfc3339();
    let current = get_conversation_sync_cursor(db, conversation_id, &client)?;
    if capped <= current {
        return Ok(current);
    }
    db.conn().execute(
        "INSERT INTO conversation_sync_cursors (conversation_id, client_id, last_sequence, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(conversation_id, client_id) DO UPDATE SET
           last_sequence = CASE
             WHEN excluded.last_sequence > conversation_sync_cursors.last_sequence
             THEN excluded.last_sequence
             ELSE conversation_sync_cursors.last_sequence
           END,
           updated_at = CASE
             WHEN excluded.last_sequence > conversation_sync_cursors.last_sequence
             THEN excluded.updated_at
             ELSE conversation_sync_cursors.updated_at
           END",
        params![conversation_id, client, capped, now],
    )?;
    get_conversation_sync_cursor(db, conversation_id, &client)
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

    fn reach_provider_started(db: &Database, idem: &str) -> TurnJournalRecord {
        let turn = create_turn(db, "c1", None, idem, None).unwrap();
        transition_turn(
            db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            db,
            &turn.id,
            &turn.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();
        get_turn(db, &turn.id).unwrap()
    }

    #[test]
    fn provider_started_can_transition_to_failed() {
        let db = db();
        let turn = reach_provider_started(&db, "idem-ps-fail");
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Failed,
            TurnPatch {
                error_category: Some("interactive_repair_exhausted".into()),
                error_message: Some("repair could not produce valid ops".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let settled = get_turn(&db, &turn.id).unwrap();
        assert_eq!(settled.state, TurnState::Failed);
        assert!(settled.finalized_at.is_some());
        // Must not silently continue the happy path after failure.
        assert!(transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::TypedTerminal,
            TurnPatch::default(),
        )
        .is_err());
    }

    #[test]
    fn provider_started_can_transition_to_interrupted_recoverable() {
        let db = db();
        let turn = reach_provider_started(&db, "idem-ps-ir");
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::InterruptedRecoverable,
            TurnPatch {
                error_category: Some("interactive_repair_exhausted".into()),
                error_message: Some("interrupted during interactive repair".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let settled = get_turn(&db, &turn.id).unwrap();
        assert_eq!(settled.state, TurnState::InterruptedRecoverable);
        // Recoverable interrupt is not a published settlement.
        assert!(transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Published,
            TurnPatch::default(),
        )
        .is_err());
        assert!(transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::TypedTerminal,
            TurnPatch::default(),
        )
        .is_err());
    }

    fn seed_conversation(db: &mut Database, id: &str) {
        crate::db::ensure_default_workspace(db).unwrap();
        let now = now_rfc3339();
        db.conn()
            .execute(
                "INSERT INTO conversations (id, workspace_id, title, created_at, updated_at)
                 VALUES (?1, ?2, 'Catch-up', ?3, ?3)",
                rusqlite::params![id, crate::db::DEFAULT_WORKSPACE_ID, now],
            )
            .unwrap();
    }

    fn seed_events(db: &Database, conversation_id: &str, count: i64) {
        for i in 1..=count {
            append_conversation_event(
                db,
                conversation_id,
                None,
                None,
                "action",
                &serde_json::json!({ "n": i }),
            )
            .unwrap();
        }
    }

    #[test]
    fn sync_cursor_defaults_to_zero_and_advances_monotonically() {
        let mut db = db();
        seed_conversation(&mut db, "c-sync");
        seed_events(&db, "c-sync", 200);
        assert_eq!(
            get_conversation_sync_cursor(&db, "c-sync", "main").unwrap(),
            0
        );
        assert_eq!(
            advance_conversation_sync_cursor(&db, "c-sync", "main", 10).unwrap(),
            10
        );
        assert_eq!(
            advance_conversation_sync_cursor(&db, "c-sync", "main", 5).unwrap(),
            10,
            "cursor must not move backwards"
        );
        assert_eq!(
            advance_conversation_sync_cursor(&db, "c-sync", "main", 10).unwrap(),
            10,
            "equal advance is a no-op"
        );
        assert_eq!(
            advance_conversation_sync_cursor(&db, "c-sync", "main", 200).unwrap(),
            200
        );
        assert_eq!(
            get_conversation_sync_cursor(&db, "c-sync", "main").unwrap(),
            200
        );
        // Inflated sequence is capped to MAX(log) — cannot permanently skip catch-up.
        assert_eq!(
            advance_conversation_sync_cursor(&db, "c-sync", "main", 9999).unwrap(),
            200
        );
    }

    #[test]
    fn sync_cursor_rejects_unknown_conversation_and_negative() {
        let missing = db();
        let err =
            advance_conversation_sync_cursor(&missing, "missing-conv", "main", 1).unwrap_err();
        assert!(matches!(err, DbError::NotFound(_)));
        let mut db = db();
        seed_conversation(&mut db, "c-neg");
        let err = advance_conversation_sync_cursor(&db, "c-neg", "main", -1).unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
        assert_eq!(
            get_conversation_sync_cursor(&db, "c-neg", "main").unwrap(),
            0
        );
        // Empty log: advance is a no-op (capped to 0).
        assert_eq!(
            advance_conversation_sync_cursor(&db, "c-neg", "main", 50).unwrap(),
            0
        );
    }

    #[test]
    fn sync_cursor_is_conversation_and_client_scoped() {
        let mut db = db();
        seed_conversation(&mut db, "c-a");
        seed_conversation(&mut db, "c-b");
        seed_events(&db, "c-a", 50);
        seed_events(&db, "c-b", 3);
        advance_conversation_sync_cursor(&db, "c-a", "main", 50).unwrap();
        advance_conversation_sync_cursor(&db, "c-b", "main", 3).unwrap();
        assert_eq!(
            get_conversation_sync_cursor(&db, "c-a", "main").unwrap(),
            50
        );
        assert_eq!(get_conversation_sync_cursor(&db, "c-b", "main").unwrap(), 3);
        // Window B advancing must not poison Window A's watermark.
        advance_conversation_sync_cursor(&db, "c-a", "tool-1", 50).unwrap();
        assert_eq!(
            get_conversation_sync_cursor(&db, "c-a", "main").unwrap(),
            50
        );
        assert_eq!(
            get_conversation_sync_cursor(&db, "c-a", "tool-1").unwrap(),
            50
        );
        assert_eq!(
            get_conversation_sync_cursor(&db, "c-a", "other").unwrap(),
            0
        );
    }

    #[test]
    fn sync_cursor_cascades_when_conversation_deleted() {
        let mut db = db();
        seed_conversation(&mut db, "c-cascade");
        seed_events(&db, "c-cascade", 42);
        advance_conversation_sync_cursor(&db, "c-cascade", "main", 42).unwrap();
        advance_conversation_sync_cursor(&db, "c-cascade", "tool-x", 20).unwrap();
        assert_eq!(
            get_conversation_sync_cursor(&db, "c-cascade", "main").unwrap(),
            42
        );
        db.conn()
            .execute(
                "DELETE FROM conversations WHERE id = ?1",
                params!["c-cascade"],
            )
            .unwrap();
        // Rows must be gone (CASCADE); getter returns the zero default.
        let remaining: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM conversation_sync_cursors WHERE conversation_id = ?1",
                params!["c-cascade"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(
            get_conversation_sync_cursor(&db, "c-cascade", "main").unwrap(),
            0
        );
    }

    #[test]
    fn catch_up_pages_resume_from_persisted_cursor() {
        let mut db = db();
        seed_conversation(&mut db, "c-page");
        // Seed 201 events so two 200-sized pages are required.
        for i in 1..=201 {
            append_conversation_event(
                &db,
                "c-page",
                None,
                None,
                if i == 100 || i == 201 {
                    "surface.transaction_applied"
                } else {
                    "action"
                },
                &serde_json::json!({ "n": i }),
            )
            .unwrap();
        }
        // Simulate crash after first page of 200.
        advance_conversation_sync_cursor(&db, "c-page", "main", 200).unwrap();
        let remaining = get_conversation_events(&db, "c-page", Some(200), Some(200)).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].sequence, 201);
        assert_eq!(remaining[0].event_type, "surface.transaction_applied");
        advance_conversation_sync_cursor(&db, "c-page", "main", 201).unwrap();
        assert!(get_conversation_events(&db, "c-page", Some(201), Some(200))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn sync_cursor_rejects_invalid_client_id() {
        let mut db = db();
        seed_conversation(&mut db, "c-bad-client");
        seed_events(&db, "c-bad-client", 1);
        let err = advance_conversation_sync_cursor(&db, "c-bad-client", "", 1).unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
        let err = advance_conversation_sync_cursor(&db, "c-bad-client", "bad id!", 1).unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
    }

    #[test]
    fn stale_and_future_cursor_never_duplicate_or_skip_events() {
        let mut db = db();
        seed_conversation(&mut db, "c-stale-future");
        seed_events(&db, "c-stale-future", 10);

        // Stale advance (behind watermark) is a no-op.
        assert_eq!(
            advance_conversation_sync_cursor(&db, "c-stale-future", "main", 4).unwrap(),
            4
        );
        assert_eq!(
            advance_conversation_sync_cursor(&db, "c-stale-future", "main", 2).unwrap(),
            4
        );

        // Future advance is capped to MAX(log); cannot permanently skip.
        assert_eq!(
            advance_conversation_sync_cursor(&db, "c-stale-future", "main", 99_999).unwrap(),
            10
        );

        // After cursor, get_conversation_events must not redeliver applied rows.
        let after = get_conversation_events(&db, "c-stale-future", Some(10), Some(200)).unwrap();
        assert!(
            after.is_empty(),
            "cursor at max must not redeliver: {after:?}"
        );

        // Mid-watermark page is exclusive after_sequence — no duplicates of 1..=4.
        let mid = get_conversation_events(&db, "c-stale-future", Some(4), Some(200)).unwrap();
        assert_eq!(mid.len(), 6);
        assert_eq!(mid.first().map(|e| e.sequence), Some(5));
        assert_eq!(mid.last().map(|e| e.sequence), Some(10));
        let seqs: Vec<i64> = mid.iter().map(|e| e.sequence).collect();
        let mut unique = seqs.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            seqs, unique,
            "event page must not contain duplicate sequences"
        );
    }

    #[test]
    fn append_assigns_monotonic_unique_sequences() {
        let mut db = db();
        seed_conversation(&mut db, "c-mono");
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..20 {
            let (_id, seq) = append_conversation_event(
                &db,
                "c-mono",
                None,
                None,
                "action",
                &serde_json::json!({}),
            )
            .unwrap();
            assert!(seen.insert(seq), "duplicate sequence {seq}");
        }
        assert_eq!(seen.len(), 20);
        assert_eq!(*seen.iter().next().unwrap(), 1);
        assert_eq!(*seen.iter().next_back().unwrap(), 20);
    }

    fn sample_checkpoint_op(id: &str) -> crate::runtime_v2::operations::AppOperation {
        crate::runtime_v2::operations::AppOperation {
            id: id.into(),
            op_type: "component.update_props".into(),
            target: crate::runtime_v2::operations::OperationTarget {
                surface_id: Some("s-study-planner-summary".into()),
                ..Default::default()
            },
            payload: serde_json::json!({ "componentId": "heading", "props": { "text": id } }),
            ..Default::default()
        }
    }

    #[test]
    fn progressive_checkpoint_persists_and_survives_reopen_semantics() {
        let mut db = db();
        seed_conversation(&mut db, "c-ckpt");
        let turn = create_turn(&db, "c-ckpt", None, "idem-ckpt-1", Some("interactive")).unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();

        let ops = vec![sample_checkpoint_op("op-1")];
        let updated =
            write_progressive_checkpoint(&db, &turn.id, &turn.attempt_id, ops.clone(), false)
                .unwrap();
        assert_eq!(updated.state, TurnState::Streaming);
        let parsed =
            ProgressiveOpsCheckpoint::parse(updated.operations_json.as_deref().unwrap()).unwrap();
        assert_eq!(parsed.v, PROGRESSIVE_CHECKPOINT_V);
        assert_eq!(parsed.sequence, 1);
        assert!(!parsed.complete);
        assert_eq!(parsed.ops.len(), 1);

        // Idempotent same-length checkpoint.
        let again =
            write_progressive_checkpoint(&db, &turn.id, &turn.attempt_id, ops.clone(), false)
                .unwrap();
        assert_eq!(again.operations_json, updated.operations_json);

        // Out-of-order length regression rejected while still incomplete.
        let err = write_progressive_checkpoint(&db, &turn.id, &turn.attempt_id, vec![], false)
            .unwrap_err();
        assert!(matches!(err, DbError::Invalid(msg) if msg.contains("out_of_order")));

        // Terminal complete with no new ops must promote complete=true.
        let completed =
            write_progressive_checkpoint(&db, &turn.id, &turn.attempt_id, ops, true).unwrap();
        let completed_cp =
            ProgressiveOpsCheckpoint::parse(completed.operations_json.as_deref().unwrap()).unwrap();
        assert!(completed_cp.complete);
        assert_eq!(completed_cp.sequence, 1);
    }

    #[test]
    fn begin_retry_clears_prior_checkpoint_and_discard_clears_recoverable() {
        let mut db = db();
        seed_conversation(&mut db, "c-retry-ckpt");
        let turn = create_turn(
            &db,
            "c-retry-ckpt",
            None,
            "idem-retry-ckpt",
            Some("interactive"),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();
        write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![sample_checkpoint_op("op-a")],
            false,
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::InterruptedRecoverable,
            TurnPatch::default(),
        )
        .unwrap();

        let prior = get_turn(&db, &turn.id).unwrap();
        assert!(prior.operations_json.is_some());
        let retried = begin_retry_attempt(&db, &turn.id, &prior.attempt_id).unwrap();
        assert!(retried.operations_json.is_none());
        assert_eq!(retried.state, TurnState::Claimed);

        // Interrupt again and discard.
        transition_turn(
            &db,
            &retried.id,
            &retried.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();
        write_progressive_checkpoint(
            &db,
            &retried.id,
            &retried.attempt_id,
            vec![sample_checkpoint_op("op-b")],
            false,
        )
        .unwrap();
        transition_turn(
            &db,
            &retried.id,
            &retried.attempt_id,
            TurnState::InterruptedRecoverable,
            TurnPatch::default(),
        )
        .unwrap();
        let discarded = discard_recoverable_turn(
            &db,
            &retried.id,
            "c-retry-ckpt",
            None,
            None,
            "discarded",
            "User discarded recoverable progressive staging",
        )
        .unwrap();
        assert_eq!(discarded.state, TurnState::Failed);
        assert_eq!(discarded.error_category.as_deref(), Some("discarded"));
        assert!(discarded.operations_json.is_none());
    }

    #[test]
    fn progressive_checkpoint_rejects_stale_attempt_terminal_state_and_bad_parse() {
        let mut db = db();
        seed_conversation(&mut db, "c-ckpt-deny");
        let turn = create_turn(
            &db,
            "c-ckpt-deny",
            None,
            "idem-ckpt-deny",
            Some("interactive"),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();

        let stale = write_progressive_checkpoint(
            &db,
            &turn.id,
            "attempt-stale",
            vec![sample_checkpoint_op("op-stale")],
            false,
        )
        .unwrap_err();
        assert!(matches!(stale, DbError::Invalid(msg) if msg.contains("stale_attempt")));

        // Terminal Failed denies further staging writes.
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Failed,
            TurnPatch {
                error_category: Some("provider".into()),
                error_message: Some("boom".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let denied = write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![sample_checkpoint_op("op-after-fail")],
            false,
        )
        .unwrap_err();
        assert!(matches!(denied, DbError::Invalid(msg) if msg.contains("checkpoint_denied")));

        // Discard is only legal from interrupted_recoverable.
        let discard_err = discard_recoverable_turn(
            &db,
            &turn.id,
            "c-ckpt-deny",
            None,
            None,
            "discarded",
            "User discarded recoverable progressive staging",
        )
        .unwrap_err();
        assert!(matches!(
            discard_err,
            DbError::Invalid(msg) if msg.contains("interrupted_recoverable")
        ));

        let bad_version = ProgressiveOpsCheckpoint::parse(
            r#"{"v":"coreside.ops.checkpoint.v0","attemptId":"a","sequence":0,"complete":false,"ops":[]}"#,
        )
        .unwrap_err();
        assert!(matches!(
            bad_version,
            DbError::Invalid(msg) if msg.contains("checkpoint_version")
        ));

        let bad_seq = ProgressiveOpsCheckpoint::parse(
            r#"{"v":"coreside.ops.checkpoint.v1","attemptId":"a","sequence":2,"complete":false,"ops":[]}"#,
        )
        .unwrap_err();
        assert!(matches!(
            bad_seq,
            DbError::Invalid(msg) if msg.contains("checkpoint_sequence_mismatch")
        ));
    }

    fn interrupt_with_checkpoint(db: &Database, turn: &TurnJournalRecord, op_id: &str) {
        transition_turn(
            db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            db,
            &turn.id,
            &turn.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();
        write_progressive_checkpoint(
            db,
            &turn.id,
            &turn.attempt_id,
            vec![sample_checkpoint_op(op_id)],
            false,
        )
        .unwrap();
        transition_turn(
            db,
            &turn.id,
            &turn.attempt_id,
            TurnState::InterruptedRecoverable,
            TurnPatch::default(),
        )
        .unwrap();
    }

    #[test]
    fn list_conversation_recoverable_turns_filters_and_sorts_newest_first() {
        let mut db = db();
        seed_conversation(&mut db, "c-list-rec");
        let older = create_turn(&db, "c-list-rec", None, "idem-old", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &older, "op-old");
        std::thread::sleep(std::time::Duration::from_millis(5));
        let newer = create_turn(&db, "c-list-rec", None, "idem-new", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &newer, "op-new");

        let empty_ops =
            create_turn(&db, "c-list-rec", None, "idem-empty", Some("interactive")).unwrap();
        transition_turn(
            &db,
            &empty_ops.id,
            &empty_ops.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &empty_ops.id,
            &empty_ops.attempt_id,
            TurnState::InterruptedRecoverable,
            TurnPatch::default(),
        )
        .unwrap();

        let cancelled =
            create_turn(&db, "c-list-rec", None, "idem-cancel", Some("interactive")).unwrap();
        transition_turn(
            &db,
            &cancelled.id,
            &cancelled.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &cancelled.id,
            &cancelled.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();
        write_progressive_checkpoint(
            &db,
            &cancelled.id,
            &cancelled.attempt_id,
            vec![sample_checkpoint_op("op-cancel")],
            false,
        )
        .unwrap();
        transition_turn(
            &db,
            &cancelled.id,
            &cancelled.attempt_id,
            TurnState::InterruptedRecoverable,
            TurnPatch {
                error_category: Some("cancelled".into()),
                error_message: Some("User cancelled request".into()),
                ..Default::default()
            },
        )
        .unwrap();

        let listed = list_conversation_recoverable_turns(&db, "c-list-rec", 10).unwrap();
        assert_eq!(
            listed.len(),
            3,
            "empty ops excluded; cancelled with checkpoint included"
        );
        assert_eq!(listed[0].id, cancelled.id, "newest checkpointed first");
        assert_eq!(listed[1].id, newer.id);
        assert_eq!(listed[2].id, older.id);
        let gens = list_recoverable_generations(&db, "c-list-rec", 10).unwrap();
        assert!(
            gens.iter().any(|g| {
                g.turn_id == cancelled.id
                    && g.status == RecoveryStatus::PreviewAvailable
                    && g.can_retry
                    && g.can_discard
                    && g.can_review
            }),
            "user-cancelled progressive hold must be recoverable"
        );
    }

    #[test]
    fn discard_recoverable_turn_requires_matching_conversation() {
        let mut db = db();
        seed_conversation(&mut db, "c-discard-own");
        let turn = create_turn(
            &db,
            "c-discard-own",
            None,
            "idem-discard",
            Some("interactive"),
        )
        .unwrap();
        interrupt_with_checkpoint(&db, &turn, "op-d");

        let wrong = discard_recoverable_turn(
            &db,
            &turn.id,
            "c-other",
            None,
            None,
            "discarded",
            "User discarded recoverable progressive staging",
        )
        .unwrap_err();
        assert!(matches!(
            wrong,
            DbError::Invalid(msg) if msg.contains("does not belong")
        ));
        let ok = discard_recoverable_turn(
            &db,
            &turn.id,
            "c-discard-own",
            Some(turn.attempt_id.as_str()),
            Some(1),
            "discarded",
            "User discarded recoverable progressive staging",
        )
        .unwrap();
        assert_eq!(ok.state, TurnState::Failed);
        assert_eq!(ok.error_category.as_deref(), Some("discarded"));
    }

    #[test]
    fn progressive_checkpoint_enforces_prefix_complete_and_hash() {
        let mut db = db();
        seed_conversation(&mut db, "c-prefix");
        let turn = create_turn(&db, "c-prefix", None, "idem-prefix", Some("interactive")).unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();

        let op_a = sample_checkpoint_op("op-a");
        let op_b = sample_checkpoint_op("op-b");
        write_progressive_checkpoint(&db, &turn.id, &turn.attempt_id, vec![op_a.clone()], false)
            .unwrap();

        // Longer checkpoint that drops the prior prefix must fail.
        let altered = write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![op_b.clone(), sample_checkpoint_op("op-c")],
            false,
        )
        .unwrap_err();
        assert!(matches!(
            altered,
            DbError::Invalid(msg) if msg.contains("prefix_mismatch")
        ));

        // Valid append preserves prefix.
        write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![op_a.clone(), op_b.clone()],
            false,
        )
        .unwrap();

        // Promote complete.
        write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![op_a.clone(), op_b.clone()],
            true,
        )
        .unwrap();

        // Demote complete rejected.
        let demote = write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![op_a.clone(), op_b.clone()],
            false,
        )
        .unwrap_err();
        assert!(matches!(
            demote,
            DbError::Invalid(msg) if msg.contains("complete_regression")
        ));

        // Late ops after complete rejected.
        let late = write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![op_a, op_b, sample_checkpoint_op("op-late")],
            true,
        )
        .unwrap_err();
        assert!(matches!(
            late,
            DbError::Invalid(msg) if msg.contains("late_ops")
        ));

        // Hash required + mismatch rejected.
        let missing_hash = ProgressiveOpsCheckpoint::parse(
            r#"{"v":"coreside.ops.checkpoint.v1","attemptId":"a","sequence":0,"complete":false,"ops":[]}"#,
        )
        .unwrap_err();
        assert!(matches!(
            missing_hash,
            DbError::Invalid(msg) if msg.contains("checkpoint_hash_required")
        ));

        let mut bad = ProgressiveOpsCheckpoint::new("a", vec![sample_checkpoint_op("x")], false);
        bad.content_hash = Some("0".repeat(64));
        let bad_json = serde_json::to_string(&bad).unwrap();
        let mismatch = ProgressiveOpsCheckpoint::parse(&bad_json).unwrap_err();
        assert!(matches!(
            mismatch,
            DbError::Invalid(msg) if msg.contains("checkpoint_hash_mismatch")
        ));
    }

    #[test]
    fn discard_rejects_stale_attempt_and_sequence() {
        let mut db = db();
        seed_conversation(&mut db, "c-discard-cas");
        let turn =
            create_turn(&db, "c-discard-cas", None, "idem-cas", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &turn, "op-cas");

        let stale_attempt = discard_recoverable_turn(
            &db,
            &turn.id,
            "c-discard-cas",
            Some("attempt-stale"),
            Some(1),
            "discarded",
            "User discarded recoverable progressive staging",
        )
        .unwrap_err();
        assert!(matches!(
            stale_attempt,
            DbError::Invalid(msg) if msg.contains("stale_attempt")
        ));

        let stale_seq = discard_recoverable_turn(
            &db,
            &turn.id,
            "c-discard-cas",
            Some(turn.attempt_id.as_str()),
            Some(99),
            "discarded",
            "User discarded recoverable progressive staging",
        )
        .unwrap_err();
        assert!(matches!(
            stale_seq,
            DbError::Invalid(msg) if msg.contains("stale_checkpoint")
        ));
    }

    #[test]
    fn classify_and_list_recoverable_generations() {
        let mut db = db();
        seed_conversation(&mut db, "c-class");
        let ok = create_turn(&db, "c-class", None, "idem-ok", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &ok, "op-ok");

        let empty = create_turn(&db, "c-class", None, "idem-empty", Some("interactive")).unwrap();
        transition_turn(
            &db,
            &empty.id,
            &empty.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &empty.id,
            &empty.attempt_id,
            TurnState::InterruptedRecoverable,
            TurnPatch::default(),
        )
        .unwrap();

        // Corrupt checkpoint still lists with Retry/Discard (no Review).
        let corrupt =
            create_turn(&db, "c-class", None, "idem-corrupt", Some("interactive")).unwrap();
        transition_turn(
            &db,
            &corrupt.id,
            &corrupt.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &corrupt.id,
            &corrupt.attempt_id,
            TurnState::InterruptedRecoverable,
            TurnPatch {
                operations_json: Some(
                    r#"{"v":"coreside.ops.checkpoint.v1","attemptId":"x","sequence":1,"complete":false,"ops":[]}"#
                        .into(),
                ),
                ..Default::default()
            },
        )
        .unwrap();

        let listed = list_recoverable_generations(&db, "c-class", 10).unwrap();
        assert!(listed.iter().any(|s| {
            s.turn_id == ok.id && s.status == RecoveryStatus::PreviewAvailable && s.can_review
        }));
        assert!(listed.iter().any(|s| {
            s.turn_id == empty.id
                && s.status == RecoveryStatus::PreviewUnavailable
                && !s.can_review
                && s.can_retry
        }));
        assert!(listed.iter().any(|s| {
            s.turn_id == corrupt.id
                && s.status == RecoveryStatus::CheckpointCorrupted
                && !s.can_review
                && s.can_retry
                && s.can_discard
        }));
    }

    #[test]
    fn progressive_checkpoint_denied_after_interrupted_recoverable() {
        let mut db = db();
        seed_conversation(&mut db, "c-freeze");
        let turn = create_turn(&db, "c-freeze", None, "idem-freeze", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &turn, "op-freeze");

        let denied = write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![
                sample_checkpoint_op("op-freeze"),
                sample_checkpoint_op("op-late"),
            ],
            false,
        )
        .unwrap_err();
        assert!(matches!(
            denied,
            DbError::Invalid(msg) if msg.contains("checkpoint_denied")
        ));
    }

    #[test]
    fn supersede_conversation_recoverable_turns_keeps_staging_bytes() {
        let mut db = db();
        seed_conversation(&mut db, "c-supersede");
        let keep = create_turn(&db, "c-supersede", None, "idem-keep", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &keep, "op-keep");
        let drop = create_turn(&db, "c-supersede", None, "idem-drop", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &drop, "op-drop");

        let n =
            supersede_conversation_recoverable_turns(&db, "c-supersede", Some(&keep.id)).unwrap();
        assert_eq!(n, 1);
        let kept = get_turn(&db, &keep.id).unwrap();
        assert_eq!(kept.state, TurnState::InterruptedRecoverable);
        assert!(kept.operations_json.is_some());
        let dropped = get_turn(&db, &drop.id).unwrap();
        assert_eq!(dropped.state, TurnState::Failed);
        assert_eq!(dropped.error_category.as_deref(), Some("superseded"));
        // Soft supersede retains checkpoint bytes (banner no longer lists it).
        assert!(dropped.operations_json.is_some());
    }

    #[test]
    fn interrupted_checkpoint_survives_sqlite_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reopen-recovery.db");
        let turn_id;
        let attempt_id;
        let ops_json;
        {
            let mut db = Database::open_path(&path).unwrap();
            seed_conversation(&mut db, "c-reopen");
            let turn =
                create_turn(&db, "c-reopen", None, "idem-reopen", Some("interactive")).unwrap();
            interrupt_with_checkpoint(&db, &turn, "op-reopen");
            let row = get_turn(&db, &turn.id).unwrap();
            assert_eq!(row.state, TurnState::InterruptedRecoverable);
            assert!(row.operations_json.is_some());
            turn_id = row.id.clone();
            attempt_id = row.attempt_id.clone();
            ops_json = row.operations_json.clone();
        }
        let db = Database::open_path(&path).unwrap();
        let restored = get_turn(&db, &turn_id).unwrap();
        assert_eq!(restored.state, TurnState::InterruptedRecoverable);
        assert_eq!(restored.attempt_id, attempt_id);
        assert_eq!(restored.operations_json, ops_json);
        let listed = list_recoverable_generations(&db, "c-reopen", 5).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, RecoveryStatus::PreviewAvailable);
    }

    #[test]
    fn prune_recoverable_turns_over_cap_retires_oldest() {
        let mut db = db();
        seed_conversation(&mut db, "c-prune");
        let t1 = create_turn(&db, "c-prune", None, "idem-p1", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &t1, "op-p1");
        let t2 = create_turn(&db, "c-prune", None, "idem-p2", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &t2, "op-p2");
        let t3 = create_turn(&db, "c-prune", None, "idem-p3", Some("interactive")).unwrap();
        interrupt_with_checkpoint(&db, &t3, "op-p3");
        // Deterministic recency: t1 oldest, t3 newest.
        db.conn()
            .execute(
                "UPDATE turn_journal SET updated_at = '2020-01-01T00:00:00Z' WHERE id = ?1",
                params![t1.id],
            )
            .unwrap();
        db.conn()
            .execute(
                "UPDATE turn_journal SET updated_at = '2020-01-02T00:00:00Z' WHERE id = ?1",
                params![t2.id],
            )
            .unwrap();
        db.conn()
            .execute(
                "UPDATE turn_journal SET updated_at = '2020-01-03T00:00:00Z' WHERE id = ?1",
                params![t3.id],
            )
            .unwrap();

        let n = prune_recoverable_turns_over_cap(&db, "c-prune", 2, None).unwrap();
        assert_eq!(n, 1);
        assert_eq!(
            get_turn(&db, &t1.id).unwrap().error_category.as_deref(),
            Some("retention_capped")
        );
        assert!(get_turn(&db, &t1.id).unwrap().operations_json.is_none());
        assert_eq!(
            get_turn(&db, &t2.id).unwrap().state,
            TurnState::InterruptedRecoverable
        );
        assert_eq!(
            get_turn(&db, &t3.id).unwrap().state,
            TurnState::InterruptedRecoverable
        );
    }

    #[test]
    fn progressive_checkpoint_rejects_same_sequence_different_ops() {
        let mut db = db();
        seed_conversation(&mut db, "c-same-seq");
        let turn = create_turn(
            &db,
            "c-same-seq",
            None,
            "idem-same-seq",
            Some("interactive"),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();

        write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![sample_checkpoint_op("op-a")],
            false,
        )
        .unwrap();

        let conflict = write_progressive_checkpoint(
            &db,
            &turn.id,
            &turn.attempt_id,
            vec![sample_checkpoint_op("op-b")],
            false,
        )
        .unwrap_err();
        assert!(
            matches!(
                conflict,
                DbError::Invalid(ref msg) if msg.contains("checkpoint_conflict")
                    && msg.contains("same sequence")
            ),
            "expected same-sequence ops conflict, got: {conflict:?}"
        );

        // Prior checkpoint must remain unchanged after conflict.
        let still = get_turn(&db, &turn.id).unwrap();
        let cp =
            ProgressiveOpsCheckpoint::parse(still.operations_json.as_deref().unwrap()).unwrap();
        assert_eq!(cp.ops[0].id, "op-a");
        assert_eq!(cp.sequence, 1);
        assert!(!cp.complete);
    }

    #[test]
    fn progressive_checkpoint_parse_rejects_oversized_payload() {
        let oversized = "x".repeat(MAX_PROGRESSIVE_CHECKPOINT_BYTES + 1);
        let err = ProgressiveOpsCheckpoint::parse(&oversized).unwrap_err();
        assert!(
            matches!(
                err,
                DbError::Invalid(ref msg) if msg.contains("checkpoint_too_large")
            ),
            "expected oversized parse rejection, got: {err:?}"
        );

        // serialize_bounded must also refuse oversized JSON before persistence.
        let mut huge_ops = Vec::new();
        // Build ops until serialized checkpoint exceeds the byte cap.
        for i in 0..512 {
            huge_ops.push(sample_checkpoint_op(&format!(
                "op-pad-{i}-{}",
                "y".repeat(512)
            )));
        }
        let huge = ProgressiveOpsCheckpoint::new("attempt-huge", huge_ops, false);
        let ser_err = huge.serialize_bounded().unwrap_err();
        assert!(
            matches!(
                ser_err,
                DbError::Invalid(ref msg) if msg.contains("checkpoint_too_large")
            ),
            "expected oversized serialize rejection, got: {ser_err:?}"
        );
    }
}
