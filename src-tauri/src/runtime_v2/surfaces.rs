//! Surface persistence and lifecycle.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::limits::{
    MAX_DEFINITION_JSON_BYTES, MAX_STATE_JSON_BYTES, MAX_SURFACES_PER_CONVERSATION,
};
use super::packs::{
    normalize_capability_packs, required_packs_for_definition,
    validate_definition_components_for_packs,
};
use crate::ai::{layout_type_string, ToolDefinition};
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

/// Options for [`delete_surface`]. Linked tools are kept unless explicitly requested.
#[derive(Debug, Clone, Default)]
pub struct DeleteSurfaceOptions {
    pub delete_linked_tool: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceRecord {
    pub id: String,
    pub instance_id: String,
    pub surface_type: String,
    pub placement: String,
    pub owner_type: String,
    pub owner_id: Option<String>,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub tool_id: Option<String>,
    pub message_id: Option<String>,
    pub name: String,
    pub definition: Value,
    pub current_revision: i64,
    pub lifecycle_state: String,
    pub archived: bool,
    pub capability_packs: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn new_instance_id() -> String {
    format!("inst-{}", Uuid::new_v4())
}

pub fn surface_id_for_tool(tool_id: &str) -> String {
    format!("surf-{tool_id}")
}

pub fn upsert_surface_from_tool(
    db: &mut Database,
    tool: &ToolDefinition,
    workspace_id: &str,
    revision: i64,
) -> DbResult<SurfaceRecord> {
    crate::security::assert_not_protected(&tool.id).map_err(DbError::Invalid)?;
    let id = surface_id_for_tool(&tool.id);
    let now = now_rfc3339();
    let mut def = tool.clone();
    def.normalize_for_frontend();
    let def_json = serde_json::to_string(&def)?;
    let existing: Option<(String, String, String, String)> = db
        .conn()
        .query_row(
            "SELECT instance_id, id, capability_packs_json, definition_json FROM surfaces WHERE tool_id = ?1 OR id = ?2",
            params![tool.id, id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional_compat()?;

    let instance_id = existing
        .as_ref()
        .map(|(i, _, _, _)| i.clone())
        .unwrap_or_else(new_instance_id);
    let packs = match existing.as_ref() {
        Some((_, _, packs_json, existing_definition_json)) => {
            let persisted: Vec<String> = serde_json::from_str(packs_json)?;
            if persisted.is_empty() {
                // Legacy rows did not persist an assignment. Backfill from the
                // already-stored definition, never the incoming replacement.
                let existing_definition: Value = serde_json::from_str(existing_definition_json)?;
                required_packs_for_definition(&existing_definition).map_err(DbError::Invalid)?
            } else {
                normalize_capability_packs(&persisted).map_err(DbError::Invalid)?
            }
        }
        None => {
            required_packs_for_definition(&serde_json::to_value(&def)?).map_err(DbError::Invalid)?
        }
    };
    validate_definition_components_for_packs(&serde_json::to_value(&def)?, &packs)
        .map_err(DbError::Invalid)?;
    let packs_json = serde_json::to_string(&packs)?;

    if existing.is_some() {
        db.conn().execute(
            "UPDATE surfaces SET name = ?1, definition_json = ?2, current_revision = ?3,
             capability_packs_json = ?4, updated_at = ?5, lifecycle_state = 'active', archived = 0 WHERE id = ?6",
            params![def.name, def_json, revision, packs_json, now, id],
        )?;
    } else {
        db.conn().execute(
            "INSERT INTO surfaces (
                id, instance_id, surface_type, placement, owner_type, owner_id,
                tool_id, name, definition_json, current_revision, lifecycle_state, archived, capability_packs_json,
                created_at, updated_at
             ) VALUES (?1, ?2, 'tool', 'tool_canvas', 'workspace', ?3, ?4, ?5, ?6, ?7, 'active', 0, ?8, ?9, ?10)",
            params![
                id,
                instance_id,
                workspace_id,
                tool.id,
                def.name,
                def_json,
                revision,
                packs_json,
                now,
                now
            ],
        )?;
    }

    let version_id = format!("sv-{}", Uuid::new_v4());
    db.conn().execute(
        "INSERT OR IGNORE INTO surface_versions (id, surface_id, revision, definition_json, change_summary, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![version_id, id, revision, def_json, "tool sync", now],
    )?;

    get_surface(db, &id)
}

pub fn create_inline_surface(
    db: &mut Database,
    conversation_id: &str,
    message_id: Option<&str>,
    project_id: Option<&str>,
    name: &str,
    definition: &Value,
    packs: &[String],
) -> DbResult<SurfaceRecord> {
    let supplied_packs = normalize_capability_packs(packs).map_err(DbError::Invalid)?;
    let effective_packs = if packs.is_empty() {
        required_packs_for_definition(definition).map_err(DbError::Invalid)?
    } else {
        supplied_packs
    };
    validate_definition_components_for_packs(definition, &effective_packs)
        .map_err(DbError::Invalid)?;
    let def_json = serde_json::to_string(definition)?;
    if def_json.len() > MAX_DEFINITION_JSON_BYTES {
        return Err(DbError::Invalid("surface definition too large".into()));
    }
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM surfaces WHERE conversation_id = ?1 AND archived = 0",
        [conversation_id],
        |r| r.get(0),
    )?;
    if count as usize >= MAX_SURFACES_PER_CONVERSATION {
        return Err(DbError::Invalid(format!(
            "surface limit reached: max {MAX_SURFACES_PER_CONVERSATION} per conversation"
        )));
    }
    let id = format!("surf-{}", Uuid::new_v4());
    let instance_id = new_instance_id();
    let now = now_rfc3339();
    let packs_json = serde_json::to_string(&effective_packs)?;
    db.conn().execute(
        "INSERT INTO surfaces (
            id, instance_id, surface_type, placement, owner_type, owner_id,
            conversation_id, project_id, message_id, name, definition_json,
            current_revision, lifecycle_state, archived, capability_packs_json, created_at, updated_at
         ) VALUES (?1, ?2, 'inline', 'chat_inline', 'conversation', ?3, ?3, ?4, ?5, ?6, ?7, 1, 'active', 0, ?8, ?9, ?10)",
        params![
            id,
            instance_id,
            conversation_id,
            project_id,
            message_id,
            name,
            def_json,
            packs_json,
            now,
            now
        ],
    )?;
    db.conn().execute(
        "INSERT INTO surface_state (surface_id, state_json, updated_at) VALUES (?1, '{}', ?2)",
        params![id, now],
    )?;
    let version_id = format!("sv-{}", Uuid::new_v4());
    db.conn().execute(
        "INSERT INTO surface_versions (id, surface_id, revision, definition_json, change_summary, created_at)
         VALUES (?1, ?2, 1, ?3, 'inline create', ?4)",
        params![version_id, id, def_json, now],
    )?;
    // ponytail: dependency edges on create are optional; delete_surface cleans both
    // directions. Wire add_dependency(conversation→surface) when impact UI needs it.
    get_surface(db, &id)
}

pub fn get_surface(db: &Database, id: &str) -> DbResult<SurfaceRecord> {
    db.conn()
        .query_row(
            "SELECT id, instance_id, surface_type, placement, owner_type, owner_id,
                    conversation_id, project_id, tool_id, message_id, name, definition_json,
                    current_revision, lifecycle_state, archived, capability_packs_json,
                    created_at, updated_at
             FROM surfaces WHERE id = ?1",
            [id],
            |row| {
                let def_json: String = row.get(11)?;
                let packs_json: String = row.get(15)?;
                let definition: Value = serde_json::from_str(&def_json).unwrap_or(Value::Null);
                let capability_packs: Vec<String> =
                    serde_json::from_str(&packs_json).unwrap_or_default();
                Ok(SurfaceRecord {
                    id: row.get(0)?,
                    instance_id: row.get(1)?,
                    surface_type: row.get(2)?,
                    placement: row.get(3)?,
                    owner_type: row.get(4)?,
                    owner_id: row.get(5)?,
                    conversation_id: row.get(6)?,
                    project_id: row.get(7)?,
                    tool_id: row.get(8)?,
                    message_id: row.get(9)?,
                    name: row.get(10)?,
                    definition,
                    current_revision: row.get(12)?,
                    lifecycle_state: row.get(13)?,
                    archived: row.get::<_, i64>(14)? != 0,
                    capability_packs,
                    created_at: row.get(16)?,
                    updated_at: row.get(17)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("surface {id}")),
            other => DbError::Sqlite(other),
        })
}

/// List all active inline surfaces for a conversation.
///
/// Do **not** apply `MAX_INLINE_SURFACES_VISIBLE` here — that ceiling is UI-only.
/// Truncating at the DB layer would hide surfaces on older messages and silently
/// drop them from branch clones (callers include `list_conversation_surfaces` and
/// `branch_chat`). Creation remains capped by `MAX_SURFACES_PER_CONVERSATION`.
pub fn list_inline_surfaces(db: &Database, conversation_id: &str) -> DbResult<Vec<SurfaceRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id FROM surfaces WHERE conversation_id = ?1 AND placement = 'chat_inline'
         AND archived = 0 ORDER BY updated_at DESC",
    )?;
    let ids: Vec<String> = stmt
        .query_map([conversation_id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter().map(|id| get_surface(db, &id)).collect()
}

/// Soft-archive a surface. Does not delete versions, state, or linked tools.
pub fn archive_surface(db: &mut Database, surface_id: &str) -> DbResult<SurfaceRecord> {
    crate::security::assert_not_protected(surface_id).map_err(DbError::Invalid)?;
    let current = get_surface(db, surface_id)?;
    if let Some(tool_id) = current.tool_id.as_ref() {
        crate::security::assert_not_protected(tool_id).map_err(DbError::Invalid)?;
    }
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE surfaces SET archived = 1, lifecycle_state = 'archived', updated_at = ?1 WHERE id = ?2",
        params![now, surface_id],
    )?;
    get_surface(db, surface_id)
}

/// Clear archived flag and return the surface to active lifecycle.
pub fn restore_surface(db: &mut Database, surface_id: &str) -> DbResult<SurfaceRecord> {
    crate::security::assert_not_protected(surface_id).map_err(DbError::Invalid)?;
    let current = get_surface(db, surface_id)?;
    if !current.archived {
        return Ok(current);
    }
    // Restoring counts toward the per-conversation active-surface cap.
    if let Some(conversation_id) = current.conversation_id.as_deref() {
        let count: i64 = db.conn().query_row(
            "SELECT COUNT(*) FROM surfaces WHERE conversation_id = ?1 AND archived = 0",
            [conversation_id],
            |r| r.get(0),
        )?;
        if count as usize >= MAX_SURFACES_PER_CONVERSATION {
            return Err(DbError::Invalid(format!(
                "surface limit reached: max {MAX_SURFACES_PER_CONVERSATION} per conversation"
            )));
        }
    }
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE surfaces SET archived = 0, lifecycle_state = 'active', updated_at = ?1 WHERE id = ?2",
        params![now, surface_id],
    )?;
    get_surface(db, surface_id)
}

