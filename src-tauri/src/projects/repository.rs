//! Project persistence and conversation membership.

use rusqlite::params;
use uuid::Uuid;

use crate::db::{
    get_conversation, get_messages, map_conversation_row, Conversation, Database, DbError,
    DbResult, now_rfc3339, DEFAULT_WORKSPACE_ID,
};

use super::indexing::{index_message, remove_conversation_from_index};
use super::models::{
    CreateProjectInput, DeleteProjectMode, Project, UpdateProjectInput,
};
use super::validation::{validate_create_input, validate_update_fields};

fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let archived: i64 = row.get(7)?;
    let pinned: i64 = row.get(8)?;
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        icon_key: row.get(3)?,
        instructions: row.get(4)?,
        summary: row.get(5)?,
        summary_updated_at: row.get(6)?,
        archived: archived != 0,
        pinned: pinned != 0,
        wallpaper_json: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        last_opened_at: row.get(12)?,
    })
}

const PROJECT_COLUMNS: &str =
    "id, name, description, icon_key, instructions, summary, summary_updated_at, archived, pinned, wallpaper_json, created_at, updated_at, last_opened_at";

pub fn list_projects(db: &Database, include_archived: bool) -> DbResult<Vec<Project>> {
    let sql = if include_archived {
        format!(
            "SELECT {PROJECT_COLUMNS} FROM projects ORDER BY pinned DESC, updated_at DESC"
        )
    } else {
        format!(
            "SELECT {PROJECT_COLUMNS} FROM projects WHERE archived = 0 ORDER BY pinned DESC, updated_at DESC"
        )
    };
    let mut stmt = db.conn().prepare(&sql)?;
    let rows = stmt.query_map([], map_project_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_project(db: &Database, id: &str) -> DbResult<Project> {
    db.conn()
        .query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?1"),
            [id],
            map_project_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("project {id}")),
            other => DbError::Sqlite(other),
        })
}

pub fn create_project(db: &mut Database, input: &CreateProjectInput) -> DbResult<Project> {
    validate_create_input(input).map_err(DbError::Invalid)?;
    let id = format!("proj-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let name = input.name.trim();
    let pinned = if input.pinned.unwrap_or(false) { 1 } else { 0 };
    db.conn().execute(
        "INSERT INTO projects (
            id, name, description, icon_key, instructions, archived, pinned, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)",
        params![
            id,
            name,
            input.description.as_deref().map(str::trim),
            input.icon_key.as_deref().map(str::trim),
            input.instructions.as_deref().map(str::trim),
            pinned,
            now,
            now,
        ],
    )?;
    get_project(db, &id)
}

pub fn update_project(
    db: &mut Database,
    id: &str,
    input: &UpdateProjectInput,
) -> DbResult<Project> {
    let existing = get_project(db, id)?;
    validate_update_fields(
        input.name.as_deref(),
        input.description.as_deref().or(existing.description.as_deref()),
        input.instructions.as_deref().or(existing.instructions.as_deref()),
        input.icon_key.as_deref().or(existing.icon_key.as_deref()),
    )
    .map_err(DbError::Invalid)?;

    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .unwrap_or(existing.name.as_str());
    let description = input
        .description
        .as_ref()
        .map(|d| d.trim().to_string())
        .or(existing.description);
    let icon_key = input
        .icon_key
        .as_ref()
        .map(|d| d.trim().to_string())
        .or(existing.icon_key);
    let instructions = input
        .instructions
        .as_ref()
        .map(|d| d.trim().to_string())
        .or(existing.instructions);
    let pinned = input.pinned.unwrap_or(existing.pinned);
    let archived = input.archived.unwrap_or(existing.archived);
    let now = now_rfc3339();

    let n = db.conn().execute(
        "UPDATE projects SET
            name = ?1,
            description = ?2,
            icon_key = ?3,
            instructions = ?4,
            pinned = ?5,
            archived = ?6,
            updated_at = ?7
         WHERE id = ?8",
        params![
            name,
            description,
            icon_key,
            instructions,
            if pinned { 1 } else { 0 },
            if archived { 1 } else { 0 },
            now,
            id,
        ],
    )?;
    if n == 0 {
        return Err(DbError::NotFound(format!("project {id}")));
    }
    get_project(db, id)
}

