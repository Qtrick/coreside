//! Repository helpers for settings, conversations, messages, tools, and state.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{now_rfc3339, Database, DbError, DbResult, DEFAULT_WORKSPACE_ID};
use crate::ai::{layout_type_string, ToolDefinition};
use crate::automations::{
    Automation, AutomationAction, AutomationRun, AutomationTrigger, MissedRunPolicy,
};
use crate::settings::{validate_added_setting_id, AddedSettingRecord, UpsertAddedSettingInput};

// ── Models ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRecord {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    pub layout: String,
    pub definition: ToolDefinition,
    pub current_version: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersionRecord {
    pub id: String,
    pub tool_id: String,
    pub version: i64,
    pub definition: ToolDefinition,
    pub change_summary: Option<String>,
    pub created_at: String,
}

// ── Settings ────────────────────────────────────────────────────────────────

pub fn get_settings(db: &Database) -> DbResult<HashMap<String, String>> {
    let mut stmt = db.conn().prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (k, v) = row?;
        map.insert(k, v);
    }
    Ok(map)
}

pub fn set_setting(db: &mut Database, key: &str, value: &str) -> DbResult<()> {
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now],
    )?;
    Ok(())
}

pub fn get_setting(db: &Database, key: &str) -> DbResult<Option<String>> {
    let mut map = get_settings(db)?;
    Ok(map.remove(key))
}

// ── Workspaces ──────────────────────────────────────────────────────────────

pub fn ensure_default_workspace(db: &Database) -> DbResult<String> {
    let exists: Option<String> = db
        .conn()
        .query_row(
            "SELECT id FROM workspaces WHERE id = ?1",
            [DEFAULT_WORKSPACE_ID],
            |r| r.get(0),
        )
        .optional()?;
    if exists.is_some() {
        return Ok(DEFAULT_WORKSPACE_ID.to_string());
    }
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO workspaces (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![DEFAULT_WORKSPACE_ID, "Personal", now, now],
    )?;
    Ok(DEFAULT_WORKSPACE_ID.to_string())
}

// ── Conversations ───────────────────────────────────────────────────────────

