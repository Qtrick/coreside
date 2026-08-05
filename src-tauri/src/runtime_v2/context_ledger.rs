//! Model-context ledger with project, branch, expiration, and consumption isolation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::limits::MAX_CONTEXT_LEDGER_PER_CONVERSATION;
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

/// Maximum lengths and payload sizes enforced at append.
pub const MAX_LEDGER_CONVERSATION_ID_LEN: usize = 128;
pub const MAX_LEDGER_PROJECT_ID_LEN: usize = 128;
pub const MAX_LEDGER_BRANCH_ID_LEN: usize = 128;
pub const MAX_LEDGER_ENTRY_TYPE_LEN: usize = 64;
pub const MAX_LEDGER_VISIBILITY_LEN: usize = 64;
pub const MAX_LEDGER_SUMMARY_CHARS: usize = 500;
pub const MAX_LEDGER_PAYLOAD_BYTES: usize = 16_384;
/// Session-scoped entries expire after this many seconds from created_at.
pub const SESSION_EXPIRATION_SECS: i64 = 24 * 60 * 60;
pub const MAX_CONTEXT_LEDGER_PER_BRANCH: usize = 100;

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
    #[serde(default)]
    pub consumed_at: Option<String>,
    #[serde(default)]
    pub consumed_by_message_id: Option<String>,
}

/// Normalize submission IDs exactly once: entry ids already use `ledger-` prefix.
pub fn ledger_submission_id(entry_id: &str) -> String {
    let id = entry_id.trim();
    if id.is_empty() {
        return String::new();
    }
    if id.starts_with("ledger-") {
        id.to_string()
    } else {
        format!("ledger-{id}")
    }
}

fn normalize_expiration_class(raw: Option<&str>, entry_type: &str) -> String {
    let lower = raw.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("");
    match lower {
        "single_use" | "once" => "single_use".into(),
        "reusable" | "persistent" => "reusable".into(),
        "branch" => "branch".into(),
        "session" => "session".into(),
        "" if entry_type.contains("form_submit") => "single_use".into(),
        "" => "session".into(),
        other => other.to_string(),
    }
}

fn is_expired(expiration_class: &str, created_at: &str, now: &str) -> bool {
    match expiration_class {
        "single_use" | "reusable" | "branch" => false,
        "session" => parse_rfc3339_secs(created_at)
            .zip(parse_rfc3339_secs(now))
            .is_some_and(|(created, current)| current - created > SESSION_EXPIRATION_SECS),
        _ => {
            // Unknown classes: apply session TTL fail-closed for injection safety.
            parse_rfc3339_secs(created_at)
                .zip(parse_rfc3339_secs(now))
                .is_some_and(|(created, current)| current - created > SESSION_EXPIRATION_SECS)
        }
    }
}

fn parse_rfc3339_secs(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

fn validate_append_bounds(
    conversation_id: &str,
    project_id: Option<&str>,
    branch_id: Option<&str>,
    entry_type: &str,
    visibility: &str,
    payload: &Value,
    summary: &str,
) -> DbResult<()> {
    if conversation_id.trim().is_empty() || conversation_id.len() > MAX_LEDGER_CONVERSATION_ID_LEN {
        return Err(DbError::Invalid("invalid conversation id for ledger".into()));
    }
    if let Some(pid) = project_id {
        if pid.trim().is_empty() || pid.len() > MAX_LEDGER_PROJECT_ID_LEN {
            return Err(DbError::Invalid("invalid project id for ledger".into()));
        }
    }
    if let Some(bid) = branch_id {
        if bid.trim().is_empty() || bid.len() > MAX_LEDGER_BRANCH_ID_LEN {
            return Err(DbError::Invalid("invalid branch id for ledger".into()));
        }
    }
    if entry_type.trim().is_empty() || entry_type.len() > MAX_LEDGER_ENTRY_TYPE_LEN {
        return Err(DbError::Invalid("invalid ledger entry type".into()));
    }
    if visibility.trim().is_empty() || visibility.len() > MAX_LEDGER_VISIBILITY_LEN {
        return Err(DbError::Invalid("invalid ledger visibility".into()));
    }
    if summary.chars().count() > MAX_LEDGER_SUMMARY_CHARS {
        return Err(DbError::Invalid("ledger summary too long".into()));
    }
    let payload_bytes = serde_json::to_vec(payload).map(|b| b.len()).unwrap_or(usize::MAX);
    if payload_bytes > MAX_LEDGER_PAYLOAD_BYTES {
        return Err(DbError::Invalid(format!(
            "ledger payload too large: max {MAX_LEDGER_PAYLOAD_BYTES} bytes"
        )));
    }
    Ok(())
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
    validate_append_bounds(
        conversation_id,
        project_id,
        branch_id,
        entry_type,
        visibility,
        payload,
        summary,
    )?;
    let exp = normalize_expiration_class(expiration_class, entry_type);
    let id = format!("ledger-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let payload_json = payload.to_string();

    // Cap enforcement must be transactional to avoid count-then-insert races.
    let tx = db.conn().unchecked_transaction()?;
    let count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM context_ledger_entries WHERE conversation_id = ?1",
        [conversation_id],
        |r| r.get(0),
    )?;
    if count as usize >= MAX_CONTEXT_LEDGER_PER_CONVERSATION {
        return Err(DbError::Invalid(format!(
            "context ledger limit reached: max {MAX_CONTEXT_LEDGER_PER_CONVERSATION}"
        )));
    }
    if let Some(bid) = branch_id {
        let branch_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM context_ledger_entries
             WHERE conversation_id = ?1 AND branch_id = ?2",
            params![conversation_id, bid],
            |r| r.get(0),
        )?;
        if branch_count as usize >= MAX_CONTEXT_LEDGER_PER_BRANCH {
            return Err(DbError::Invalid(format!(
                "context ledger branch limit reached: max {MAX_CONTEXT_LEDGER_PER_BRANCH}"
            )));
        }
    }
    tx.execute(
        "INSERT INTO context_ledger_entries (
            id, conversation_id, project_id, branch_id, entry_type, visibility,
            payload_json, summary, expiration_class, created_at,
            consumed_at, consumed_by_message_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, NULL)",
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
    tx.commit()?;
    get_ledger_entry(db, &id)
}