/// Hard-delete a surface and non-cascading related rows.
///
/// Does **not** delete application manifests or shared application data.
/// Linked tools are kept unless [`DeleteSurfaceOptions::delete_linked_tool`] is set.
pub fn delete_surface(
    db: &mut Database,
    surface_id: &str,
    options: DeleteSurfaceOptions,
) -> DbResult<SurfaceRecord> {
    crate::security::assert_not_protected(surface_id).map_err(DbError::Invalid)?;
    let snapshot = get_surface(db, surface_id)?;
    if let Some(tool_id) = snapshot.tool_id.as_ref() {
        crate::security::assert_not_protected(tool_id).map_err(DbError::Invalid)?;
    }

    // Non-cascading cleanup (014 continuity / drafts / preservation).
    let _ = super::drafts::delete_drafts_for_surface(db, surface_id)?;
    db.conn().execute(
        "DELETE FROM component_preservation WHERE surface_id = ?1",
        [surface_id],
    )?;
    db.conn().execute(
        "DELETE FROM surface_continuity WHERE surface_id = ?1",
        [surface_id],
    )?;
    db.conn().execute(
        "DELETE FROM manual_edit_provenance WHERE surface_id = ?1",
        [surface_id],
    )?;
    db.conn().execute(
        "DELETE FROM patch_scheduler_items WHERE surface_id = ?1",
        [surface_id],
    )?;
    // Dependency edges referencing this surface (no FK cascade).
    db.conn().execute(
        "DELETE FROM application_dependencies
         WHERE (source_type = 'surface' AND source_id = ?1)
            OR (target_type = 'surface' AND target_id = ?1)",
        [surface_id],
    )?;

    db.conn()
        .execute("DELETE FROM surfaces WHERE id = ?1", [surface_id])?;

    if options.delete_linked_tool {
        if let Some(tool_id) = snapshot.tool_id.as_ref() {
            crate::security::assert_not_protected(tool_id).map_err(DbError::Invalid)?;
            db.conn()
                .execute("DELETE FROM tools WHERE id = ?1", [tool_id])?;
        }
    }

    Ok(snapshot)
}

