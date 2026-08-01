//! Chat branching and read-only snapshots.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::surfaces::{list_inline_surfaces, SurfaceRecord};
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatBranchRecord {
    pub id: String,
    pub source_conversation_id: String,
    pub source_message_id: Option<String>,
    pub new_conversation_id: String,
    pub branch_name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRecord {
    pub id: String,
    pub conversation_id: String,
    pub project_id: Option<String>,
    pub description: String,
    pub payload: Value,
    pub created_at: String,
}

fn conversation_project_id(db: &Database, conversation_id: &str) -> Option<String> {
    db.conn()
        .query_row(
            "SELECT project_id FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
}

/// Branch a conversation from a message (inclusive of that message and earlier).
pub fn branch_from_message(
    db: &mut Database,
    source_conversation_id: &str,
    source_message_id: &str,
    branch_name: &str,
    workspace_id: &str,
) -> DbResult<(ChatBranchRecord, Vec<SurfaceRecord>)> {
    let existing = list_branches(db, source_conversation_id)?;
    // Depth is approximated by the number of branches already taken from this
    // conversation. A true tree depth would require walking parent links; this
    // ceiling still prevents unbounded branching from one chat.
    if existing.len() >= super::limits::MAX_BRANCH_DEPTH {
        return Err(DbError::Invalid(format!(
            "branch limit reached ({})",
            super::limits::MAX_BRANCH_DEPTH
        )));
    }

    let messages = crate::db::get_messages(db, source_conversation_id)?;
    let cut = messages
        .iter()
        .position(|m| m.id == source_message_id)
        .ok_or_else(|| DbError::NotFound(format!("message {source_message_id}")))?;
    let kept = &messages[..=cut];

    let title = if branch_name.trim().is_empty() {
        "Branch".to_string()
    } else {
        format!("{branch_name} (branch)")
    };
    let project_id = conversation_project_id(db, source_conversation_id);
    let new_conv = crate::db::create_conversation(db, workspace_id, &title, project_id.as_deref())?;

    for m in kept {
        crate::db::insert_message(db, &new_conv.id, &m.role, &m.content, m.metadata.as_ref())?;
    }

    let mut cloned_surfaces = Vec::new();
    for surface in list_inline_surfaces(db, source_conversation_id)? {
        let mut def = surface.definition.clone();
        if let Some(obj) = def.as_object_mut() {
            obj.insert("_branchedFrom".into(), json!(surface.instance_id));
        }
        let created = super::surfaces::create_inline_surface(
            db,
            &new_conv.id,
            None,
            surface.project_id.as_deref(),
            &surface.name,
            &def,
            &surface.capability_packs,
        )?;
        cloned_surfaces.push(created);
    }

    let id = format!("br-{}", Uuid::new_v4());
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO chat_branches (
            id, source_conversation_id, source_message_id, new_conversation_id, branch_name, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            source_conversation_id,
            source_message_id,
            new_conv.id,
            branch_name,
            now
        ],
    )?;

    Ok((
        ChatBranchRecord {
            id,
            source_conversation_id: source_conversation_id.into(),
            source_message_id: Some(source_message_id.into()),
            new_conversation_id: new_conv.id,
            branch_name: branch_name.into(),
            created_at: now,
        },
        cloned_surfaces,
    ))
}

pub fn create_snapshot(
    db: &mut Database,
    conversation_id: &str,
    project_id: Option<&str>,
    description: &str,
) -> DbResult<SnapshotRecord> {
    // Build the payload before mutating retention so a read failure cannot
    // delete older snapshots without saving a replacement.
    let messages = crate::db::get_messages(db, conversation_id)?;
    let surfaces = list_inline_surfaces(db, conversation_id)?;
    let txns = super::transactions::list_transactions(db, conversation_id, 100)?;
    let payload = json!({
        "conversationId": conversation_id,
        "projectId": project_id,
        "messages": messages.iter().map(|m| json!({
            "id": m.id,
            "role": m.role,
            "content": m.content,
            "createdAt": m.created_at,
        })).collect::<Vec<_>>(),
        "inlineSurfaces": surfaces.iter().map(|s| json!({
            "id": s.id,
            "instanceId": s.instance_id,
            "name": s.name,
            "definition": s.definition,
            "revision": s.current_revision,
        })).collect::<Vec<_>>(),
        "transactions": txns.iter().map(|t| json!({
            "id": t.id,
            "summary": t.summary,
            "status": t.status,
            "operations": t.operations,
        })).collect::<Vec<_>>(),
        "readOnly": true,
    });
    let id = format!("snap-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let payload_json = payload.to_string();
    let max = super::limits::MAX_SNAPSHOTS_PER_CONVERSATION as i64;

    // Insert first, then prune overflow in the same transaction so a failed
    // insert never drops retained snapshots.
    db.with_transaction(|conn| {
        conn.execute(
            "INSERT INTO conversation_snapshots (id, conversation_id, project_id, description, read_only_payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                conversation_id,
                project_id,
                description,
                payload_json,
                now
            ],
        )?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM conversation_snapshots WHERE conversation_id = ?1",
            [conversation_id],
            |r| r.get(0),
        )?;
        if count > max {
            let overflow = count - max;
            conn.execute(
                "DELETE FROM conversation_snapshots WHERE id IN (
                    SELECT id FROM conversation_snapshots
                    WHERE conversation_id = ?1
                    ORDER BY created_at ASC
                    LIMIT ?2
                 )",
                params![conversation_id, overflow],
            )?;
        }
        Ok(())
    })?;

    Ok(SnapshotRecord {
        id,
        conversation_id: conversation_id.into(),
        project_id: project_id.map(|s| s.to_string()),
        description: description.into(),
        payload,
        created_at: now,
    })
}

