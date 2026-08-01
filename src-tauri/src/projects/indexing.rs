//! FTS indexing for project-scoped message search.

use rusqlite::params;

use crate::db::{get_conversation, get_messages, Database, DbError, DbResult, Message};

fn snippet_bound(content: &str, max_chars: usize) -> String {
    let collapsed: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        let truncated: String = collapsed
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect();
        format!("{truncated}…")
    }
}

/// Index a single message into `message_fts`.
pub fn index_message(
    db: &Database,
    message: &Message,
    conversation_title: &str,
    project_id: Option<&str>,
) -> DbResult<()> {
    db.conn().execute(
        "DELETE FROM message_fts WHERE message_id = ?1",
        [&message.id],
    )?;
    db.conn().execute(
        "INSERT INTO message_fts (message_id, conversation_id, project_id, role, content, title)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            message.id,
            message.conversation_id,
            project_id,
            message.role,
            snippet_bound(&message.content, 8000),
            conversation_title,
        ],
    )?;
    Ok(())
}

/// Index a message by loading conversation metadata from the database.
pub fn index_message_for_conversation(db: &Database, message: &Message) -> DbResult<()> {
    let conv = get_conversation(db, &message.conversation_id)?;
    index_message(db, message, &conv.title, conv.project_id.as_deref())
}

/// Remove all FTS rows for a conversation.
pub fn remove_conversation_from_index(db: &Database, conversation_id: &str) -> DbResult<()> {
    db.conn().execute(
        "DELETE FROM message_fts WHERE conversation_id = ?1",
        [conversation_id],
    )?;
    Ok(())
}

/// Rebuild the FTS index for every message in a project.
pub fn rebuild_project_index(db: &Database, project_id: &str) -> DbResult<u64> {
    if project_id.trim().is_empty() {
        return Err(DbError::Invalid("project_id is required".into()));
    }
    db.conn().execute(
        "DELETE FROM message_fts WHERE project_id = ?1",
        [project_id],
    )?;

    let mut stmt = db
        .conn()
        .prepare("SELECT id FROM conversations WHERE project_id = ?1 ORDER BY updated_at DESC")?;
    let conversation_ids: Vec<String> = stmt
        .query_map([project_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut indexed = 0u64;
    for conversation_id in conversation_ids {
        let conv = get_conversation(db, &conversation_id)?;
        let messages = get_messages(db, &conversation_id)?;
        for message in messages {
            index_message(db, &message, &conv.title, Some(project_id))?;
            indexed += 1;
        }
    }
    Ok(indexed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, insert_message, Database, DEFAULT_WORKSPACE_ID};
    use crate::projects::{create_project, CreateProjectInput};
    use tempfile::tempdir;

    #[test]
    fn indexes_and_isolates_by_project() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("fts.db")).unwrap();

        let p1 = create_project(
            &mut db,
            &CreateProjectInput {
                name: "Alpha".into(),
                description: None,
                icon_key: None,
                instructions: None,
                pinned: None,
            },
        )
        .unwrap();
        let p2 = create_project(
            &mut db,
            &CreateProjectInput {
                name: "Beta".into(),
                description: None,
                icon_key: None,
                instructions: None,
                pinned: None,
            },
        )
        .unwrap();

        let c1 =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat A", Some(&p1.id)).unwrap();
        let c2 =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat B", Some(&p2.id)).unwrap();

        let m1 = insert_message(&mut db, &c1.id, "user", "alpha secret keyword", None).unwrap();
        let _m2 = insert_message(&mut db, &c2.id, "user", "alpha secret keyword", None).unwrap();

        let count_p1: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM message_fts WHERE project_id = ?1 AND content MATCH 'keyword'",
                [&p1.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_p1, 1);

        let msg_id: String = db
            .conn()
            .query_row(
                "SELECT message_id FROM message_fts WHERE project_id = ?1 AND content MATCH 'keyword'",
                [&p1.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(msg_id, m1.id);

        remove_conversation_from_index(&db, &c1.id).unwrap();
        let count_after: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM message_fts WHERE project_id = ?1",
                [&p1.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_after, 0);
    }
}