pub fn update_surface_definition(
    db: &mut Database,
    surface_id: &str,
    definition: &Value,
    change_summary: &str,
    expected_revision: Option<i64>,
) -> DbResult<SurfaceRecord> {
    let current = get_surface(db, surface_id)?;
    let effective_packs = if current.capability_packs.is_empty() {
        required_packs_for_definition(&current.definition).map_err(DbError::Invalid)?
    } else {
        normalize_capability_packs(&current.capability_packs).map_err(DbError::Invalid)?
    };
    validate_definition_components_for_packs(definition, &effective_packs)
        .map_err(DbError::Invalid)?;
    if let Some(tool_id) = current.tool_id.as_ref() {
        crate::security::assert_not_protected(tool_id).map_err(DbError::Invalid)?;
    }
    crate::security::assert_not_protected(surface_id).map_err(DbError::Invalid)?;
    if let Some(base) = expected_revision {
        if base != current.current_revision {
            return Err(DbError::Invalid(format!(
                "stale revision: base {base}, current {}",
                current.current_revision
            )));
        }
    }
    let next = current.current_revision + 1;
    let now = now_rfc3339();
    let def_json = serde_json::to_string(definition)?;
    if def_json.len() > MAX_DEFINITION_JSON_BYTES {
        return Err(DbError::Invalid("surface definition too large".into()));
    }
    db.conn().execute(
        "UPDATE surfaces SET definition_json = ?1, current_revision = ?2, capability_packs_json = ?3, updated_at = ?4, name = COALESCE(json_extract(?1, '$.name'), name) WHERE id = ?5",
        params![def_json, next, serde_json::to_string(&effective_packs)?, now, surface_id],
    )?;
    let version_id = format!("sv-{}", Uuid::new_v4());
    db.conn().execute(
        "INSERT INTO surface_versions (id, surface_id, revision, definition_json, change_summary, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![version_id, surface_id, next, def_json, change_summary, now],
    )?;

    // Keep linked tool in sync when this is a tool surface
    if let Some(tool_id) = current.tool_id.as_ref() {
        if let Ok(tool) = serde_json::from_value::<ToolDefinition>(definition.clone()) {
            let layout = layout_type_string(&tool.layout);
            db.conn().execute(
                "UPDATE tools SET name = ?1, description = ?2, layout = ?3, definition_json = ?4,
                 current_version = ?5, updated_at = ?6 WHERE id = ?7",
                params![
                    tool.name,
                    tool.description,
                    layout,
                    def_json,
                    next,
                    now,
                    tool_id
                ],
            )?;
        }
    }
    get_surface(db, surface_id)
}

