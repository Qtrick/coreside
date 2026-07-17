//! Repository helpers for settings, conversations, messages, tools, and state.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{now_rfc3339, Database, DbError, DbResult, DEFAULT_WORKSPACE_ID};
use crate::ai::{layout_type_string, ToolDefinition};

// ── Models ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
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
        "SELECT id, workspace_id, title, created_at, updated_at
         FROM conversations WHERE workspace_id = ?1
         ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([ws], |row| {
        Ok(Conversation {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            title: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn create_conversation(
    db: &mut Database,
    workspace_id: &str,
    title: &str,
) -> DbResult<Conversation> {
    let id = format!("conv-{}", Uuid::new_v4());
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO conversations (id, workspace_id, title, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, workspace_id, title, now, now],
    )?;
    Ok(Conversation {
        id,
        workspace_id: workspace_id.to_string(),
        title: title.to_string(),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn delete_conversation(db: &mut Database, id: &str) -> DbResult<()> {
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
    let n = db.conn().execute("DELETE FROM messages", [])?;
    let _ = n;
    let n = db.conn().execute("DELETE FROM conversations", [])?;
    Ok(n as u64)
}

pub fn get_conversation(db: &Database, id: &str) -> DbResult<Conversation> {
    db.conn()
        .query_row(
            "SELECT id, workspace_id, title, created_at, updated_at FROM conversations WHERE id = ?1",
            [id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
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
    Ok(Message {
        id,
        conversation_id: conversation_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        metadata: metadata.cloned(),
        created_at: now,
    })
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
