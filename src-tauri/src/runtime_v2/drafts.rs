//! Surface draft persistence with revision conflict detection.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceDraft {
    pub id: String,
    pub surface_id: String,
    pub component_id: String,
    pub form_id: Option<String>,
    pub window_id: String,
    pub base_revision: i64,
    pub draft: Value,
    pub persistence_policy: String,
    pub updated_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DraftConflict {
    pub stored_revision: i64,
    pub requested_revision: i64,
}

pub fn get_draft(
    db: &Database,
    surface_id: &str,
    component_id: &str,
    window_id: &str,
) -> DbResult<Option<SurfaceDraft>> {
    db.conn()
        .query_row(
            "SELECT id, surface_id, component_id, form_id, window_id, base_revision,
                    draft_json, persistence_policy, updated_at, created_at
             FROM surface_drafts
             WHERE surface_id = ?1 AND component_id = ?2 AND window_id = ?3",
            params![surface_id, component_id, window_id],
            |row| {
                let draft_json: String = row.get(6)?;
                Ok(SurfaceDraft {
                    id: row.get(0)?,
                    surface_id: row.get(1)?,
                    component_id: row.get(2)?,
                    form_id: row.get(3)?,
                    window_id: row.get(4)?,
                    base_revision: row.get(5)?,
                    draft: serde_json::from_str(&draft_json)
                        .unwrap_or(Value::Object(Default::default())),
                    persistence_policy: row.get(7)?,
                    updated_at: row.get(8)?,
                    created_at: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(DbError::Sqlite)
}

pub fn save_draft(
    db: &mut Database,
    surface_id: &str,
    component_id: &str,
    window_id: &str,
    base_revision: i64,
    draft: &Value,
    form_id: Option<&str>,
    persistence_policy: Option<&str>,
    force: bool,
) -> Result<SurfaceDraft, DraftConflict> {
    if let Some(existing) =
        get_draft(db, surface_id, component_id, window_id).map_err(|_| DraftConflict {
            stored_revision: -1,
            requested_revision: base_revision,
        })?
    {
        if !force && existing.base_revision >= base_revision {
            return Err(DraftConflict {
                stored_revision: existing.base_revision,
                requested_revision: base_revision,
            });
        }
    }
    let now = now_rfc3339();
    let id = format!("draft-{}", Uuid::new_v4());
    let draft_json = draft.to_string();
    let policy = persistence_policy.unwrap_or("session");
    db.conn()
        .execute(
            "INSERT INTO surface_drafts (
                id, surface_id, component_id, form_id, window_id, base_revision,
                draft_json, persistence_policy, updated_at, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(surface_id, component_id, window_id) DO UPDATE SET
                form_id = excluded.form_id,
                base_revision = excluded.base_revision,
                draft_json = excluded.draft_json,
                persistence_policy = excluded.persistence_policy,
                updated_at = excluded.updated_at",
            params![
                id,
                surface_id,
                component_id,
                form_id,
                window_id,
                base_revision,
                draft_json,
                policy,
                now,
                now
            ],
        )
        .map_err(|_| DraftConflict {
            stored_revision: base_revision,
            requested_revision: base_revision,
        })?;
    get_draft(db, surface_id, component_id, window_id)
        .map_err(|_| DraftConflict {
            stored_revision: base_revision,
            requested_revision: base_revision,
        })?
        .ok_or(DraftConflict {
            stored_revision: base_revision,
            requested_revision: base_revision,
        })
}

pub fn delete_draft(
    db: &mut Database,
    surface_id: &str,
    component_id: &str,
    window_id: &str,
) -> DbResult<()> {
    db.conn().execute(
        "DELETE FROM surface_drafts WHERE surface_id = ?1 AND component_id = ?2 AND window_id = ?3",
        params![surface_id, component_id, window_id],
    )?;
    Ok(())
}

/// Delete all drafts for a surface (used before hard-deleting the surface).
pub fn delete_drafts_for_surface(db: &mut Database, surface_id: &str) -> DbResult<u64> {
    let n = db.conn().execute(
        "DELETE FROM surface_drafts WHERE surface_id = ?1",
        [surface_id],
    )?;
    Ok(n as u64)
}

/// Delete drafts for every surface belonging to a conversation.
/// Call **before** deleting the conversation: surfaces SET NULL conversation_id on cascade.
pub fn delete_drafts_for_conversation(db: &mut Database, conversation_id: &str) -> DbResult<u64> {
    let n = db.conn().execute(
        "DELETE FROM surface_drafts WHERE surface_id IN (
            SELECT id FROM surfaces WHERE conversation_id = ?1
         )",
        [conversation_id],
    )?;
    Ok(n as u64)
}

trait OptionalRow<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalRow<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
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
    fn draft_conflict_on_stale_revision() {
        let mut db = test_db();
        save_draft(
            &mut db,
            "surf-1",
            "cmp-1",
            "main",
            1,
            &json!({"text": "hello"}),
            None,
            None,
            false,
        )
        .unwrap();
        let err = save_draft(
            &mut db,
            "surf-1",
            "cmp-1",
            "main",
            1,
            &json!({"text": "world"}),
            None,
            None,
            false,
        )
        .unwrap_err();
        assert_eq!(err.stored_revision, 1);
        assert_eq!(err.requested_revision, 1);
        let ok = save_draft(
            &mut db,
            "surf-1",
            "cmp-1",
            "main",
            2,
            &json!({"text": "world"}),
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(ok.base_revision, 2);
    }

    #[test]
    fn delete_draft_removes_row() {
        let mut db = test_db();
        save_draft(
            &mut db,
            "surf-1",
            "cmp-1",
            "main",
            1,
            &json!({"text": "hello"}),
            None,
            None,
            false,
        )
        .unwrap();
        delete_draft(&mut db, "surf-1", "cmp-1", "main").unwrap();
        assert!(get_draft(&db, "surf-1", "cmp-1", "main").unwrap().is_none());
    }

    #[test]
    fn delete_drafts_for_surface_and_conversation() {
        let mut db = test_db();
        let conv = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Drafts",
            None,
        )
        .unwrap();
        let surface = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "S",
            &json!({
                "id": "d",
                "name": "S",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        save_draft(
            &mut db,
            &surface.id,
            "t",
            "main",
            1,
            &json!({"v": 1}),
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(delete_drafts_for_surface(&mut db, &surface.id).unwrap(), 1);
        assert!(get_draft(&db, &surface.id, "t", "main").unwrap().is_none());

        save_draft(
            &mut db,
            &surface.id,
            "t",
            "main",
            1,
            &json!({"v": 2}),
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            delete_drafts_for_conversation(&mut db, &conv.id).unwrap(),
            1
        );
    }

    #[test]
    fn delete_conversation_cleans_surface_drafts() {
        let mut db = test_db();
        let conv =
            crate::db::create_conversation(&mut db, crate::db::DEFAULT_WORKSPACE_ID, "Gone", None)
                .unwrap();
        let surface = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "S",
            &json!({
                "id": "d",
                "name": "S",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        save_draft(
            &mut db,
            &surface.id,
            "t",
            "main",
            1,
            &json!({"v": 1}),
            None,
            None,
            false,
        )
        .unwrap();
        crate::db::delete_conversation(&mut db, &conv.id).unwrap();
        assert!(get_draft(&db, &surface.id, "t", "main").unwrap().is_none());
    }
}
