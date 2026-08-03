//! Per-conversation agent request queue.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::limits::MAX_QUEUED_TURNS;
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

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
                    started_at, finished_at, error_message
             FROM agent_request_queue WHERE id = ?1",
            [id],
            |row| {
                let prompt_json: String = row.get(4)?;
                Ok(QueueItem {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    priority: row.get(2)?,
                    status: row.get(3)?,
                    prompt: serde_json::from_str(&prompt_json).unwrap_or(Value::Null),
                    created_at: row.get(5)?,
                    started_at: row.get(6)?,
                    finished_at: row.get(7)?,
                    error_message: row.get(8)?,
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

/// Return an activated item to `queued` so a later drain can retry (e.g. lost a
/// race with a live turn).
pub fn requeue(db: &mut Database, id: &str) -> DbResult<QueueItem> {
    let n = db.conn().execute(
        "UPDATE agent_request_queue
         SET status = 'queued', started_at = NULL
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
    db.conn().execute(
        "UPDATE agent_request_queue SET status = 'cancelled', finished_at = ?1 WHERE id = ?2
         AND status IN ('queued', 'active')",
        params![now, id],
    )?;
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

/// Recover stale active items after crash (mark failed so queue can proceed).
pub fn recover_stale_active(db: &mut Database) -> DbResult<u64> {
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE agent_request_queue SET status = 'failed', finished_at = ?1,
         error_message = 'recovered after interruption' WHERE status = 'active'",
        [now],
    )?;
    Ok(n as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, DEFAULT_WORKSPACE_ID};
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

        let again = activate_next(&mut db, &conv.id).unwrap().expect("after requeue");
        assert_eq!(again.id, a.id);
    }
}
