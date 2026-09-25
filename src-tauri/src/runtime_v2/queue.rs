//! Per-conversation agent request queue.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::limits::MAX_QUEUED_TURNS;
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: String,
    pub conversation_id: String,
    pub priority: i64,
    pub status: String,
    pub prompt: Value,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
    pub turn_id: Option<String>,
}

pub fn enqueue(
    db: &mut Database,
    conversation_id: &str,
    prompt: &Value,
    priority: i64,
) -> DbResult<QueueItem> {
    let queued_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM agent_request_queue
         WHERE conversation_id = ?1 AND status IN ('queued', 'active')",
        [conversation_id],
        |r| r.get(0),
    )?;
    if queued_count as usize >= MAX_QUEUED_TURNS {
        return Err(DbError::Invalid(format!(
            "queue full: max {MAX_QUEUED_TURNS} turns per conversation"
        )));
    }
    let id = format!("q-{}", Uuid::new_v4());
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO agent_request_queue (id, conversation_id, priority, status, prompt_json, created_at)
         VALUES (?1, ?2, ?3, 'queued', ?4, ?5)",
        params![id, conversation_id, priority, prompt.to_string(), now],
    )?;
    get_item(db, &id)
}

pub fn get_item(db: &Database, id: &str) -> DbResult<QueueItem> {
    db.conn()
        .query_row(
            "SELECT id, conversation_id, priority, status, prompt_json, created_at,
                    started_at, finished_at, error_message, turn_id
             FROM agent_request_queue WHERE id = ?1",
            [id],
            |row| {
                let prompt_json: String = row.get(4)?;
                Ok(QueueItem {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    priority: row.get(2)?,
                    status: row.get(3)?,
                    prompt: serde_json::from_str(&prompt_json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                    created_at: row.get(5)?,
                    started_at: row.get(6)?,
                    finished_at: row.get(7)?,
                    error_message: row.get(8)?,
                    turn_id: row.get(9)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("queue item {id}")),
            other => DbError::Sqlite(other),
        })
}

pub fn list_queue(db: &Database, conversation_id: &str) -> DbResult<Vec<QueueItem>> {
    let mut stmt = db.conn().prepare(
        "SELECT id FROM agent_request_queue
         WHERE conversation_id = ?1 AND status IN ('queued', 'active')
         ORDER BY priority ASC, created_at ASC",
    )?;
    let ids: Vec<String> = stmt
        .query_map([conversation_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter().map(|id| get_item(db, &id)).collect()
}

pub fn activate_next(db: &mut Database, conversation_id: &str) -> DbResult<Option<QueueItem>> {
    // Single atomic UPDATE so concurrent drainers cannot activate two items (or the
    // same item twice) after both observe an empty active set.
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE agent_request_queue
         SET status = 'active', started_at = ?1
         WHERE id = (
           SELECT q.id FROM agent_request_queue q
           WHERE q.conversation_id = ?2
             AND q.status = 'queued'
             AND NOT EXISTS (
               SELECT 1 FROM agent_request_queue a
               WHERE a.conversation_id = ?2 AND a.status = 'active'
             )
           ORDER BY q.priority ASC, q.created_at ASC
           LIMIT 1
         )",
        params![now, conversation_id],
    )?;
    if n == 0 {
        return Ok(None);
    }
    let id: String = db.conn().query_row(
        "SELECT id FROM agent_request_queue
         WHERE conversation_id = ?1 AND status = 'active' AND started_at = ?2
         LIMIT 1",
        params![conversation_id, now],
        |r| r.get(0),
    )?;
    Ok(Some(get_item(db, &id)?))
}

/// Explicitly bind an active queue item to its corresponding turn journal entry.
pub fn bind_turn(db: &mut Database, queue_item_id: &str, turn_id: &str) -> DbResult<QueueItem> {
    let n = db.conn().execute(
        "UPDATE agent_request_queue SET turn_id = ?1 WHERE id = ?2 AND status = 'active'",
        params![turn_id, queue_item_id],
    )?;
    if n == 0 {
        return Err(DbError::Invalid(format!(
            "queue item {queue_item_id} is not active and cannot be bound to turn {turn_id}"
        )));
    }
    get_item(db, queue_item_id)
}

/// Return an activated item to `queued` so a later drain can retry (e.g. lost a
/// race with a live turn).
pub fn requeue(db: &mut Database, id: &str) -> DbResult<QueueItem> {
    let n = db.conn().execute(
        "UPDATE agent_request_queue
         SET status = 'queued', started_at = NULL, turn_id = NULL
         WHERE id = ?1 AND status = 'active'",
        params![id],
    )?;
    if n == 0 {
        return Err(DbError::Invalid(format!(
            "queue item {id} is not active and cannot be requeued"
        )));
    }
    get_item(db, id)
}

pub fn complete(db: &mut Database, id: &str, error: Option<&str>) -> DbResult<QueueItem> {
    let now = now_rfc3339();
    let status = if error.is_some() {
        "failed"
    } else {
        "completed"
    };
    db.conn().execute(
        "UPDATE agent_request_queue SET status = ?1, finished_at = ?2, error_message = ?3 WHERE id = ?4",
        params![status, now, error, id],
    )?;
    get_item(db, id)
}

pub fn cancel(db: &mut Database, id: &str) -> DbResult<QueueItem> {
    let now = now_rfc3339();
    // Only queued items: cancelling an active item races with drain/bind and can
    // release staged attachments that the live turn already claimed.
    let n = db.conn().execute(
        "UPDATE agent_request_queue SET status = 'cancelled', finished_at = ?1 WHERE id = ?2
         AND status = 'queued'",
        params![now, id],
    )?;
    if n == 0 {
        let item = get_item(db, id)?;
        if item.status == "cancelled" {
            return Ok(item);
        }
        return Err(DbError::Invalid(
            "only queued turns can be cancelled".into(),
        ));
    }
    get_item(db, id)
}

pub fn remove_queued(db: &mut Database, id: &str) -> DbResult<()> {
    let n = db.conn().execute(
        "DELETE FROM agent_request_queue WHERE id = ?1 AND status = 'queued'",
        [id],
    )?;
    if n == 0 {
        return Err(DbError::Invalid(
            "only queued (not active) items can be removed".into(),
        ));
    }
    Ok(())
}

/// Recover stale active items after crash by reconciling against the turn journal:
/// - if turn_id is bound:
///   - if turn reached committed/published: mark queue item completed (prevents duplicate execution)
///   - if turn was only created/claimed: requeue so turn can be safely processed
///   - otherwise (started/streaming/failed/aborted): mark failed after interruption
/// - if turn_id is NOT bound:
///   - the process crashed before any turn was created/claimed; safely requeue so the user prompt is not dropped
pub fn recover_stale_active(db: &mut Database) -> DbResult<u64> {
    let now = now_rfc3339();
    let mut stmt = db.conn().prepare(
        "SELECT id, conversation_id, turn_id FROM agent_request_queue WHERE status = 'active'",
    )?;
    let active_items: Vec<(String, String, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut recovered = 0u64;
    for (item_id, _conv_id, maybe_turn_id) in active_items {
        if let Some(ref turn_id) = maybe_turn_id {
            // Check linked turn journal explicitly by turn_id, NOT by conversation-wide latest!
            let turn_state: Option<String> = db
                .conn()
                .query_row(
                    "SELECT state FROM turn_journal WHERE id = ?1",
                    [turn_id],
                    |r| r.get(0),
                )
                .optional()?;

            match turn_state.as_deref() {
                Some("committed") | Some("published") => {
                    // The turn committed successfully before restart; mark completed
                    db.conn().execute(
                        "UPDATE agent_request_queue SET status = 'completed', finished_at = ?1 WHERE id = ?2 AND status = 'active'",
                        params![now, item_id],
                    )?;
                    recovered += 1;
                }
                Some("created") | Some("claimed") => {
                    // The turn was only claimed or created but provider never started; safe to requeue
                    db.conn().execute(
                        "UPDATE agent_request_queue SET status = 'queued', started_at = NULL, turn_id = NULL WHERE id = ?1 AND status = 'active'",
                        params![item_id],
                    )?;
                    recovered += 1;
                }
                _ => {
                    // Provider was started / streaming / failed or turn journal absent; mark failed with reason
                    db.conn().execute(
                        "UPDATE agent_request_queue SET status = 'failed', finished_at = ?1, error_message = 'recovered after interruption' WHERE id = ?2 AND status = 'active'",
                        params![now, item_id],
                    )?;
                    recovered += 1;
                }
            }
        } else {
            // Queue item was active, but process crashed BEFORE any turn was created or claimed.
            // Provider was never invoked for this item; safe to requeue so the user prompt is not lost.
            db.conn().execute(
                "UPDATE agent_request_queue SET status = 'queued', started_at = NULL, turn_id = NULL WHERE id = ?1 AND status = 'active'",
                params![item_id],
            )?;
            recovered += 1;
        }
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, DEFAULT_WORKSPACE_ID};
    use crate::runtime_v2::turn_journal::{create_turn, transition_turn, TurnPatch, TurnState};
    use serde_json::json;
    use tempfile::tempdir;

    fn test_db() -> Database {
        let dir = tempdir().unwrap();
        Database::open_path(&dir.path().join("q.db")).unwrap()
    }

    #[test]
    fn activate_next_is_exclusive_and_requeue_restores_queued() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Queue", None).unwrap();
        let a = enqueue(&mut db, &conv.id, &json!({"content": "one"}), 100).unwrap();
        let _b = enqueue(&mut db, &conv.id, &json!({"content": "two"}), 100).unwrap();

        let first = activate_next(&mut db, &conv.id).unwrap().expect("first");
        assert_eq!(first.id, a.id);
        assert_eq!(first.status, "active");

        // Second activate must not promote another item while one is active.
        assert!(activate_next(&mut db, &conv.id).unwrap().is_none());

        let restored = requeue(&mut db, &a.id).unwrap();
        assert_eq!(restored.status, "queued");

        let again = activate_next(&mut db, &conv.id)
            .unwrap()
            .expect("after requeue");
        assert_eq!(again.id, a.id);
    }

    #[test]
    fn cancel_rejects_active_items() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Queue", None).unwrap();
        let a = enqueue(&mut db, &conv.id, &json!({"content": "one"}), 100).unwrap();
        let active = activate_next(&mut db, &conv.id).unwrap().expect("active");
        assert_eq!(active.id, a.id);
        let err = cancel(&mut db, &a.id).unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
        let queued = enqueue(&mut db, &conv.id, &json!({"content": "two"}), 100).unwrap();
        let cancelled = cancel(&mut db, &queued.id).unwrap();
        assert_eq!(cancelled.status, "cancelled");
    }

    #[test]
    fn bind_turn_updates_active_item_and_requeue_clears_turn_id() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Queue", None).unwrap();
        let _item = enqueue(&mut db, &conv.id, &json!({"content": "task"}), 100).unwrap();

        let active = activate_next(&mut db, &conv.id).unwrap().expect("active");
        let turn = create_turn(&db, &conv.id, None, "idem-bind", Some("interactive")).unwrap();
        let bound = bind_turn(&mut db, &active.id, &turn.id).unwrap();
        assert_eq!(bound.turn_id, Some(turn.id.clone()));

        // Requeue clears turn_id so subsequent re-execution starts fresh
        let requeued = requeue(&mut db, &active.id).unwrap();
        assert_eq!(requeued.turn_id, None);
    }

    #[test]
    fn recover_stale_active_without_turn_requeues_even_if_older_turn_committed() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Queue", None).unwrap();

        // 1. Prior turn in same conversation was committed
        let t1 = create_turn(&db, &conv.id, None, "idem-1", Some("interactive")).unwrap();
        transition_turn(
            &db,
            &t1.id,
            &t1.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &t1.id,
            &t1.attempt_id,
            TurnState::ProviderStarted,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &t1.id,
            &t1.attempt_id,
            TurnState::TypedTerminal,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &t1.id,
            &t1.attempt_id,
            TurnState::Finalizing,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &t1.id,
            &t1.attempt_id,
            TurnState::Committed,
            TurnPatch::default(),
        )
        .unwrap();

        // 2. User enqueued a second turn, which activated, but process crashed BEFORE turn creation
        let item2 = enqueue(
            &mut db,
            &conv.id,
            &json!({"content": "second request"}),
            100,
        )
        .unwrap();
        let active2 = activate_next(&mut db, &conv.id).unwrap().expect("active");
        assert_eq!(active2.id, item2.id);
        assert_eq!(active2.turn_id, None);

        // 3. Stale recovery: previously would falsely inspect latest turn (t1) and mark item2 "completed"!
        // With our fix, since item2 has no turn_id, it is safely requeued so user request is NOT lost.
        let recovered_count = recover_stale_active(&mut db).unwrap();
        assert_eq!(recovered_count, 1);

        let item2_recovered = get_item(&db, &item2.id).unwrap();
        assert_eq!(item2_recovered.status, "queued");
        assert_eq!(item2_recovered.started_at, None);
    }

    #[test]
    fn recover_stale_active_with_committed_turn_completes_queue_item() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Queue", None).unwrap();

        let _item = enqueue(&mut db, &conv.id, &json!({"content": "task"}), 100).unwrap();
        let active = activate_next(&mut db, &conv.id).unwrap().expect("active");

        let turn = create_turn(&db, &conv.id, None, "idem-active", Some("interactive")).unwrap();
        bind_turn(&mut db, &active.id, &turn.id).unwrap();

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
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::TypedTerminal,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Finalizing,
            TurnPatch::default(),
        )
        .unwrap();
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Committed,
            TurnPatch::default(),
        )
        .unwrap();

        let recovered = recover_stale_active(&mut db).unwrap();
        assert_eq!(recovered, 1);

        let item_recovered = get_item(&db, &active.id).unwrap();
        assert_eq!(item_recovered.status, "completed");
    }

    #[test]
    fn recover_stale_active_with_claimed_turn_requeues_queue_item() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Queue", None).unwrap();

        let _item = enqueue(&mut db, &conv.id, &json!({"content": "task"}), 100).unwrap();
        let active = activate_next(&mut db, &conv.id).unwrap().expect("active");

        let turn = create_turn(&db, &conv.id, None, "idem-claimed", Some("interactive")).unwrap();
        bind_turn(&mut db, &active.id, &turn.id).unwrap();

        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Claimed,
            TurnPatch::default(),
        )
        .unwrap();

        let recovered = recover_stale_active(&mut db).unwrap();
        assert_eq!(recovered, 1);

        let item_recovered = get_item(&db, &active.id).unwrap();
        assert_eq!(item_recovered.status, "queued");
        assert_eq!(item_recovered.turn_id, None);
    }

    #[test]
    fn recover_stale_active_with_streaming_turn_marks_failed() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Queue", None).unwrap();

        let _item = enqueue(&mut db, &conv.id, &json!({"content": "task"}), 100).unwrap();
        let active = activate_next(&mut db, &conv.id).unwrap().expect("active");

        let turn = create_turn(&db, &conv.id, None, "idem-stream", Some("interactive")).unwrap();
        bind_turn(&mut db, &active.id, &turn.id).unwrap();

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
        transition_turn(
            &db,
            &turn.id,
            &turn.attempt_id,
            TurnState::Streaming,
            TurnPatch::default(),
        )
        .unwrap();

        let recovered = recover_stale_active(&mut db).unwrap();
        assert_eq!(recovered, 1);

        let item_recovered = get_item(&db, &active.id).unwrap();
        assert_eq!(item_recovered.status, "failed");
        assert_eq!(
            item_recovered.error_message,
            Some("recovered after interruption".to_string())
        );
    }
}
