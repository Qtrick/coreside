//! Chat and project summary storage.

use rusqlite::{params, OptionalExtension};

use crate::db::{get_conversation, Database, DbError, DbResult, now_rfc3339};

use super::repository::list_project_conversations;

use super::models::ChatSummary;

pub fn get_chat_summary(db: &Database, conversation_id: &str) -> DbResult<Option<ChatSummary>> {
    let mut stmt = db.conn().prepare(
        "SELECT conversation_id, summary, updated_at FROM chat_summaries WHERE conversation_id = ?1",
    )?;
    let row = stmt
        .query_row([conversation_id], |row| {
            Ok(ChatSummary {
                conversation_id: row.get(0)?,
                summary: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })
        .optional()
        .map_err(DbError::from)?;
    Ok(row)
}

pub fn set_chat_summary(
    db: &mut Database,
    conversation_id: &str,
    summary: &str,
) -> DbResult<ChatSummary> {
    let _ = get_conversation(db, conversation_id)?;
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO chat_summaries (conversation_id, summary, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(conversation_id) DO UPDATE SET
           summary = excluded.summary,
           updated_at = excluded.updated_at",
        params![conversation_id, summary.trim(), now],
    )?;
    Ok(ChatSummary {
        conversation_id: conversation_id.to_string(),
        summary: summary.trim().to_string(),
        updated_at: now,
    })
}

pub fn get_project_summary(db: &Database, project_id: &str) -> DbResult<Option<String>> {
    super::repository::get_project(db, project_id).map(|p| p.summary)
}

pub fn set_project_summary(
    db: &mut Database,
    project_id: &str,
    summary: &str,
) -> DbResult<String> {
    let now = now_rfc3339();
    let n = db.conn().execute(
        "UPDATE projects SET summary = ?1, summary_updated_at = ?2, updated_at = ?3 WHERE id = ?4",
        params![summary.trim(), now, now, project_id],
    )?;
    if n == 0 {
        return Err(DbError::NotFound(format!("project {project_id}")));
    }
    Ok(summary.trim().to_string())
}

fn snippet(text: &str, max_chars: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        let truncated: String = collapsed.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Build a deterministic summary from conversation titles and recent snippets.
pub fn deterministic_fallback_summary(db: &Database, project_id: &str) -> DbResult<String> {
    let conversations = list_project_conversations(db, project_id)?;
    if conversations.is_empty() {
        return Ok("No chats in this project yet.".into());
    }

    let mut parts = Vec::new();
    for conv in conversations.iter().take(8) {
        if let Ok(Some(chat_summary)) = get_chat_summary(db, &conv.id) {
            parts.push(format!("{}: {}", conv.title, snippet(&chat_summary.summary, 120)));
            continue;
        }
        let recent = crate::db::get_recent_messages(db, &conv.id, 2)?;
        if recent.is_empty() {
            parts.push(format!("{}: (empty)", conv.title));
        } else {
            let snippets: Vec<String> = recent
                .iter()
                .map(|m| format!("{}: {}", m.role, snippet(&m.content, 80)))
                .collect();
            parts.push(format!("{} — {}", conv.title, snippets.join("; ")));
        }
    }
    Ok(parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, insert_message, Database, DEFAULT_WORKSPACE_ID};
    use crate::projects::{create_project, CreateProjectInput};
    use tempfile::tempdir;

    #[test]
    fn fallback_summary_uses_titles_and_messages() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("sum.db")).unwrap();
        let project = create_project(
            &mut db,
            &CreateProjectInput {
                name: "Work".into(),
                description: None,
                icon_key: None,
                instructions: None,
                pinned: None,
            },
        )
        .unwrap();
        let conv = create_conversation(
            &mut db,
            DEFAULT_WORKSPACE_ID,
            "Planning",
            Some(&project.id),
        )
        .unwrap();
        insert_message(&mut db, &conv.id, "user", "Ship the feature", None).unwrap();

        let summary = deterministic_fallback_summary(&db, &project.id).unwrap();
        assert!(summary.contains("Planning"));
        assert!(summary.contains("Ship"));
    }
}