pub fn list_conversations(db: &Database, workspace_id: Option<&str>) -> DbResult<Vec<Conversation>> {
    let ws = workspace_id.unwrap_or(DEFAULT_WORKSPACE_ID);
    let mut stmt = db.conn().prepare(
        "SELECT id, workspace_id, title, project_id, pinned, archived, created_at, updated_at
         FROM conversations WHERE workspace_id = ?1
         ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([ws], map_conversation_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub(crate) fn map_conversation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    let pinned: i64 = row.get(4)?;
    let archived: i64 = row.get(5)?;
    Ok(Conversation {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        title: row.get(2)?,
        project_id: row.get(3)?,
        pinned: pinned != 0,
        archived: archived != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub fn create_conversation(
    db: &mut Database,
    workspace_id: &str,
    title: &str,
    project_id: Option<&str>,
) -> DbResult<Conversation> {
    let id = format!("conv-{}", Uuid::new_v4());
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO conversations (id, workspace_id, title, project_id, pinned, archived, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 0, 0, ?5, ?6)",
        params![id, workspace_id, title, project_id, now, now],
    )?;
    Ok(Conversation {
        id,
        workspace_id: workspace_id.to_string(),
        title: title.to_string(),
        project_id: project_id.map(|s| s.to_string()),
        pinned: false,
        archived: false,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn delete_conversation(db: &mut Database, id: &str) -> DbResult<()> {
    // Fail closed: never leave orphan FTS rows that could leak deleted chat text.
    crate::projects::remove_conversation_from_index(db, id)?;
    let n = db
        .conn()
        .execute("DELETE FROM conversations WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(DbError::NotFound(format!("conversation {id}")));
    }
    Ok(())
}

pub fn touch_conversation(db: &mut Database, id: &str) -> DbResult<()> {
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Rename a conversation when it still has a default placeholder title.
pub fn maybe_rename_conversation_from_message(
    db: &mut Database,
    id: &str,
    content: &str,
) -> DbResult<Option<String>> {
    let current = get_conversation(db, id)?;
    let is_default = current.title == "New chat"
        || current.title == "New conversation"
        || current.title.trim().is_empty();
    if !is_default {
        return Ok(None);
    }
    let title = title_from_content(content);
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title, now, id],
    )?;
    // Keep FTS title column in sync for project-scoped search.
    if current.project_id.is_some() {
        crate::projects::remove_conversation_from_index(db, id)?;
        for message in get_messages(db, id)? {
            crate::projects::index_message_for_conversation(db, &message)?;
        }
    }
    Ok(Some(title))
}

fn title_from_content(content: &str) -> String {
    let trimmed = content.trim().replace('\n', " ");
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return "New chat".into();
    }
    if collapsed.chars().count() > 48 {
        let truncated: String = collapsed.chars().take(45).collect();
        format!("{truncated}…")
    } else {
        collapsed
    }
}

pub fn clear_conversations(db: &mut Database) -> DbResult<u64> {
    db.with_transaction(|conn| {
        // message_fts has no FK to conversations — must clear explicitly.
        conn.execute("DELETE FROM message_fts", [])?;
        conn.execute("DELETE FROM messages", [])?;
        let n = conn.execute("DELETE FROM conversations", [])?;
        Ok(n as u64)
    })
}

pub fn get_conversation(db: &Database, id: &str) -> DbResult<Conversation> {
    db.conn()
        .query_row(
            "SELECT id, workspace_id, title, project_id, pinned, archived, created_at, updated_at
             FROM conversations WHERE id = ?1",
            [id],
            map_conversation_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("conversation {id}")),
            other => DbError::Sqlite(other),
        })
}

// ── Messages ────────────────────────────────────────────────────────────────

pub fn get_messages(db: &Database, conversation_id: &str) -> DbResult<Vec<Message>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, conversation_id, role, content, metadata, created_at
         FROM messages WHERE conversation_id = ?1
         ORDER BY created_at ASC, rowid ASC",
    )?;
    let rows = stmt.query_map([conversation_id], |row| {
        let meta: Option<String> = row.get(4)?;
        Ok(Message {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            metadata: meta
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            created_at: row.get(5)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_recent_messages(
    db: &Database,
    conversation_id: &str,
    limit: usize,
) -> DbResult<Vec<Message>> {
    let mut all = get_messages(db, conversation_id)?;
    if all.len() > limit {
        all = all.split_off(all.len() - limit);
    }
    Ok(all)
}

pub fn insert_message(
    db: &mut Database,
    conversation_id: &str,
    role: &str,
    content: &str,
    metadata: Option<&serde_json::Value>,
) -> DbResult<Message> {
    let id = format!("msg-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let meta_str = metadata.map(|m| m.to_string());
    db.conn().execute(
        "INSERT INTO messages (id, conversation_id, role, content, metadata, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, conversation_id, role, content, meta_str, now],
    )?;
    touch_conversation(db, conversation_id)?;
    let message = Message {
        id,
        conversation_id: conversation_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        metadata: metadata.cloned(),
        created_at: now,
    };
    if let Err(e) = crate::projects::index_message_for_conversation(db, &message) {
        tracing::warn!(error = %e, message_id = %message.id, "failed to index message");
    }
    Ok(message)
}

pub fn get_message(db: &Database, id: &str) -> DbResult<Message> {
    db.conn()
        .query_row(
            "SELECT id, conversation_id, role, content, metadata, created_at FROM messages WHERE id = ?1",
            [id],
            |row| {
                let meta: Option<String> = row.get(4)?;
                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    metadata: meta
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok()),
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("message {id}")),
            other => DbError::Sqlite(other),
        })
}

pub fn update_message_metadata(
    db: &mut Database,
    id: &str,
    metadata: &serde_json::Value,
) -> DbResult<()> {
    let n = db.conn().execute(
        "UPDATE messages SET metadata = ?1 WHERE id = ?2",
        params![metadata.to_string(), id],
    )?;
    if n == 0 {
        return Err(DbError::NotFound(format!("message {id}")));
    }
    Ok(())
}

/// Delete `message_id` and every later message in the same conversation (by created_at, then id).
pub fn delete_messages_from(db: &mut Database, message_id: &str) -> DbResult<u64> {
    let msg = get_message(db, message_id)?;
    let n = db.conn().execute(
        "DELETE FROM messages
         WHERE conversation_id = ?1
           AND (created_at > ?2 OR (created_at = ?2 AND id >= ?3))",
        params![msg.conversation_id, msg.created_at, msg.id],
    )?;
    touch_conversation(db, &msg.conversation_id)?;
    Ok(n as u64)
}

// ── Tools ───────────────────────────────────────────────────────────────────

fn parse_tool_row(
    id: String,
    workspace_id: String,
    name: String,
    description: String,
    layout: String,
    definition_json: String,
    current_version: i64,
    created_at: String,
    updated_at: String,
) -> DbResult<ToolRecord> {
    let mut definition: ToolDefinition = serde_json::from_str(&definition_json)?;
    definition.normalize_for_frontend();
    Ok(ToolRecord {
        id,
        workspace_id,
        name,
        description,
        layout,
        definition,
        current_version,
        created_at,
        updated_at,
    })
}

pub fn list_tools(db: &Database, workspace_id: Option<&str>) -> DbResult<Vec<ToolRecord>> {
    let ws = workspace_id.unwrap_or(DEFAULT_WORKSPACE_ID);
    let mut stmt = db.conn().prepare(
        "SELECT id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at
         FROM tools WHERE workspace_id = ?1 ORDER BY updated_at DESC",
    )?;
    let mut out = Vec::new();
    let mut rows = stmt.query([ws])?;
    while let Some(row) = rows.next()? {
        out.push(parse_tool_row(
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
            row.get(8)?,
        )?);
    }
    Ok(out)
}

pub fn get_tool(db: &Database, id: &str) -> DbResult<ToolRecord> {
    db.conn()
        .query_row(
            "SELECT id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at
             FROM tools WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("tool {id}")),
            other => DbError::Sqlite(other),
        })
        .and_then(|(a, b, c, d, e, f, g, h, i)| parse_tool_row(a, b, c, d, e, f, g, h, i))
}

pub fn get_tool_versions(db: &Database, tool_id: &str) -> DbResult<Vec<ToolVersionRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, tool_id, version, definition_json, change_summary, created_at
         FROM tool_versions WHERE tool_id = ?1 ORDER BY version DESC",
    )?;
    let mut out = Vec::new();
    let mut rows = stmt.query([tool_id])?;
    while let Some(row) = rows.next()? {
        let definition_json: String = row.get(3)?;
        out.push(ToolVersionRecord {
            id: row.get(0)?,
            tool_id: row.get(1)?,
            version: row.get(2)?,
            definition: serde_json::from_str(&definition_json)?,
            change_summary: row.get(4)?,
            created_at: row.get(5)?,
        });
    }
    Ok(out)
}

/// Create or update a tool and append a version row (transactional).
pub fn apply_tool_change(
    db: &mut Database,
    workspace_id: &str,
    definition: &ToolDefinition,
    action: &str,
    target_tool_id: Option<&str>,
    change_summary: &str,
) -> DbResult<ToolRecord> {
    crate::security::assert_not_protected(&definition.id).map_err(DbError::Invalid)?;
    if let Some(target) = target_tool_id {
        crate::security::assert_not_protected(target).map_err(DbError::Invalid)?;
    }

    db.with_transaction(|conn| {
        let now = now_rfc3339();
        let tool_id = match action {
            "create" => definition.id.clone(),
            "update" | "replace" => target_tool_id
                .filter(|s| !s.is_empty())
                .unwrap_or(definition.id.as_str())
                .to_string(),
            other => {
                return Err(DbError::Invalid(format!("Unknown tool action: {other}")));
            }
        };

        let mut def = definition.clone();
        def.id = tool_id.clone();
        def.normalize_for_frontend();
        let def_json = serde_json::to_string(&def)?;
        let layout_str = layout_type_string(&def.layout);

        let existing: Option<(i64,)> = conn
            .query_row(
                "SELECT current_version FROM tools WHERE id = ?1",
                [&tool_id],
                |r| Ok((r.get(0)?,)),
            )
            .optional()?;

        let new_version = if let Some((ver,)) = existing {
            let next = ver + 1;
            conn.execute(
                "UPDATE tools SET name = ?1, description = ?2, layout = ?3, definition_json = ?4,
                 current_version = ?5, updated_at = ?6 WHERE id = ?7",
                params![
                    def.name,
                    def.description,
                    layout_str,
                    def_json,
                    next,
                    now,
                    tool_id
                ],
            )?;
            next
        } else {
            conn.execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)",
                params![
                    tool_id,
                    workspace_id,
                    def.name,
                    def.description,
                    layout_str,
                    def_json,
                    now,
                    now
                ],
            )?;
            1
        };

        let version_id = format!("tv-{}", Uuid::new_v4());
        conn.execute(
            "INSERT INTO tool_versions (id, tool_id, version, definition_json, change_summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                version_id,
                tool_id,
                new_version,
                serde_json::to_string(&def)?,
                change_summary,
                now
            ],
        )?;

        // Re-read via connection
        let row = conn.query_row(
            "SELECT id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at
             FROM tools WHERE id = ?1",
            [&tool_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )?;
        parse_tool_row(row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8)
    })
}