pub fn archive_project(db: &mut Database, id: &str) -> DbResult<Project> {
    update_project(
        db,
        id,
        &UpdateProjectInput {
            name: None,
            description: None,
            icon_key: None,
            instructions: None,
            pinned: None,
            archived: Some(true),
        },
    )
}

pub fn restore_project(db: &mut Database, id: &str) -> DbResult<Project> {
    update_project(
        db,
        id,
        &UpdateProjectInput {
            name: None,
            description: None,
            icon_key: None,
            instructions: None,
            pinned: None,
            archived: Some(false),
        },
    )
}

pub fn delete_project(db: &mut Database, id: &str, mode: DeleteProjectMode) -> DbResult<()> {
    let _ = get_project(db, id)?;
    match mode {
        // Keep Chats is the safe default: clear project-scoped FTS, unassign
        // chats explicitly, then delete the project row (FK ON DELETE SET NULL
        // is defense-in-depth; explicit UPDATE keeps behavior correct even if
        // PRAGMA foreign_keys were ever off).
        DeleteProjectMode::KeepChats => db.with_transaction(|conn| {
            conn.execute("DELETE FROM message_fts WHERE project_id = ?1", [id])?;
            let now = now_rfc3339();
            conn.execute(
                "UPDATE conversations SET project_id = NULL, updated_at = ?1 WHERE project_id = ?2",
                params![now, id],
            )?;
            conn.execute(
                "UPDATE search_sessions SET project_id = NULL WHERE project_id = ?1",
                [id],
            )?;
            let n = conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
            if n == 0 {
                return Err(DbError::NotFound(format!("project {id}")));
            }
            Ok(())
        }),
        DeleteProjectMode::DeleteChats => db.with_transaction(|conn| {
            let mut stmt = conn.prepare("SELECT id FROM conversations WHERE project_id = ?1")?;
            let conversation_ids: Vec<String> = stmt
                .query_map([id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            drop(stmt);
            for conversation_id in &conversation_ids {
                conn.execute(
                    "DELETE FROM message_fts WHERE conversation_id = ?1",
                    [conversation_id],
                )?;
                conn.execute("DELETE FROM conversations WHERE id = ?1", [conversation_id])?;
            }
            conn.execute(
                "UPDATE search_sessions SET project_id = NULL WHERE project_id = ?1",
                [id],
            )?;
            let n = conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
            if n == 0 {
                return Err(DbError::NotFound(format!("project {id}")));
            }
            Ok(())
        }),
    }
}

pub fn assign_conversation_to_project(
    db: &mut Database,
    conversation_id: &str,
    project_id: &str,
) -> DbResult<Conversation> {
    let _ = get_project(db, project_id)?;
    let _ = get_conversation(db, conversation_id)?;
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE conversations SET project_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![project_id, now, conversation_id],
    )?;
    reindex_conversation_messages(db, conversation_id, project_id)?;
    get_conversation(db, conversation_id)
}

pub fn remove_conversation_from_project(
    db: &mut Database,
    conversation_id: &str,
) -> DbResult<Conversation> {
    let _ = get_conversation(db, conversation_id)?;
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE conversations SET project_id = NULL, updated_at = ?1 WHERE id = ?2",
        params![now, conversation_id],
    )?;
    remove_conversation_from_index(db, conversation_id)?;
    get_conversation(db, conversation_id)
}