pub fn promote_inline_to_tool(
    db: &mut Database,
    surface_id: &str,
    workspace_id: &str,
) -> DbResult<SurfaceRecord> {
    let surface = get_surface(db, surface_id)?;
    if surface.placement != "chat_inline" {
        return Err(DbError::Invalid(
            "only inline surfaces can be promoted".into(),
        ));
    }
    let mut tool: ToolDefinition = serde_json::from_value(surface.definition.clone())
        .map_err(|e| DbError::Invalid(e.to_string()))?;
    if tool.id.trim().is_empty() {
        tool.id = format!("tool-{}", Uuid::new_v4());
    }
    crate::security::assert_not_protected(&tool.id).map_err(DbError::Invalid)?;
    tool.normalize_for_frontend();
    let applied = crate::db::apply_tool_change(
        db,
        workspace_id,
        &tool,
        "create",
        None,
        "Promoted from inline surface",
    )?;
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE surfaces SET placement = 'tool_canvas', surface_type = 'tool', tool_id = ?1,
         lifecycle_state = 'promoted', updated_at = ?2 WHERE id = ?3",
        params![applied.id, now, surface_id],
    )?;
    get_surface(db, surface_id)
}

pub fn get_surface_state(db: &Database, surface_id: &str) -> DbResult<Value> {
    let _: String = db
        .conn()
        .query_row("SELECT id FROM surfaces WHERE id = ?1", [surface_id], |r| {
            r.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("surface {surface_id}"))
            }
            other => DbError::Sqlite(other),
        })?;
    match db.conn().query_row(
        "SELECT state_json FROM surface_state WHERE surface_id = ?1",
        [surface_id],
        |row| {
            let s: String = row.get(0)?;
            Ok(serde_json::from_str(&s).unwrap_or(Value::Object(Default::default())))
        },
    ) {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Value::Object(Default::default())),
        Err(other) => Err(DbError::Sqlite(other)),
    }
}