/// Undo to previous version (transactional). Deletes current version row.
pub fn undo_tool_change(db: &mut Database, tool_id: &str) -> DbResult<ToolRecord> {
    db.with_transaction(|conn| {
        let current: i64 = conn
            .query_row(
                "SELECT current_version FROM tools WHERE id = ?1",
                [tool_id],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    DbError::NotFound(format!("tool {tool_id}"))
                }
                other => DbError::Sqlite(other),
            })?;

        if current <= 1 {
            return Err(DbError::Invalid(
                "Cannot undo: tool is at the first version".into(),
            ));
        }

        let prev_version = current - 1;
        let (def_json,): (String,) = conn.query_row(
            "SELECT definition_json FROM tool_versions WHERE tool_id = ?1 AND version = ?2",
            params![tool_id, prev_version],
            |r| Ok((r.get(0)?,)),
        )?;

        let mut def: ToolDefinition = serde_json::from_str(&def_json)?;
        def.normalize_for_frontend();
        let def_json = serde_json::to_string(&def)?;
        let layout_str = layout_type_string(&def.layout);
        let now = now_rfc3339();

        conn.execute(
            "UPDATE tools SET name = ?1, description = ?2, layout = ?3, definition_json = ?4,
             current_version = ?5, updated_at = ?6 WHERE id = ?7",
            params![
                def.name,
                def.description,
                layout_str,
                def_json,
                prev_version,
                now,
                tool_id
            ],
        )?;

        conn.execute(
            "DELETE FROM tool_versions WHERE tool_id = ?1 AND version = ?2",
            params![tool_id, current],
        )?;

        let row = conn.query_row(
            "SELECT id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at
             FROM tools WHERE id = ?1",
            [tool_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )?;
        parse_tool_row(row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8)
    })
}

pub fn clear_tools(db: &mut Database) -> DbResult<u64> {
    db.with_transaction(|conn| {
        conn.execute("DELETE FROM tool_state", [])?;
        conn.execute("DELETE FROM tool_versions", [])?;
        let n = conn.execute("DELETE FROM tools", [])?;
        Ok(n as u64)
    })
}

