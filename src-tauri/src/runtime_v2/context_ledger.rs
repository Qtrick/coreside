//! Model-context ledger with project isolation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::limits::MAX_CONTEXT_LEDGER_PER_CONVERSATION;
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextLedgerEntry {
    pub id: String,
    pub conversation_id: String,
    pub project_id: Option<String>,
    pub branch_id: Option<String>,
    pub entry_type: String,
    pub visibility: String,
    pub payload: Value,
    pub summary: String,
    pub expiration_class: String,
    pub created_at: String,
}

pub fn append_ledger_entry(
    db: &mut Database,
    conversation_id: &str,
    project_id: Option<&str>,
    branch_id: Option<&str>,
    entry_type: &str,
    visibility: &str,
    payload: &Value,
    summary: &str,
    expiration_class: Option<&str>,
) -> DbResult<ContextLedgerEntry> {
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM context_ledger_entries WHERE conversation_id = ?1",
        [conversation_id],
        |r| r.get(0),
    )?;
    if count as usize >= MAX_CONTEXT_LEDGER_PER_CONVERSATION {
        return Err(DbError::Invalid(format!(
            "context ledger limit reached: max {MAX_CONTEXT_LEDGER_PER_CONVERSATION}"
        )));
    }
    let id = format!("ledger-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let payload_json = payload.to_string();
    let exp = expiration_class.unwrap_or("session");
    db.conn().execute(
        "INSERT INTO context_ledger_entries (
            id, conversation_id, project_id, branch_id, entry_type, visibility,
            payload_json, summary, expiration_class, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            conversation_id,
            project_id,
            branch_id,
            entry_type,
            visibility,
            payload_json,
            summary,
            exp,
            now
        ],
    )?;
    get_ledger_entry(db, &id)
}

pub fn get_ledger_entry(db: &Database, id: &str) -> DbResult<ContextLedgerEntry> {
    db.conn()
        .query_row(
            "SELECT id, conversation_id, project_id, branch_id, entry_type, visibility,
                    payload_json, summary, expiration_class, created_at
             FROM context_ledger_entries WHERE id = ?1",
            [id],
            parse_ledger_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("ledger entry {id}")),
            other => DbError::Sqlite(other),
        })
}

pub fn list_ledger_entries(
    db: &Database,
    conversation_id: &str,
    project_id: Option<&str>,
    limit: usize,
) -> DbResult<Vec<ContextLedgerEntry>> {
    let mut out = Vec::new();
    if let Some(pid) = project_id {
        let mut stmt = db.conn().prepare(
            "SELECT id, conversation_id, project_id, branch_id, entry_type, visibility,
                    payload_json, summary, expiration_class, created_at
             FROM context_ledger_entries
             WHERE conversation_id = ?1 AND project_id = ?2
             ORDER BY created_at DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![conversation_id, pid, limit as i64], |row| {
            parse_ledger_row(row)
        })?;
        for r in rows {
            out.push(r.map_err(DbError::Sqlite)?);
        }
    } else {
        let mut stmt = db.conn().prepare(
            "SELECT id, conversation_id, project_id, branch_id, entry_type, visibility,
                    payload_json, summary, expiration_class, created_at
             FROM context_ledger_entries
             WHERE conversation_id = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![conversation_id, limit as i64], |row| {
            parse_ledger_row(row)
        })?;
        for r in rows {
            out.push(r.map_err(DbError::Sqlite)?);
        }
    }
    Ok(out)
}

fn parse_ledger_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContextLedgerEntry> {
    let payload_json: String = row.get(6)?;
    Ok(ContextLedgerEntry {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        project_id: row.get(2)?,
        branch_id: row.get(3)?,
        entry_type: row.get(4)?,
        visibility: row.get(5)?,
        payload: serde_json::from_str(&payload_json).unwrap_or(Value::Null),
        summary: row.get(7)?,
        expiration_class: row.get(8)?,
        created_at: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn test_db() -> crate::db::Database {
        let dir = tempdir().unwrap();
        crate::db::Database::open_path(&dir.path().join("t.db")).unwrap()
    }

    #[test]
    fn context_ledger_project_isolation() {
        let mut db = test_db();
        append_ledger_entry(
            &mut db,
            "conv-1",
            Some("proj-a"),
            None,
            "note",
            "model_context_only",
            &json!({"k": 1}),
            "a",
            None,
        )
        .unwrap();
        append_ledger_entry(
            &mut db,
            "conv-1",
            Some("proj-b"),
            None,
            "note",
            "model_context_only",
            &json!({"k": 2}),
            "b",
            None,
        )
        .unwrap();
        let a = list_ledger_entries(&db, "conv-1", Some("proj-a"), 10).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].summary, "a");
        let all = list_ledger_entries(&db, "conv-1", None, 10).unwrap();
        assert_eq!(all.len(), 2);
    }
}