pub fn list_project_conversations(db: &Database, project_id: &str) -> DbResult<Vec<Conversation>> {
    let _ = get_project(db, project_id)?;
    let mut stmt = db.conn().prepare(
        "SELECT id, workspace_id, title, project_id, pinned, archived, created_at, updated_at
         FROM conversations
         WHERE project_id = ?1
         ORDER BY pinned DESC, updated_at DESC",
    )?;
    let rows = stmt.query_map([project_id], map_conversation_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn list_unassigned_conversations(
    db: &Database,
    workspace_id: Option<&str>,
) -> DbResult<Vec<Conversation>> {
    let ws = workspace_id.unwrap_or(DEFAULT_WORKSPACE_ID);
    let mut stmt = db.conn().prepare(
        "SELECT id, workspace_id, title, project_id, pinned, archived, created_at, updated_at
         FROM conversations
         WHERE workspace_id = ?1 AND project_id IS NULL
         ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([ws], map_conversation_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn move_conversations_to_project(
    db: &mut Database,
    conversation_ids: &[String],
    project_id: &str,
) -> DbResult<Vec<Conversation>> {
    if conversation_ids.is_empty() {
        return Ok(Vec::new());
    }
    let _ = get_project(db, project_id)?;
    db.with_transaction(|conn| {
        let now = now_rfc3339();
        for conversation_id in conversation_ids {
            let n = conn.execute(
                "UPDATE conversations SET project_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![project_id, now, conversation_id],
            )?;
            if n == 0 {
                return Err(DbError::NotFound(format!("conversation {conversation_id}")));
            }
        }
        Ok(())
    })?;
    for conversation_id in conversation_ids {
        reindex_conversation_messages(db, conversation_id, project_id)?;
    }
    let mut out = Vec::with_capacity(conversation_ids.len());
    for conversation_id in conversation_ids {
        out.push(get_conversation(db, conversation_id)?);
    }
    Ok(out)
}

pub fn touch_last_opened(db: &mut Database, project_id: &str) -> DbResult<Project> {
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE projects SET last_opened_at = ?1, updated_at = ?2 WHERE id = ?3",
        params![now, now, project_id],
    )?;
    if n == 0 {
        return Err(DbError::NotFound(format!("project {project_id}")));
    }
    get_project(db, project_id)
}

pub fn rename_conversation(
    db: &mut Database,
    conversation_id: &str,
    title: &str,
) -> DbResult<Conversation> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(DbError::Invalid("Conversation title is required".into()));
    }
    let conv = get_conversation(db, conversation_id)?;
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![trimmed, now, conversation_id],
    )?;
    if let Some(project_id) = conv.project_id.as_deref() {
        reindex_conversation_messages(db, conversation_id, project_id)?;
    }
    get_conversation(db, conversation_id)
}

pub fn duplicate_conversation(
    db: &mut Database,
    conversation_id: &str,
    title: Option<&str>,
) -> DbResult<Conversation> {
    let source = get_conversation(db, conversation_id)?;
    let messages = get_messages(db, conversation_id)?;
    let new_title = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{} (copy)", source.title));
    let new_id = format!("conv-{}", Uuid::new_v4());
    let now = now_rfc3339();

    db.with_transaction(|conn| {
        conn.execute(
            "INSERT INTO conversations (id, workspace_id, title, project_id, pinned, archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, 0, ?5, ?6)",
            params![
                new_id,
                source.workspace_id,
                new_title,
                source.project_id,
                now,
                now,
            ],
        )?;
        for message in &messages {
            let new_message_id = format!("msg-{}", Uuid::new_v4());
            let meta_str = message.metadata.as_ref().map(|m| m.to_string());
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, metadata, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    new_message_id,
                    new_id,
                    message.role,
                    message.content,
                    meta_str,
                    message.created_at,
                ],
            )?;
        }
        Ok(())
    })?;

    if let Some(project_id) = source.project_id.as_deref() {
        reindex_conversation_messages(db, &new_id, project_id)?;
    }
    get_conversation(db, &new_id)
}