pub fn get_snapshot(db: &Database, id: &str) -> DbResult<SnapshotRecord> {
    db.conn()
        .query_row(
            "SELECT id, conversation_id, project_id, description, read_only_payload_json, created_at
             FROM conversation_snapshots WHERE id = ?1",
            [id],
            |row| {
                let payload_json: String = row.get(4)?;
                Ok(SnapshotRecord {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    project_id: row.get(2)?,
                    description: row.get(3)?,
                    payload: serde_json::from_str(&payload_json).unwrap_or(Value::Null),
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("snapshot {id}")),
            other => DbError::Sqlite(other),
        })
}

pub fn delete_snapshot(db: &mut Database, id: &str) -> DbResult<()> {
    let n = db
        .conn()
        .execute("DELETE FROM conversation_snapshots WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(DbError::NotFound(format!("snapshot {id}")));
    }
    Ok(())
}

pub fn list_branches(
    db: &Database,
    source_conversation_id: &str,
) -> DbResult<Vec<ChatBranchRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, source_conversation_id, source_message_id, new_conversation_id, branch_name, created_at
         FROM chat_branches WHERE source_conversation_id = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([source_conversation_id], |row| {
        Ok(ChatBranchRecord {
            id: row.get(0)?,
            source_conversation_id: row.get(1)?,
            source_message_id: row.get(2)?,
            new_conversation_id: row.get(3)?,
            branch_name: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::Sqlite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, Database, DEFAULT_WORKSPACE_ID};
    use tempfile::tempdir;

    #[test]
    fn snapshot_pruning_keeps_newest_within_limit() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("snap.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Snap", None).unwrap();
        let limit = super::super::limits::MAX_SNAPSHOTS_PER_CONVERSATION;

        let mut ids = Vec::new();
        for i in 0..(limit + 3) {
            let snap = create_snapshot(&mut db, &conv.id, None, &format!("s{i}")).unwrap();
            ids.push(snap.id);
        }

        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM conversation_snapshots WHERE conversation_id = ?1",
                [&conv.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count as usize, limit);

        // Oldest three must be gone; newest must remain.
        for old in &ids[..3] {
            assert!(get_snapshot(&db, old).is_err());
        }
        assert!(get_snapshot(&db, ids.last().unwrap()).is_ok());
    }

    #[test]
    fn branch_limit_rejects_unbounded_fanout() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("branch.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Root", None).unwrap();
        let msg = crate::db::insert_message(&mut db, &conv.id, "user", "hi", None).unwrap();

        for i in 0..super::super::limits::MAX_BRANCH_DEPTH {
            branch_from_message(
                &mut db,
                &conv.id,
                &msg.id,
                &format!("b{i}"),
                DEFAULT_WORKSPACE_ID,
            )
            .unwrap();
        }

        let err = branch_from_message(
            &mut db,
            &conv.id,
            &msg.id,
            "overflow",
            DEFAULT_WORKSPACE_ID,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("branch limit"),
            "expected branch limit error, got {err}"
        );
    }
}
