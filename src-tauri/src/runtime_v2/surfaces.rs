//! Surface persistence and lifecycle.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::limits::{
    MAX_DEFINITION_JSON_BYTES, MAX_STATE_JSON_BYTES, MAX_SURFACES_PER_CONVERSATION,
};
use super::packs::validate_definition_components;
use crate::ai::{layout_type_string, ToolDefinition};
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

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
    let existing: Option<(String, String)> = db
        .conn()
        .query_row(
            "SELECT instance_id, id FROM surfaces WHERE tool_id = ?1 OR id = ?2",
            params![tool.id, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional_compat()?;

    let instance_id = existing
        .as_ref()
        .map(|(i, _)| i.clone())
        .unwrap_or_else(new_instance_id);

    if existing.is_some() {
        db.conn().execute(
            "UPDATE surfaces SET name = ?1, definition_json = ?2, current_revision = ?3,
             updated_at = ?4, lifecycle_state = 'active', archived = 0 WHERE id = ?5",
            params![def.name, def_json, revision, now, id],
        )?;
    } else {
        db.conn().execute(
            "INSERT INTO surfaces (
                id, instance_id, surface_type, placement, owner_type, owner_id,
                tool_id, name, definition_json, current_revision, lifecycle_state, archived,
                created_at, updated_at
             ) VALUES (?1, ?2, 'tool', 'tool_canvas', 'workspace', ?3, ?4, ?5, ?6, ?7, 'active', 0, ?8, ?9)",
            params![
                id,
                instance_id,
                workspace_id,
                tool.id,
                def.name,
                def_json,
                revision,
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
    validate_definition_components(definition).map_err(DbError::Invalid)?;
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
    let packs_json = serde_json::to_string(packs)?;
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

pub fn update_surface_definition(
    db: &mut Database,
    surface_id: &str,
    definition: &Value,
    change_summary: &str,
    expected_revision: Option<i64>,
) -> DbResult<SurfaceRecord> {
    validate_definition_components(definition).map_err(DbError::Invalid)?;
    let current = get_surface(db, surface_id)?;
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
        "UPDATE surfaces SET definition_json = ?1, current_revision = ?2, updated_at = ?3, name = COALESCE(json_extract(?1, '$.name'), name) WHERE id = ?4",
        params![def_json, next, now, surface_id],
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