pub fn get_ledger_entry(db: &Database, id: &str) -> DbResult<ContextLedgerEntry> {
    db.conn()
        .query_row(
            "SELECT id, conversation_id, project_id, branch_id, entry_type, visibility,
                    payload_json, summary, expiration_class, created_at,
                    consumed_at, consumed_by_message_id
             FROM context_ledger_entries WHERE id = ?1",
            [id],
            parse_ledger_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("ledger entry {id}")),
            other => DbError::Sqlite(other),
        })
}

/// List injectable ledger entries with privacy isolation.
///
/// - `Some(project_id)` → exact project rows only
/// - `None` → only explicitly unscoped rows (`project_id IS NULL`)
/// - Branch: matching `branch_id`, plus unbranched conversation-global rows
/// - Expired / consumed rows are excluded
pub fn list_ledger_entries(
    db: &Database,
    conversation_id: &str,
    project_id: Option<&str>,
    limit: usize,
) -> DbResult<Vec<ContextLedgerEntry>> {
    list_ledger_entries_for_inject(db, conversation_id, project_id, None, limit, true)
}

/// Full filter for model injection (branch + expiration + consumption).
pub fn list_ledger_entries_for_inject(
    db: &Database,
    conversation_id: &str,
    project_id: Option<&str>,
    branch_id: Option<&str>,
    limit: usize,
    exclude_unusable: bool,
) -> DbResult<Vec<ContextLedgerEntry>> {
    let mut out = Vec::new();
    let now = now_rfc3339();
    // Fetch a bounded surplus so post-filters still fill `limit`.
    let fetch_limit = (limit.saturating_mul(3).max(limit)).min(MAX_CONTEXT_LEDGER_PER_CONVERSATION);

    let rows: Vec<ContextLedgerEntry> = if let Some(pid) = project_id {
        let mut stmt = db.conn().prepare(
            "SELECT id, conversation_id, project_id, branch_id, entry_type, visibility,
                    payload_json, summary, expiration_class, created_at,
                    consumed_at, consumed_by_message_id
             FROM context_ledger_entries
             WHERE conversation_id = ?1 AND project_id = ?2
             ORDER BY created_at DESC LIMIT ?3",
        )?;
        let mapped = stmt.query_map(params![conversation_id, pid, fetch_limit as i64], |row| {
            parse_ledger_row(row)
        })?;
        mapped
            .collect::<Result<Vec<_>, _>>()
            .map_err(DbError::Sqlite)?
    } else {
        // Privacy: None means unscoped rows only — never every project for the conversation.
        let mut stmt = db.conn().prepare(
            "SELECT id, conversation_id, project_id, branch_id, entry_type, visibility,
                    payload_json, summary, expiration_class, created_at,
                    consumed_at, consumed_by_message_id
             FROM context_ledger_entries
             WHERE conversation_id = ?1 AND project_id IS NULL
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let mapped = stmt.query_map(params![conversation_id, fetch_limit as i64], |row| {
            parse_ledger_row(row)
        })?;
        mapped
            .collect::<Result<Vec<_>, _>>()
            .map_err(DbError::Sqlite)?
    };

    for entry in rows {
        if exclude_unusable {
            if entry.consumed_at.is_some() {
                continue;
            }
            if is_expired(&entry.expiration_class, &entry.created_at, &now) {
                continue;
            }
            if !branch_allows(branch_id, entry.branch_id.as_deref()) {
                continue;
            }
        }
        out.push(entry);
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

/// Unbranched entries are conversation-global within the project filter.
/// Branch-scoped entries require an exact active-branch match.
fn branch_allows(active_branch: Option<&str>, entry_branch: Option<&str>) -> bool {
    match (active_branch, entry_branch) {
        (_, None) => true, // conversation-global
        (Some(active), Some(entry)) => active == entry,
        (None, Some(_)) => false, // branch-scoped must not leak onto unbranched main
    }
}

/// Mark a single-use (or form_submit) entry consumed after durable turn success.
pub fn mark_ledger_consumed(
    db: &mut Database,
    entry_id: &str,
    message_id: Option<&str>,
) -> DbResult<()> {
    let now = now_rfc3339();
    let updated = db.conn().execute(
        "UPDATE context_ledger_entries
         SET consumed_at = ?2, consumed_by_message_id = ?3
         WHERE id = ?1 AND consumed_at IS NULL",
        params![entry_id, now, message_id],
    )?;
    if updated == 0 {
        // Already consumed or missing — not an error for idempotent inject.
        return Ok(());
    }
    Ok(())
}

/// Whether this expiration class should be consumed after one successful inject.
pub fn should_consume_after_inject(expiration_class: &str, entry_type: &str) -> bool {
    expiration_class == "single_use"
        || expiration_class == "once"
        || entry_type.contains("form_submit")
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
        consumed_at: row.get(10)?,
        consumed_by_message_id: row.get(11)?,
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
        append_ledger_entry(
            &mut db,
            "conv-1",
            None,
            None,
            "note",
            "model_context_only",
            &json!({"k": 3}),
            "unscoped",
            None,
        )
        .unwrap();

        let a = list_ledger_entries(&db, "conv-1", Some("proj-a"), 10).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].summary, "a");

        let b = list_ledger_entries(&db, "conv-1", Some("proj-b"), 10).unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].summary, "b");

        // None must NOT return project-scoped rows (privacy).
        let unscoped = list_ledger_entries(&db, "conv-1", None, 10).unwrap();
        assert_eq!(unscoped.len(), 1);
        assert_eq!(unscoped[0].summary, "unscoped");
        assert!(unscoped[0].project_id.is_none());
    }

    #[test]
    fn branch_isolation_and_unbranched_global() {
        let mut db = test_db();
        append_ledger_entry(
            &mut db,
            "conv-1",
            Some("proj-a"),
            Some("branch-1"),
            "note",
            "model_context_only",
            &json!({}),
            "b1",
            Some("branch"),
        )
        .unwrap();
        append_ledger_entry(
            &mut db,
            "conv-1",
            Some("proj-a"),
            Some("branch-2"),
            "note",
            "model_context_only",
            &json!({}),
            "b2",
            Some("branch"),
        )
        .unwrap();
        append_ledger_entry(
            &mut db,
            "conv-1",
            Some("proj-a"),
            None,
            "note",
            "model_context_only",
            &json!({}),
            "global",
            Some("reusable"),
        )
        .unwrap();

        let on_b1 =
            list_ledger_entries_for_inject(&db, "conv-1", Some("proj-a"), Some("branch-1"), 10, true)
                .unwrap();
        assert_eq!(on_b1.len(), 2);
        assert!(on_b1.iter().any(|e| e.summary == "b1"));
        assert!(on_b1.iter().any(|e| e.summary == "global"));
        assert!(!on_b1.iter().any(|e| e.summary == "b2"));

        let main =
            list_ledger_entries_for_inject(&db, "conv-1", Some("proj-a"), None, 10, true).unwrap();
        assert_eq!(main.len(), 1);
        assert_eq!(main[0].summary, "global");
    }

    #[test]
    fn single_use_consumed_not_reinjected() {
        let mut db = test_db();
        let entry = append_ledger_entry(
            &mut db,
            "conv-1",
            Some("proj-a"),
            None,
            "form_submit",
            "model_context_only",
            &json!({"values": {"q": 1}}),
            "form",
            Some("single_use"),
        )
        .unwrap();
        assert_eq!(entry.expiration_class, "single_use");
        assert!(entry.id.starts_with("ledger-"));

        let before =
            list_ledger_entries_for_inject(&db, "conv-1", Some("proj-a"), None, 10, true).unwrap();
        assert_eq!(before.len(), 1);

        mark_ledger_consumed(&mut db, &entry.id, Some("msg-1")).unwrap();
        let after =
            list_ledger_entries_for_inject(&db, "conv-1", Some("proj-a"), None, 10, true).unwrap();
        assert!(after.is_empty());
    }

    #[test]
    fn submission_id_normalized_once() {
        assert_eq!(ledger_submission_id("ledger-abc"), "ledger-abc");
        assert_eq!(ledger_submission_id("abc"), "ledger-abc");
        assert_eq!(ledger_submission_id("  ledger-x  "), "ledger-x");
    }

    #[test]
    fn rejects_oversized_payload() {
        let mut db = test_db();
        let big = "x".repeat(MAX_LEDGER_PAYLOAD_BYTES + 1);
        let err = append_ledger_entry(
            &mut db,
            "conv-1",
            None,
            None,
            "note",
            "model_context_only",
            &json!({ "blob": big }),
            "big",
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("too large"));
    }

    #[test]
    fn no_cross_conversation_injection() {
        let mut db = test_db();
        append_ledger_entry(
            &mut db,
            "conv-a",
            Some("proj-a"),
            None,
            "note",
            "model_context_only",
            &json!({}),
            "secret-a",
            None,
        )
        .unwrap();
        let other = list_ledger_entries(&db, "conv-b", Some("proj-a"), 10).unwrap();
        assert!(other.is_empty());
    }
}
