//! Redacted turn timeline for read-only replay (never re-execute).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

/// Allowed timeline kinds (must match migration CHECK).
pub const TIMELINE_KINDS: &[&str] = &[
    "user_request",
    "provider_start",
    "text_checkpoint",
    "operation_received",
    "operation_accepted",
    "operation_rejected",
    "preview_update",
    "approval",
    "commit",
    "failure",
    "cancellation",
    "completion",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnTimelineEvent {
    pub id: String,
    pub conversation_id: String,
    pub turn_id: String,
    pub sequence: i64,
    pub kind: String,
    pub redacted_payload: Value,
    pub created_at: String,
}

fn next_sequence(db: &Database, conversation_id: &str, turn_id: &str) -> DbResult<i64> {
    let max: i64 = db.conn().query_row(
        "SELECT COALESCE(MAX(sequence), 0) FROM turn_timeline_events
         WHERE conversation_id = ?1 AND turn_id = ?2",
        params![conversation_id, turn_id],
        |r| r.get(0),
    )?;
    Ok(max.saturating_add(1))
}

/// Append a redacted timeline event. Caller must never pass secrets/prompts/raw bodies.
/// Defense in depth: pattern-redact the JSON before persist (never store raw keys).
pub fn append_turn_timeline_event(
    db: &Database,
    conversation_id: &str,
    turn_id: &str,
    kind: &str,
    redacted_payload: &Value,
) -> DbResult<TurnTimelineEvent> {
    if !TIMELINE_KINDS.contains(&kind) {
        return Err(DbError::Invalid(format!("unknown timeline kind: {kind}")));
    }
    let conversation_id = conversation_id.trim();
    let turn_id = turn_id.trim();
    if conversation_id.is_empty() || turn_id.is_empty() {
        return Err(DbError::Invalid(
            "conversation_id and turn_id are required".into(),
        ));
    }
    let sequence = next_sequence(db, conversation_id, turn_id)?;
    let id = format!("tle-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let payload_json =
        crate::security::redact_secrets(&redacted_payload.to_string(), None);
    let stored_payload: Value =
        serde_json::from_str(&payload_json).unwrap_or_else(|_| json!({}));
    db.conn().execute(
        "INSERT INTO turn_timeline_events (
            id, conversation_id, turn_id, sequence, kind, redacted_payload_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            conversation_id,
            turn_id,
            sequence,
            kind,
            payload_json,
            now
        ],
    )?;
    Ok(TurnTimelineEvent {
        id,
        conversation_id: conversation_id.to_string(),
        turn_id: turn_id.to_string(),
        sequence,
        kind: kind.to_string(),
        redacted_payload: stored_payload,
        created_at: now,
    })
}

/// Best-effort append — never fails the turn. Logs and returns None on error.
pub fn try_append_turn_timeline_event(
    db: &Database,
    conversation_id: &str,
    turn_id: &str,
    kind: &str,
    redacted_payload: Value,
) {
    if let Err(err) = append_turn_timeline_event(
        db,
        conversation_id,
        turn_id,
        kind,
        &redacted_payload,
    ) {
        tracing::debug!(
            error = %err,
            conversation_id,
            turn_id,
            kind,
            "turn timeline append skipped"
        );
    }
}

pub fn list_turn_timeline_events(
    db: &Database,
    conversation_id: &str,
    turn_id: Option<&str>,
    limit: usize,
) -> DbResult<Vec<TurnTimelineEvent>> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err(DbError::Invalid("conversation_id is required".into()));
    }
    let limit = limit.clamp(1, 500) as i64;
    let mut out = Vec::new();
    if let Some(tid) = turn_id.map(str::trim).filter(|s| !s.is_empty()) {
        let mut stmt = db.conn().prepare(
            "SELECT id, conversation_id, turn_id, sequence, kind, redacted_payload_json, created_at
             FROM turn_timeline_events
             WHERE conversation_id = ?1 AND turn_id = ?2
             ORDER BY sequence ASC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![conversation_id, tid, limit], parse_row)?;
        for row in rows {
            out.push(row?);
        }
    } else {
        let mut stmt = db.conn().prepare(
            "SELECT id, conversation_id, turn_id, sequence, kind, redacted_payload_json, created_at
             FROM turn_timeline_events
             WHERE conversation_id = ?1
             ORDER BY created_at ASC, sequence ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![conversation_id, limit], parse_row)?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}

fn parse_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TurnTimelineEvent> {
    let payload_json: String = row.get(5)?;
    let redacted_payload = serde_json::from_str(&payload_json).unwrap_or_else(|_| json!({}));
    Ok(TurnTimelineEvent {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        turn_id: row.get(2)?,
        sequence: row.get(3)?,
        kind: row.get(4)?,
        redacted_payload,
        created_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn append_and_list_timeline_events() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("timeline.db")).unwrap();
        db.conn()
            .execute(
                "INSERT INTO conversations (id, workspace_id, title) VALUES ('c1', ?1, 't')",
                [crate::db::DEFAULT_WORKSPACE_ID],
            )
            .unwrap();

        append_turn_timeline_event(
            &db,
            "c1",
            "turn-1",
            "commit",
            &json!({ "summary": "Applied change" }),
        )
        .unwrap();
        append_turn_timeline_event(
            &db,
            "c1",
            "turn-1",
            "completion",
            &json!({}),
        )
        .unwrap();
        append_turn_timeline_event(
            &db,
            "c1",
            "turn-2",
            "failure",
            &json!({ "message": "provider error" }),
        )
        .unwrap();

        let for_turn = list_turn_timeline_events(&db, "c1", Some("turn-1"), 50).unwrap();
        assert_eq!(for_turn.len(), 2);
        assert_eq!(for_turn[0].kind, "commit");
        assert_eq!(for_turn[0].sequence, 1);
        assert_eq!(for_turn[1].kind, "completion");
        assert_eq!(for_turn[1].sequence, 2);

        let all = list_turn_timeline_events(&db, "c1", None, 50).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn rejects_unknown_kind() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("timeline-bad.db")).unwrap();
        db.conn()
            .execute(
                "INSERT INTO conversations (id, workspace_id, title) VALUES ('c1', ?1, 't')",
                [crate::db::DEFAULT_WORKSPACE_ID],
            )
            .unwrap();
        let err = append_turn_timeline_event(&db, "c1", "t1", "raw_provider", &json!({})).unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
    }

    #[test]
    fn persists_redacted_payload_not_raw_key_shapes() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("timeline-redact.db")).unwrap();
        db.conn()
            .execute(
                "INSERT INTO conversations (id, workspace_id, title) VALUES ('c1', ?1, 't')",
                [crate::db::DEFAULT_WORKSPACE_ID],
            )
            .unwrap();
        let ev = append_turn_timeline_event(
            &db,
            "c1",
            "t1",
            "failure",
            &json!({ "note": "Bearer sk-abcdefghijklmnopqrstuvwxyz0123456789" }),
        )
        .unwrap();
        let stored = ev.redacted_payload.to_string();
        assert!(
            !stored.contains("sk-abcdefghijklmnopqrstuvwxyz0123456789"),
            "raw key must not persist: {stored}"
        );
        assert!(stored.contains("[REDACTED]"), "expected redaction marker: {stored}");
    }
}