pub fn set_project_wallpaper(
    db: &mut Database,
    project_id: &str,
    wallpaper_json: Option<&str>,
) -> DbResult<Project> {
    let _ = get_project(db, project_id)?;
    if let Some(raw) = wallpaper_json {
        crate::wallpapers::validate_wallpaper_config(raw).map_err(|e| DbError::Invalid(e))?;
    }
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE projects SET wallpaper_json = ?1, updated_at = ?2 WHERE id = ?3",
        params![wallpaper_json, now, project_id],
    )?;
    get_project(db, project_id)
}

fn reindex_conversation_messages(
    db: &Database,
    conversation_id: &str,
    project_id: &str,
) -> DbResult<()> {
    remove_conversation_from_index(db, conversation_id)?;
    let conv = get_conversation(db, conversation_id)?;
    let messages = get_messages(db, conversation_id)?;
    for message in messages {
        index_message(db, &message, &conv.title, Some(project_id))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, insert_message, Database, DEFAULT_WORKSPACE_ID};
    use tempfile::tempdir;

    fn sample_input(name: &str) -> CreateProjectInput {
        CreateProjectInput {
            name: name.into(),
            description: Some("desc".into()),
            icon_key: Some("folder".into()),
            instructions: None,
            pinned: None,
        }
    }

    #[test]
    fn migration_creates_projects_table() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("mig.db")).unwrap();
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'projects'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn project_crud_roundtrip() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("crud.db")).unwrap();
        let created = create_project(&mut db, &sample_input("Alpha")).unwrap();
        assert_eq!(created.name, "Alpha");

        let listed = list_projects(&db, false).unwrap();
        assert_eq!(listed.len(), 1);

        let updated = update_project(
            &mut db,
            &created.id,
            &UpdateProjectInput {
                name: Some("Beta".into()),
                description: None,
                icon_key: None,
                instructions: None,
                pinned: Some(true),
                archived: None,
            },
        )
        .unwrap();
        assert_eq!(updated.name, "Beta");
        assert!(updated.pinned);

        let archived = archive_project(&mut db, &created.id).unwrap();
        assert!(archived.archived);
        assert!(list_projects(&db, false).unwrap().is_empty());
        assert_eq!(list_projects(&db, true).unwrap().len(), 1);
    }

    #[test]
    fn delete_keep_chats_unassigns_conversations() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("keep.db")).unwrap();
        let project = create_project(&mut db, &sample_input("Keep")).unwrap();
        let conv = create_conversation(
            &mut db,
            DEFAULT_WORKSPACE_ID,
            "Chat",
            Some(&project.id),
        )
        .unwrap();
        insert_message(&mut db, &conv.id, "user", "keep me", None).unwrap();
        delete_project(&mut db, &project.id, DeleteProjectMode::KeepChats).unwrap();
        let reloaded = get_conversation(&db, &conv.id).unwrap();
        assert!(reloaded.project_id.is_none());
        let fts: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM message_fts WHERE project_id = ?1",
                [&project.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts, 0);
    }

    #[test]
    fn delete_project_mode_defaults_to_keep_chats() {
        assert_eq!(
            DeleteProjectMode::default(),
            DeleteProjectMode::KeepChats
        );
        let parsed: DeleteProjectMode =
            serde_json::from_value(serde_json::json!("keepChats")).unwrap();
        assert_eq!(parsed, DeleteProjectMode::KeepChats);
    }

    #[test]
    fn delete_chats_removes_conversations() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("del.db")).unwrap();
        let project = create_project(&mut db, &sample_input("Gone")).unwrap();
        let conv = create_conversation(
            &mut db,
            DEFAULT_WORKSPACE_ID,
            "Chat",
            Some(&project.id),
        )
        .unwrap();
        insert_message(&mut db, &conv.id, "user", "hello", None).unwrap();
        delete_project(&mut db, &project.id, DeleteProjectMode::DeleteChats).unwrap();
        assert!(get_conversation(&db, &conv.id).is_err());
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