pub fn save_surface_state(db: &mut Database, surface_id: &str, state: &Value) -> DbResult<()> {
    let _: String = db
        .conn()
        .query_row("SELECT id FROM surfaces WHERE id = ?1", [surface_id], |r| {
            r.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("surface {surface_id}"))
            }
            other => DbError::Sqlite(other),
        })?;
    let state_json = state.to_string();
    if state_json.len() > MAX_STATE_JSON_BYTES {
        return Err(DbError::Invalid("surface state too large".into()));
    }
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO surface_state (surface_id, state_json, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(surface_id) DO UPDATE SET state_json = excluded.state_json, updated_at = excluded.updated_at",
        params![surface_id, state_json, now],
    )?;
    Ok(())
}

/// Deep-merge object patches for `state.patch` (preview + durable apply must match).
pub(crate) fn merge_json_objects(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(dst), Value::Object(src)) => {
            for (k, v) in src {
                match dst.get_mut(k) {
                    Some(existing) if existing.is_object() && v.is_object() => {
                        merge_json_objects(existing, v);
                    }
                    _ => {
                        dst.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (target, patch) => {
            *target = patch.clone();
        }
    }
}

trait OptionalCompat<T> {
    fn optional_compat(self) -> DbResult<Option<T>>;
}

impl<T> OptionalCompat<T> for Result<T, rusqlite::Error> {
    fn optional_compat(self) -> DbResult<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ToolDefinition;
    use crate::db::{apply_tool_change, create_conversation, Database, DEFAULT_WORKSPACE_ID};
    use serde_json::json;
    use tempfile::tempdir;

    fn test_db() -> Database {
        let dir = tempdir().unwrap();
        Database::open_path(&dir.path().join("t.db")).unwrap()
    }

    fn minimal_def(name: &str) -> Value {
        json!({
            "id": "inline-def",
            "name": name,
            "description": "",
            "layout": "stack",
            "components": [{
                "id": "title",
                "type": "heading",
                "props": { "text": name }
            }]
        })
    }

    #[test]
    fn delete_inline_surface_removes_row_and_drafts() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat", None).unwrap();
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Inline",
            &minimal_def("Inline"),
            &[],
        )
        .unwrap();
        super::super::drafts::save_draft(
            &mut db,
            &surface.id,
            "title",
            "main",
            1,
            &json!({"text": "draft"}),
            None,
            None,
            false,
        )
        .unwrap();

        let deleted =
            delete_surface(&mut db, &surface.id, DeleteSurfaceOptions::default()).unwrap();
        assert_eq!(deleted.id, surface.id);
        assert!(get_surface(&db, &surface.id).is_err());
        assert!(
            super::super::drafts::get_draft(&db, &surface.id, "title", "main")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn delete_tool_surface_keeps_tool_by_default() {
        let mut db = test_db();
        let tool = ToolDefinition {
            id: "tool-keep-me".into(),
            name: "Keep Me".into(),
            description: "d".into(),
            layout: json!("stack"),
            components: vec![crate::ai::ToolComponent {
                id: "h".into(),
                component_type: "heading".into(),
                value_key: None,
                props: Some(json!({"text": "Hi"})),
                children: None,
            }],
        };
        let applied =
            apply_tool_change(&mut db, DEFAULT_WORKSPACE_ID, &tool, "create", None, "test")
                .unwrap();
        let surface =
            upsert_surface_from_tool(&mut db, &applied.definition, DEFAULT_WORKSPACE_ID, 1)
                .unwrap();
        assert!(surface.tool_id.is_some());

        delete_surface(&mut db, &surface.id, DeleteSurfaceOptions::default()).unwrap();
        assert!(get_surface(&db, &surface.id).is_err());
        assert!(crate::db::get_tool(&db, "tool-keep-me").is_ok());
    }

    #[test]
    fn archive_and_restore_surface() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat", None).unwrap();
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Archivable",
            &minimal_def("Archivable"),
            &[],
        )
        .unwrap();

        let archived = archive_surface(&mut db, &surface.id).unwrap();
        assert!(archived.archived);
        assert_eq!(archived.lifecycle_state, "archived");
        assert!(list_inline_surfaces(&db, &conv.id).unwrap().is_empty());

        let restored = restore_surface(&mut db, &surface.id).unwrap();
        assert!(!restored.archived);
        assert_eq!(restored.lifecycle_state, "active");
        assert_eq!(list_inline_surfaces(&db, &conv.id).unwrap().len(), 1);
    }

    #[test]
    fn restore_surface_respects_conversation_limit() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat", None).unwrap();
        let archived = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Archived",
            &minimal_def("Archived"),
            &[],
        )
        .unwrap();
        archive_surface(&mut db, &archived.id).unwrap();

        for i in 0..MAX_SURFACES_PER_CONVERSATION {
            create_inline_surface(
                &mut db,
                &conv.id,
                None,
                None,
                &format!("Fill{i}"),
                &minimal_def(&format!("Fill{i}")),
                &[],
            )
            .unwrap();
        }

        let err = restore_surface(&mut db, &archived.id).unwrap_err();
        assert!(
            err.to_string().contains("surface limit"),
            "expected surface limit error, got {err}"
        );
    }

    #[test]
    fn list_inline_surfaces_returns_all_active_not_ui_visible_cap() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat", None).unwrap();
        // UI visible cap is 12; listing must still return everything for branch/UI by message.
        let count = super::super::limits::MAX_INLINE_SURFACES_VISIBLE + 3;
        for i in 0..count {
            create_inline_surface(
                &mut db,
                &conv.id,
                None,
                None,
                &format!("S{i}"),
                &minimal_def(&format!("S{i}")),
                &[],
            )
            .unwrap();
        }
        let listed = list_inline_surfaces(&db, &conv.id).unwrap();
        assert_eq!(listed.len(), count);
    }

    #[test]
    fn surface_packs_reject_ungranted_components_and_backfill_legacy_assignments() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat", None).unwrap();
        let core_only = vec!["coreside.core".to_string()];
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Core only",
            &minimal_def("Core only"),
            &core_only,
        )
        .unwrap();
        assert_eq!(surface.capability_packs, core_only);

        let mut forbidden = minimal_def("Core only");
        forbidden["components"].as_array_mut().unwrap().push(json!({
            "id": "scene",
            "type": "svgScene",
            "props": {}
        }));
        let err = update_surface_definition(&mut db, &surface.id, &forbidden, "forbidden", None)
            .unwrap_err();
        assert!(err.to_string().contains("has not been granted"));
        assert_eq!(
            get_surface(&db, &surface.id).unwrap().definition,
            minimal_def("Core only")
        );

        // Simulate an older row which predates persisted pack identity. The next
        // safe update derives only the packs its already-trusted definition needs.
        db.conn()
            .execute(
                "UPDATE surfaces SET capability_packs_json = '[]' WHERE id = ?1",
                [&surface.id],
            )
            .unwrap();
        let backfilled = update_surface_definition(
            &mut db,
            &surface.id,
            &minimal_def("Core only"),
            "legacy backfill",
            None,
        )
        .unwrap();
        assert_eq!(backfilled.capability_packs, vec!["coreside.core"]);
    }

    #[test]
    fn legacy_tool_surface_backfill_cannot_expand_on_upsert() {
        let mut db = test_db();
        let core_tool = ToolDefinition {
            id: "legacy-tool".into(),
            name: "Legacy".into(),
            description: String::new(),
            layout: json!({"type": "single-column"}),
            components: vec![crate::ai::ToolComponent {
                id: "title".into(),
                component_type: "heading".into(),
                value_key: None,
                props: Some(json!({"text": "Legacy"})),
                children: None,
            }],
        };
        let applied = apply_tool_change(
            &mut db,
            DEFAULT_WORKSPACE_ID,
            &core_tool,
            "create",
            None,
            "test",
        )
        .unwrap();
        let surface =
            upsert_surface_from_tool(&mut db, &applied.definition, DEFAULT_WORKSPACE_ID, 1)
                .unwrap();
        db.conn()
            .execute(
                "UPDATE surfaces SET capability_packs_json = '[]' WHERE id = ?1",
                [&surface.id],
            )
            .unwrap();

        let replacement = ToolDefinition {
            components: vec![crate::ai::ToolComponent {
                id: "scene".into(),
                component_type: "svgScene".into(),
                value_key: None,
                props: Some(json!({})),
                children: None,
            }],
            ..core_tool
        };
        let err =
            upsert_surface_from_tool(&mut db, &replacement, DEFAULT_WORKSPACE_ID, 2).unwrap_err();
        assert!(err.to_string().contains("has not been granted"));
        assert!(get_surface(&db, &surface.id)
            .unwrap()
            .capability_packs
            .is_empty());
    }
}
