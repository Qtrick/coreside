//! Chat branching and read-only snapshots.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::surfaces::{list_inline_surfaces, SurfaceRecord};
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::{params, OptionalExtension};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceDiffSummary {
    pub surface_id: String,
    pub name: String,
    pub status: String,
    pub changed_components: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchDiffRecord {
    pub branch_id: String,
    pub source_conversation_id: String,
    pub branch_conversation_id: String,
    pub fork_message_id: Option<String>,
    pub source_message_count: usize,
    pub branch_message_count: usize,
    pub unique_source_messages: usize,
    pub unique_branch_messages: usize,
    pub surfaces: Vec<SurfaceDiffSummary>,
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
    db.conn()
        .execute_batch("SAVEPOINT branch_sp")
        .map_err(DbError::Sqlite)?;

    let res = (|| -> DbResult<(ChatBranchRecord, Vec<SurfaceRecord>)> {
        let existing = list_branches(db, source_conversation_id)?;
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
        let _source_msg = &messages[cut];
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

        let next_msg_created_at = messages.get(cut + 1).map(|m| m.created_at.clone());

        let subsequent_prev_snapshots: Vec<String> = if let Some(ref cutoff) = next_msg_created_at {
            let mut stmt = db.conn().prepare(
                "SELECT previous_snapshot_json FROM app_transactions
                 WHERE conversation_id = ?1 AND created_at >= ?2 AND status = 'applied' AND previous_snapshot_json IS NOT NULL
                 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![source_conversation_id, cutoff], |row| {
                row.get::<_, String>(0)
            })?;
            rows.filter_map(|r| r.ok()).collect()
        } else {
            Vec::new()
        };

        let mut cloned_surfaces = Vec::new();
        for surface in list_inline_surfaces(db, source_conversation_id)? {
            // Query surface_versions for the highest revision valid up to the cut message.
            // If there is a subsequent message, the cutoff is strictly before that next message.
            let historical_row: Option<(i64, String)> = if let Some(ref cutoff) = next_msg_created_at {
                db.conn().query_row(
                    "SELECT revision, definition_json FROM surface_versions
                     WHERE surface_id = ?1 AND created_at < ?2
                     ORDER BY revision DESC LIMIT 1",
                    params![surface.id, cutoff],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                ).optional()?
            } else {
                db.conn().query_row(
                    "SELECT revision, definition_json FROM surface_versions
                     WHERE surface_id = ?1
                     ORDER BY revision DESC LIMIT 1",
                    params![surface.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                ).optional()?
            };

            // If no version existed before the next message cut,
            // the surface was created after the message, so do not copy it into the branch.
            let (_rev, def_json_str) = match historical_row {
                Some(row) => row,
                None => continue,
            };

            let mut def = serde_json::from_str::<Value>(&def_json_str)
                .unwrap_or_else(|_| surface.definition.clone());
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

            // Restore historical surface state
            let mut historical_state = super::surfaces::get_surface_state(db, &surface.id)
                .unwrap_or_else(|_| json!({}));
            for prev_str in &subsequent_prev_snapshots {
                if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(prev_str) {
                    if let Some(surface_snap) = map.get(&surface.id) {
                        if let Some(st) = surface_snap.get("state") {
                            historical_state = st.clone();
                            break;
                        }
                    }
                }
            }

            let _ = super::surfaces::save_surface_state(db, &created.id, &historical_state);
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
    })();

    match res {
        Ok(val) => {
            db.conn()
                .execute_batch("RELEASE SAVEPOINT branch_sp")
                .map_err(DbError::Sqlite)?;
            Ok(val)
        }
        Err(e) => {
            let _ = db
                .conn()
                .execute_batch("ROLLBACK TO SAVEPOINT branch_sp; RELEASE SAVEPOINT branch_sp");
            Err(e)
        }
    }
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

/// List snapshots for a conversation. Payload is omitted (Null) for list size;
/// call [`get_snapshot`] when full read-only payload is needed.
pub fn list_snapshots(db: &Database, conversation_id: &str) -> DbResult<Vec<SnapshotRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, conversation_id, project_id, description, created_at
         FROM conversation_snapshots WHERE conversation_id = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([conversation_id], |row| {
        Ok(SnapshotRecord {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            project_id: row.get(2)?,
            description: row.get(3)?,
            payload: Value::Null,
            created_at: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::Sqlite)
}

pub fn get_branch(db: &Database, id: &str) -> DbResult<ChatBranchRecord> {
    db.conn()
        .query_row(
            "SELECT id, source_conversation_id, source_message_id, new_conversation_id, branch_name, created_at
             FROM chat_branches WHERE id = ?1",
            [id],
            |row| {
                Ok(ChatBranchRecord {
                    id: row.get(0)?,
                    source_conversation_id: row.get(1)?,
                    source_message_id: row.get(2)?,
                    new_conversation_id: row.get(3)?,
                    branch_name: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("branch {id}")),
            other => DbError::Sqlite(other),
        })
}

/// Calculate a non-destructive diff between a branch and its parent conversation.
pub fn diff_branch(db: &Database, branch_id: &str) -> DbResult<BranchDiffRecord> {
    let branch = get_branch(db, branch_id)?;
    let source_msgs = crate::db::get_messages(db, &branch.source_conversation_id)?;
    let branch_msgs = crate::db::get_messages(db, &branch.new_conversation_id)?;

    let fork_idx = branch
        .source_message_id
        .as_deref()
        .and_then(|mid| source_msgs.iter().position(|m| m.id == mid));

    let shared_count = fork_idx.map(|idx| idx + 1).unwrap_or(0);
    let unique_source = source_msgs.len().saturating_sub(shared_count);
    let unique_branch = branch_msgs.len().saturating_sub(shared_count);

    let source_surfs = list_inline_surfaces(db, &branch.source_conversation_id)?;
    let branch_surfs = list_inline_surfaces(db, &branch.new_conversation_id)?;

    let mut surfaces_diff = Vec::new();

    for b_surf in &branch_surfs {
        let matching_source = source_surfs.iter().find(|s| {
            s.name == b_surf.name
                || b_surf
                    .definition
                    .get("_branchedFrom")
                    .and_then(|v| v.as_str())
                    == Some(&s.instance_id)
        });

        if let Some(s_surf) = matching_source {
            let changed_comps = diff_surface_components(&s_surf.definition, &b_surf.definition);
            let mut clean_s = s_surf.definition.clone();
            let mut clean_b = b_surf.definition.clone();
            if let Some(obj) = clean_s.as_object_mut() {
                obj.remove("_branchedFrom");
            }
            if let Some(obj) = clean_b.as_object_mut() {
                obj.remove("_branchedFrom");
            }
            let status = if changed_comps.is_empty() && clean_s == clean_b {
                "identical".to_string()
            } else {
                "modified".to_string()
            };
            surfaces_diff.push(SurfaceDiffSummary {
                surface_id: b_surf.id.clone(),
                name: b_surf.name.clone(),
                status,
                changed_components: changed_comps,
            });
        } else {
            surfaces_diff.push(SurfaceDiffSummary {
                surface_id: b_surf.id.clone(),
                name: b_surf.name.clone(),
                status: "added".to_string(),
                changed_components: vec![],
            });
        }
    }

    for s_surf in &source_surfs {
        let matching_branch = branch_surfs.iter().any(|b| {
            b.name == s_surf.name
                || b.definition.get("_branchedFrom").and_then(|v| v.as_str())
                    == Some(&s_surf.instance_id)
        });
        if !matching_branch {
            surfaces_diff.push(SurfaceDiffSummary {
                surface_id: s_surf.id.clone(),
                name: s_surf.name.clone(),
                status: "removed".to_string(),
                changed_components: vec![],
            });
        }
    }

    Ok(BranchDiffRecord {
        branch_id: branch.id,
        source_conversation_id: branch.source_conversation_id,
        branch_conversation_id: branch.new_conversation_id,
        fork_message_id: branch.source_message_id,
        source_message_count: source_msgs.len(),
        branch_message_count: branch_msgs.len(),
        unique_source_messages: unique_source,
        unique_branch_messages: unique_branch,
        surfaces: surfaces_diff,
    })
}

fn diff_surface_components(def_a: &Value, def_b: &Value) -> Vec<String> {
    let empty = vec![];
    let comps_a = def_a
        .get("components")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    let comps_b = def_b
        .get("components")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let mut changed = Vec::new();
    let mut map_a = std::collections::HashMap::new();
    for c in comps_a {
        if let Some(id) = c.get("id").and_then(|v| v.as_str()) {
            map_a.insert(id, c);
        }
    }

    for c in comps_b {
        if let Some(id) = c.get("id").and_then(|v| v.as_str()) {
            if let Some(a_comp) = map_a.remove(id) {
                if a_comp != c {
                    changed.push(id.to_string());
                }
            } else {
                changed.push(id.to_string());
            }
        }
    }

    for id in map_a.keys() {
        changed.push(id.to_string());
    }

    changed
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
    fn list_snapshots_returns_metadata_without_payload() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("list-snap.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "List", None).unwrap();
        let created = create_snapshot(&mut db, &conv.id, None, "checkpoint").unwrap();
        assert!(!created.payload.is_null());

        let listed = list_snapshots(&db, &conv.id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].description, "checkpoint");
        assert!(listed[0].payload.is_null());
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

        let err = branch_from_message(&mut db, &conv.id, &msg.id, "overflow", DEFAULT_WORKSPACE_ID)
            .unwrap_err();
        assert!(
            err.to_string().contains("branch limit"),
            "expected branch limit error, got {err}"
        );
    }

    #[test]
    fn diff_branch_detects_message_and_surface_changes() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("branch_diff.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Root Chat", None).unwrap();
        let m1 = crate::db::insert_message(&mut db, &conv.id, "user", "Message 1", None).unwrap();
        let def = json!({
            "id": "tool-1",
            "name": "Planner",
            "layout": "stack",
            "components": [
                {"id": "comp-header", "type": "heading", "props": {"text": "Original Header"}},
                {"id": "comp-content", "type": "text", "props": {"text": "Original Body"}}
            ]
        });
        let _s = super::super::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            Some(&m1.id),
            None,
            "Planner",
            &def,
            &[],
        )
        .unwrap();

        let _m2 =
            crate::db::insert_message(&mut db, &conv.id, "assistant", "Message 2", None).unwrap();

        let (branch, branch_surfs) = branch_from_message(
            &mut db,
            &conv.id,
            &m1.id,
            "Alternative Flow",
            DEFAULT_WORKSPACE_ID,
        )
        .unwrap();

        // 1. Initially right after branch: source has 2 messages, branch has 1 message, surface identical
        let diff_initial = diff_branch(&db, &branch.id).unwrap();
        assert_eq!(diff_initial.source_message_count, 2);
        assert_eq!(diff_initial.branch_message_count, 1);
        assert_eq!(diff_initial.unique_source_messages, 1);
        assert_eq!(diff_initial.unique_branch_messages, 0);
        assert_eq!(diff_initial.surfaces.len(), 1);
        assert_eq!(diff_initial.surfaces[0].status, "identical");

        // 2. Add message to branch and modify branch surface component
        let _m_branch = crate::db::insert_message(
            &mut db,
            &branch.new_conversation_id,
            "user",
            "Branch specific message",
            None,
        )
        .unwrap();

        let branch_surf_id = &branch_surfs[0].id;
        let mut modified_def = def.clone();
        modified_def["components"][0]["props"]["text"] = json!("Branch Changed Header");
        super::super::surfaces::update_surface_definition(
            &mut db,
            branch_surf_id,
            &modified_def,
            "Updated header in branch",
            None,
        )
        .unwrap();

        let diff_after = diff_branch(&db, &branch.id).unwrap();
        assert_eq!(diff_after.source_message_count, 2);
        assert_eq!(diff_after.branch_message_count, 2);
        assert_eq!(diff_after.unique_source_messages, 1);
        assert_eq!(diff_after.unique_branch_messages, 1);
        assert_eq!(diff_after.surfaces.len(), 1);
        assert_eq!(diff_after.surfaces[0].status, "modified");
        assert_eq!(
            diff_after.surfaces[0].changed_components,
            vec!["comp-header"]
        );
    }

    #[test]
    fn test_branch_historical_checkpoint_reconstruction() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("branch_hist.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Hist Chat", None).unwrap();

        let def_a1 = json!({
            "id": "tool-a",
            "name": "Surface A",
            "layout": "stack",
            "components": [{"id": "comp-1", "type": "text", "props": {"text": "v1"}}]
        });
        let m1 = crate::db::insert_message(&mut db, &conv.id, "user", "Message 1", None).unwrap();
        let surf_a = super::super::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            Some(&m1.id),
            None,
            "Surface A",
            &def_a1,
            &[],
        )
        .unwrap();

        // Later turn: Message 2, update Surface A to revision 2, and create Surface B
        let m2 = crate::db::insert_message(&mut db, &conv.id, "user", "Message 2", None).unwrap();
        let mut def_a2 = def_a1.clone();
        def_a2["components"][0]["props"]["text"] = json!("v2");
        super::super::surfaces::update_surface_definition(
            &mut db,
            &surf_a.id,
            &def_a2,
            "Rev 2",
            None,
        )
        .unwrap();

        let def_b = json!({
            "id": "tool-b",
            "name": "Surface B",
            "layout": "stack",
            "components": [{"id": "comp-b", "type": "text", "props": {"text": "b"}}]
        });
        let _surf_b = super::super::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            Some(&m2.id),
            None,
            "Surface B",
            &def_b,
            &[],
        )
        .unwrap();

        // Branch from Message 1:
        // 1. Must include Surface A, but reconstructed at revision 1 ("v1")!
        // 2. Must NOT include Surface B (created in message 2)!
        let (_branch1, branch1_surfs) = branch_from_message(
            &mut db,
            &conv.id,
            &m1.id,
            "Branch At M1",
            DEFAULT_WORKSPACE_ID,
        )
        .unwrap();

        assert_eq!(branch1_surfs.len(), 1);
        assert_eq!(branch1_surfs[0].name, "Surface A");
        let text_in_branch = branch1_surfs[0]
            .definition
            .pointer("/components/0/props/text")
            .and_then(|v| v.as_str());
        assert_eq!(text_in_branch, Some("v1"));

        // Branch from Message 2:
        // Must include both Surface A (at revision 2) and Surface B!
        let (_branch2, branch2_surfs) = branch_from_message(
            &mut db,
            &conv.id,
            &m2.id,
            "Branch At M2",
            DEFAULT_WORKSPACE_ID,
        )
        .unwrap();

        assert_eq!(branch2_surfs.len(), 2);
        let surf_a_in_b2 = branch2_surfs.iter().find(|s| s.name == "Surface A").unwrap();
        let text_in_b2 = surf_a_in_b2
            .definition
            .pointer("/components/0/props/text")
            .and_then(|v| v.as_str());
        assert_eq!(text_in_b2, Some("v2"));
        assert!(branch2_surfs.iter().any(|s| s.name == "Surface B"));
    }
}