// ── Tool state ──────────────────────────────────────────────────────────────

pub fn save_tool_state(
    db: &mut Database,
    tool_id: &str,
    state: &serde_json::Value,
) -> DbResult<()> {
    // Ensure tool exists
    let _: String = db
        .conn()
        .query_row("SELECT id FROM tools WHERE id = ?1", [tool_id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("tool {tool_id}")),
            other => DbError::Sqlite(other),
        })?;

    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO tool_state (tool_id, state_json, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(tool_id) DO UPDATE SET state_json = excluded.state_json, updated_at = excluded.updated_at",
        params![tool_id, state.to_string(), now],
    )?;
    Ok(())
}

pub fn get_tool_state(db: &Database, tool_id: &str) -> DbResult<Option<serde_json::Value>> {
    let row: Option<String> = db
        .conn()
        .query_row(
            "SELECT state_json FROM tool_state WHERE tool_id = ?1",
            [tool_id],
            |r| r.get(0),
        )
        .optional()?;
    match row {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

// ── Added settings ──────────────────────────────────────────────────────────

fn parse_json_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or(Value::Null)
}

fn map_added_setting_row(
    id: String,
    owner_tool_id: Option<String>,
    label: String,
    description: String,
    setting_type: String,
    default_value: String,
    current_value: String,
    constraints: String,
    version: i64,
    created_at: String,
    updated_at: String,
) -> AddedSettingRecord {
    AddedSettingRecord {
        id,
        owner_tool_id,
        label,
        description,
        setting_type,
        default_value: parse_json_value(&default_value),
        current_value: parse_json_value(&current_value),
        constraints: parse_json_value(&constraints),
        version,
        created_at,
        updated_at,
    }
}

pub fn list_added_settings(
    db: &Database,
    owner_tool_id: Option<&str>,
) -> DbResult<Vec<AddedSettingRecord>> {
    let mut out = Vec::new();
    if let Some(owner) = owner_tool_id {
        let mut stmt = db.conn().prepare(
            "SELECT id, owner_tool_id, label, description, setting_type,
                    default_value, current_value, constraints, version, created_at, updated_at
             FROM added_settings WHERE owner_tool_id = ?1
             ORDER BY label ASC, id ASC",
        )?;
        let rows = stmt.query_map([owner], |row| {
            Ok(map_added_setting_row(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ))
        })?;
        for row in rows {
            out.push(row?);
        }
    } else {
        let mut stmt = db.conn().prepare(
            "SELECT id, owner_tool_id, label, description, setting_type,
                    default_value, current_value, constraints, version, created_at, updated_at
             FROM added_settings
             ORDER BY label ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(map_added_setting_row(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ))
        })?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}

pub fn upsert_added_setting(
    db: &mut Database,
    input: &UpsertAddedSettingInput,
) -> DbResult<AddedSettingRecord> {
    validate_added_setting_id(&input.id).map_err(DbError::Invalid)?;

    let now = now_rfc3339();
    let description = input.description.clone().unwrap_or_default();
    let default_value = input
        .default_value
        .clone()
        .unwrap_or(Value::Null)
        .to_string();
    let current_value = input
        .current_value
        .clone()
        .unwrap_or_else(|| {
            input
                .default_value
                .clone()
                .unwrap_or(Value::Null)
        })
        .to_string();
    let constraints = input
        .constraints
        .clone()
        .unwrap_or_else(|| serde_json::json!({}))
        .to_string();

    // Optional FK: only set owner when the tool exists.
    let owner = match input.owner_tool_id.as_deref() {
        Some(oid) if !oid.trim().is_empty() => {
            let exists: Option<String> = db
                .conn()
                .query_row("SELECT id FROM tools WHERE id = ?1", [oid], |r| r.get(0))
                .optional()?;
            exists
        }
        _ => None,
    };

    let existing: Option<(i64, String)> = db
        .conn()
        .query_row(
            "SELECT version, created_at FROM added_settings WHERE id = ?1",
            [&input.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;

    let (version, created_at) = if let Some((ver, created)) = existing {
        let next = ver + 1;
        db.conn().execute(
            "UPDATE added_settings SET owner_tool_id = ?1, label = ?2, description = ?3,
             setting_type = ?4, default_value = ?5, current_value = ?6, constraints = ?7,
             version = ?8, updated_at = ?9 WHERE id = ?10",
            params![
                owner,
                input.label,
                description,
                input.setting_type,
                default_value,
                current_value,
                constraints,
                next,
                now,
                input.id
            ],
        )?;
        (next, created)
    } else {
        db.conn().execute(
            "INSERT INTO added_settings (
                id, owner_tool_id, label, description, setting_type,
                default_value, current_value, constraints, version, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10)",
            params![
                input.id,
                owner,
                input.label,
                description,
                input.setting_type,
                default_value,
                current_value,
                constraints,
                now,
                now
            ],
        )?;
        (1, now.clone())
    };

    Ok(AddedSettingRecord {
        id: input.id.clone(),
        owner_tool_id: owner,
        label: input.label.clone(),
        description,
        setting_type: input.setting_type.clone(),
        default_value: parse_json_value(&default_value),
        current_value: parse_json_value(&current_value),
        constraints: parse_json_value(&constraints),
        version,
        created_at,
        updated_at: now,
    })
}

pub fn delete_added_setting(db: &mut Database, id: &str) -> DbResult<()> {
    validate_added_setting_id(id).map_err(DbError::Invalid)?;
    let n = db
        .conn()
        .execute("DELETE FROM added_settings WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(DbError::NotFound(format!("added setting {id}")));
    }
    Ok(())
}

// ── Provider connections (metadata only) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnection {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub base_url: Option<String>,
    pub model_default: Option<String>,
    pub keyring_account: String,
    pub is_active: bool,
    pub last_status: Option<String>,
    pub last_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn map_provider_connection(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderConnection> {
    Ok(ProviderConnection {
        id: row.get(0)?,
        provider: row.get(1)?,
        label: row.get(2)?,
        base_url: row.get(3)?,
        model_default: row.get(4)?,
        keyring_account: row.get(5)?,
        is_active: row.get::<_, i64>(6)? != 0,
        last_status: row.get(7)?,
        last_tested_at: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

const PROVIDER_CONNECTION_COLS: &str = "id, provider, label, base_url, model_default, keyring_account,
    is_active, last_status, last_tested_at, created_at, updated_at";

pub fn list_provider_connections(db: &Database) -> DbResult<Vec<ProviderConnection>> {
    let mut stmt = db.conn().prepare(&format!(
        "SELECT {PROVIDER_CONNECTION_COLS} FROM provider_connections ORDER BY updated_at DESC"
    ))?;
    let rows = stmt.query_map([], map_provider_connection)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get_provider_connection(db: &Database, id: &str) -> DbResult<ProviderConnection> {
    db.conn()
        .query_row(
            &format!("SELECT {PROVIDER_CONNECTION_COLS} FROM provider_connections WHERE id = ?1"),
            [id],
            map_provider_connection,
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("provider connection {id}")))
}

pub fn get_active_provider_connection(db: &Database) -> DbResult<Option<ProviderConnection>> {
    db.conn()
        .query_row(
            &format!(
                "SELECT {PROVIDER_CONNECTION_COLS} FROM provider_connections WHERE is_active = 1 LIMIT 1"
            ),
            [],
            map_provider_connection,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn upsert_provider_connection(
    db: &mut Database,
    connection: &ProviderConnection,
) -> DbResult<ProviderConnection> {
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO provider_connections (
            id, provider, label, base_url, model_default, keyring_account,
            is_active, last_status, last_tested_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
            provider = excluded.provider,
            label = excluded.label,
            base_url = excluded.base_url,
            model_default = excluded.model_default,
            keyring_account = excluded.keyring_account,
            is_active = excluded.is_active,
            last_status = excluded.last_status,
            last_tested_at = excluded.last_tested_at,
            updated_at = excluded.updated_at",
        params![
            connection.id,
            connection.provider,
            connection.label,
            connection.base_url,
            connection.model_default,
            connection.keyring_account,
            if connection.is_active { 1 } else { 0 },
            connection.last_status,
            connection.last_tested_at,
            connection.created_at.clone(),
            now,
        ],
    )?;
    get_provider_connection(db, &connection.id)
}

pub fn set_active_provider_connection(
    db: &mut Database,
    id: &str,
) -> DbResult<ProviderConnection> {
    // Ensure the target exists before mutating.
    let _ = get_provider_connection(db, id)?;
    db.with_transaction(|conn| {
        conn.execute("UPDATE provider_connections SET is_active = 0", [])?;
        let n = conn.execute(
            "UPDATE provider_connections SET is_active = 1, updated_at = ?1 WHERE id = ?2",
            params![now_rfc3339(), id],
        )?;
        if n == 0 {
            return Err(DbError::NotFound(format!("provider connection {id}")));
        }
        Ok(())
    })?;
    set_setting(db, "activeProviderConnectionId", id)?;
    get_provider_connection(db, id)
}

pub fn clear_active_provider_connection(db: &mut Database) -> DbResult<()> {
    db.conn()
        .execute("UPDATE provider_connections SET is_active = 0", [])?;
    set_setting(db, "activeProviderConnectionId", "")?;
    Ok(())
}

pub fn delete_provider_connection(db: &mut Database, id: &str) -> DbResult<()> {
    let existing = get_provider_connection(db, id)?;
    let n = db
        .conn()
        .execute("DELETE FROM provider_connections WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(DbError::NotFound(format!("provider connection {id}")));
    }
    if existing.is_active {
        set_setting(db, "activeProviderConnectionId", "")?;
    }
    Ok(())
}

pub fn update_provider_connection_status(
    db: &mut Database,
    id: &str,
    status: &str,
) -> DbResult<()> {
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE provider_connections SET last_status = ?1, last_tested_at = ?2, updated_at = ?2 WHERE id = ?3",
        params![status, now, id],
    )?;
    if n == 0 {
        return Err(DbError::NotFound(format!("provider connection {id}")));
    }
    Ok(())
}

// ── Action Log events (sanitized only) ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionEvent {
    pub id: String,
    pub request_id: String,
    pub conversation_id: Option<String>,
    pub event_type: String,
    pub label: String,
    pub status: String,
    pub metadata_json: Option<String>,
    pub sequence: i64,
    pub created_at: String,
}

/// Action Log Base Setting modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionLogMode {
    Off,
    Always,
    Intelligent,
}

impl ActionLogMode {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "always" | "on" | "true" | "1" | "yes" => Self::Always,
            "intelligent" | "auto" | "smart" => Self::Intelligent,
            _ => Self::Off,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Always => "always",
            Self::Intelligent => "intelligent",
        }
    }

    pub fn collects(self) -> bool {
        !matches!(self, Self::Off)
    }
}

pub fn action_log_mode(db: &Database) -> ActionLogMode {
    match get_settings(db) {
        Ok(map) => {
            if let Some(mode) = map
                .get("actionLogMode")
                .or_else(|| map.get("action_log_mode"))
            {
                return ActionLogMode::parse(mode);
            }
            // Legacy boolean.
            map.get("actionLogEnabled")
                .or_else(|| map.get("action_log_enabled"))
                .map(|v| ActionLogMode::parse(v))
                .unwrap_or(ActionLogMode::Off)
        }
        Err(_) => ActionLogMode::Off,
    }
}

pub fn is_action_log_enabled(db: &Database) -> bool {
    action_log_mode(db).collects()
}

/// Whether Action Log UI should surface these events for the active mode.
pub fn action_events_are_substantive(events: &[serde_json::Value]) -> bool {
    const SUBSTANTIVE: &[&str] = &[
        "tool_started",
        "tool_completed",
        "tool_failed",
        "tool_reference_resolved",
        "change_applied",
        "tool_change_proposed",
        "search_started",
        "search_completed",
        "crawl_started",
        "crawl_completed",
        "project_context_loaded",
    ];
    events.iter().any(|evt| {
        let Some(obj) = evt.as_object() else {
            return false;
        };
        let ty = obj
            .get("eventType")
            .or_else(|| obj.get("event_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if SUBSTANTIVE.iter().any(|s| *s == ty) {
            return true;
        }
        // Labels that indicate real work when type is generic.
        let label = obj
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        label.contains("search")
            || label.contains("crawl")
            || label.contains("tool")
            || label.contains("propos")
            || label.contains("appear")
            || label.contains("import")
            || label.contains("fetch")
            || label.contains("project")
    })
}

pub fn should_attach_action_events(mode: ActionLogMode, events: &[serde_json::Value]) -> bool {
    match mode {
        ActionLogMode::Off => false,
        ActionLogMode::Always => !events.is_empty(),
        ActionLogMode::Intelligent => action_events_are_substantive(events),
    }
}

pub fn insert_action_event(
    db: &mut Database,
    request_id: &str,
    conversation_id: Option<&str>,
    event_type: &str,
    label: &str,
    status: &str,
    sequence: i64,
) -> DbResult<ActionEvent> {
    let id = format!("act-{}", Uuid::new_v4());
    let created_at = now_rfc3339();
    // Labels only — never persist prompts, CoT, keys, or free-form metadata blobs.
    let safe_label = {
        let redacted = crate::security::redact_secrets(label, None);
        let trimmed = redacted.trim();
        if trimmed.is_empty() {
            "Action".to_string()
        } else if trimmed.chars().count() > 160 {
            let truncated: String = trimmed.chars().take(160).collect();
            format!("{truncated}…")
        } else {
            trimmed.to_string()
        }
    };
    let safe_status = match status.trim() {
        "completed" | "failed" | "cancelled" | "running" => status.trim().to_string(),
        _ => "completed".to_string(),
    };
    db.conn().execute(
        "INSERT INTO action_events (
            id, request_id, conversation_id, event_type, label, status,
            metadata_json, sequence, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)",
        params![
            id,
            request_id,
            conversation_id,
            event_type,
            safe_label,
            safe_status,
            sequence,
            created_at
        ],
    )?;
    Ok(ActionEvent {
        id,
        request_id: request_id.to_string(),
        conversation_id: conversation_id.map(str::to_string),
        event_type: event_type.to_string(),
        label: safe_label,
        status: safe_status,
        metadata_json: None,
        sequence,
        created_at,
    })
}

pub fn list_action_events_for_request(
    db: &Database,
    request_id: &str,
) -> DbResult<Vec<ActionEvent>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, request_id, conversation_id, event_type, label, status,
                metadata_json, sequence, created_at
         FROM action_events
         WHERE request_id = ?1
         ORDER BY sequence ASC, created_at ASC",
    )?;
    let rows = stmt.query_map([request_id], |row| {
        Ok(ActionEvent {
            id: row.get(0)?,
            request_id: row.get(1)?,
            conversation_id: row.get(2)?,
            event_type: row.get(3)?,
            label: row.get(4)?,
            status: row.get(5)?,
            metadata_json: row.get(6)?,
            sequence: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

// ── Automations & workspace backgrounds ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBackground {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub kind: String,
    pub definition_json: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

fn parse_missed_policy(raw: &str) -> MissedRunPolicy {
    match raw.trim().to_lowercase().as_str() {
        "skip" => MissedRunPolicy::Skip,
        _ => MissedRunPolicy::RunOnce,
    }
}

fn map_automation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Automation> {
    let trigger_json: String = row.get(6)?;
    let action_json: String = row.get(8)?;
    let trigger: AutomationTrigger = serde_json::from_str(&trigger_json).unwrap_or(
        AutomationTrigger::Interval {
            interval_minutes: 60,
            timezone: None,
        },
    );
    let action: AutomationAction = serde_json::from_str(&action_json).unwrap_or(
        AutomationAction::SetWorkspaceBackground {
            workspace_id: DEFAULT_WORKSPACE_ID.into(),
            preset_id: String::new(),
        },
    );
    let policy: String = row.get(11)?;
    Ok(Automation {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        owner_tool_id: row.get(2)?,
        name: row.get(3)?,
        enabled: row.get::<_, i64>(4)? == 1,
        trigger,
        action,
        requires_ai: row.get::<_, i64>(9)? == 1,
        provider_connection_id: row.get(10)?,
        missed_run_policy: parse_missed_policy(&policy),
        next_run_at: row.get(12)?,
        last_run_at: row.get(13)?,
        last_status: row.get(14)?,
        consecutive_failures: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

const AUTOMATION_COLS: &str = "id, workspace_id, owner_tool_id, name, enabled,
    trigger_type, trigger_json, action_type, action_json, requires_ai,
    provider_connection_id, missed_run_policy, next_run_at, last_run_at,
    last_status, consecutive_failures, created_at, updated_at";

pub fn list_automations(db: &Database) -> DbResult<Vec<Automation>> {
    let mut stmt = db.conn().prepare(&format!(
        "SELECT {AUTOMATION_COLS} FROM automations ORDER BY updated_at DESC"
    ))?;
    let rows = stmt.query_map([], map_automation)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_automation(db: &Database, id: &str) -> DbResult<Automation> {
    db.conn()
        .query_row(
            &format!("SELECT {AUTOMATION_COLS} FROM automations WHERE id = ?1"),
            [id],
            map_automation,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("automation {id}")),
            other => DbError::Sqlite(other),
        })
}

pub fn list_due_automations(db: &Database, now_rfc3339: &str) -> DbResult<Vec<Automation>> {
    let mut stmt = db.conn().prepare(&format!(
        "SELECT {AUTOMATION_COLS} FROM automations
         WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= ?1
         ORDER BY next_run_at ASC"
    ))?;
    let rows = stmt.query_map([now_rfc3339], map_automation)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn upsert_automation(db: &mut Database, automation: &Automation) -> DbResult<Automation> {
    let trigger_type = match &automation.trigger {
        AutomationTrigger::Interval { .. } => "interval",
        AutomationTrigger::Daily { .. } => "daily",
        AutomationTrigger::Weekly { .. } => "weekly",
    };
    let action_type = match &automation.action {
        AutomationAction::CycleWorkspaceBackgrounds { .. } => "cycle_workspace_backgrounds",
        AutomationAction::SetWorkspaceBackground { .. } => "set_workspace_background",
        AutomationAction::SetToolStateValue { .. } => "set_tool_state_value",
        AutomationAction::AiPrompt { .. } => "ai_prompt",
    };
    let trigger_json = serde_json::to_string(&automation.trigger)?;
    let action_json = serde_json::to_string(&automation.action)?;
    let policy = match automation.missed_run_policy {
        MissedRunPolicy::Skip => "skip",
        MissedRunPolicy::RunOnce => "run_once",
    };
    db.conn().execute(
        "INSERT INTO automations (
            id, workspace_id, owner_tool_id, name, enabled, trigger_type, trigger_json,
            action_type, action_json, requires_ai, provider_connection_id, missed_run_policy,
            next_run_at, last_run_at, last_status, consecutive_failures, created_at, updated_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
         ON CONFLICT(id) DO UPDATE SET
            name=excluded.name, enabled=excluded.enabled, trigger_type=excluded.trigger_type,
            trigger_json=excluded.trigger_json, action_type=excluded.action_type,
            action_json=excluded.action_json, requires_ai=excluded.requires_ai,
            provider_connection_id=excluded.provider_connection_id,
            missed_run_policy=excluded.missed_run_policy, next_run_at=excluded.next_run_at,
            last_run_at=excluded.last_run_at, last_status=excluded.last_status,
            consecutive_failures=excluded.consecutive_failures, updated_at=excluded.updated_at",
        params![
            automation.id,
            automation.workspace_id,
            automation.owner_tool_id,
            automation.name,
            if automation.enabled { 1 } else { 0 },
            trigger_type,
            trigger_json,
            action_type,
            action_json,
            if automation.requires_ai { 1 } else { 0 },
            automation.provider_connection_id,
            policy,
            automation.next_run_at,
            automation.last_run_at,
            automation.last_status,
            automation.consecutive_failures,
            automation.created_at,
            automation.updated_at,
        ],
    )?;
    get_automation(db, &automation.id)
}

pub fn update_automation_schedule(
    db: &mut Database,
    id: &str,
    next_run_at: Option<&str>,
    last_run_at: Option<&str>,
    last_status: Option<&str>,
    consecutive_failures: i64,
    enabled: bool,
) -> DbResult<()> {
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE automations SET next_run_at=?1, last_run_at=?2, last_status=?3,
         consecutive_failures=?4, enabled=?5, updated_at=?6 WHERE id=?7",
        params![
            next_run_at,
            last_run_at,
            last_status,
            consecutive_failures,
            if enabled { 1 } else { 0 },
            now,
            id
        ],
    )?;
    if n == 0 {
        return Err(DbError::NotFound(format!("automation {id}")));
    }
    Ok(())
}

pub fn delete_automation(db: &mut Database, id: &str) -> DbResult<()> {
    let n = db
        .conn()
        .execute("DELETE FROM automations WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(DbError::NotFound(format!("automation {id}")));
    }
    Ok(())
}

pub fn insert_automation_run(
    db: &mut Database,
    id: &str,
    automation_id: &str,
    scheduled_at: Option<&str>,
    started_at: &str,
    completed_at: Option<&str>,
    status: &str,
    result_summary: Option<&str>,
    error_category: Option<&str>,
) -> DbResult<()> {
    let created_at = now_rfc3339();
    db.conn().execute(
        "INSERT INTO automation_runs (
            id, automation_id, scheduled_at, started_at, completed_at, status,
            result_summary, error_category, created_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            id,
            automation_id,
            scheduled_at,
            started_at,
            completed_at,
            status,
            result_summary,
            error_category,
            created_at
        ],
    )?;
    Ok(())
}

pub fn list_automation_runs(
    db: &Database,
    automation_id: &str,
    limit: usize,
) -> DbResult<Vec<AutomationRun>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, automation_id, scheduled_at, started_at, completed_at, status,
                result_summary, error_category, created_at
         FROM automation_runs WHERE automation_id = ?1
         ORDER BY created_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![automation_id, limit as i64], |row| {
        Ok(AutomationRun {
            id: row.get(0)?,
            automation_id: row.get(1)?,
            scheduled_at: row.get(2)?,
            started_at: row.get(3)?,
            completed_at: row.get(4)?,
            status: row.get(5)?,
            result_summary: row.get(6)?,
            error_category: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn list_workspace_backgrounds(
    db: &Database,
    workspace_id: &str,
) -> DbResult<Vec<WorkspaceBackground>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, workspace_id, name, kind, definition_json, sort_order, created_at, updated_at
         FROM workspace_backgrounds WHERE workspace_id = ?1 ORDER BY sort_order ASC, created_at ASC",
    )?;
    let rows = stmt.query_map([workspace_id], |row| {
        Ok(WorkspaceBackground {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            name: row.get(2)?,
            kind: row.get(3)?,
            definition_json: row.get(4)?,
            sort_order: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_workspace_background(db: &Database, id: &str) -> DbResult<WorkspaceBackground> {
    db.conn()
        .query_row(
            "SELECT id, workspace_id, name, kind, definition_json, sort_order, created_at, updated_at
             FROM workspace_backgrounds WHERE id = ?1",
            [id],
            |row| {
                Ok(WorkspaceBackground {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: row.get(3)?,
                    definition_json: row.get(4)?,
                    sort_order: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("workspace background {id}"))
            }
            other => DbError::Sqlite(other),
        })
}

pub fn upsert_workspace_background(
    db: &mut Database,
    bg: &WorkspaceBackground,
) -> DbResult<WorkspaceBackground> {
    db.conn().execute(
        "INSERT INTO workspace_backgrounds (
            id, workspace_id, name, kind, definition_json, sort_order, created_at, updated_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(id) DO UPDATE SET
            name=excluded.name, kind=excluded.kind, definition_json=excluded.definition_json,
            sort_order=excluded.sort_order, updated_at=excluded.updated_at",
        params![
            bg.id,
            bg.workspace_id,
            bg.name,
            bg.kind,
            bg.definition_json,
            bg.sort_order,
            bg.created_at,
            bg.updated_at
        ],
    )?;
    get_workspace_background(db, &bg.id)
}

#[cfg(test)]
mod action_log_mode_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_modes() {
        assert_eq!(ActionLogMode::parse("intelligent"), ActionLogMode::Intelligent);
        assert_eq!(ActionLogMode::parse("always"), ActionLogMode::Always);
        assert_eq!(ActionLogMode::parse("false"), ActionLogMode::Off);
    }

    #[test]
    fn intelligent_filters_boilerplate() {
        let boilerplate = vec![
            json!({"eventType": "request_started", "label": "Preparing your request"}),
            json!({"eventType": "context_loaded", "label": "Building agent context"}),
        ];
        assert!(!should_attach_action_events(
            ActionLogMode::Intelligent,
            &boilerplate
        ));
        let with_tool = vec![
            json!({"eventType": "request_started", "label": "Preparing your request"}),
            json!({"eventType": "tool_change_proposed", "label": "Preparing tool change preview"}),
        ];
        assert!(should_attach_action_events(
            ActionLogMode::Intelligent,
            &with_tool
        ));
        assert!(should_attach_action_events(
            ActionLogMode::Always,
            &boilerplate
        ));
    }
}

