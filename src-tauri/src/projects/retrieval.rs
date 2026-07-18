//! Project-scoped full-text retrieval.

use rusqlite::params;

use crate::db::{Database, DbError, DbResult};

use super::models::ProjectContextHit;
use super::repository::get_project;

const MAX_SNIPPET_CHARS: usize = 240;

fn sanitize_fts_query(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| {
            let escaped = t.replace('"', "\"\"");
            format!("\"{escaped}\"")
        })
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

/// Search messages within a single project. Never returns other projects' content.
pub fn search_project_context(
    db: &Database,
    project_id: &str,
    query: &str,
    limit: usize,
) -> DbResult<Vec<ProjectContextHit>> {
    let project_id = project_id.trim();
    if project_id.is_empty() {
        return Err(DbError::Invalid("project_id is required".into()));
    }
    let _ = get_project(db, project_id)?;

    let fts_query = match sanitize_fts_query(query) {
        Some(q) => q,
        None => return Ok(Vec::new()),
    };

    let limit = limit.clamp(1, 50) as i64;
    // EXISTS membership check so stale/orphan FTS rows cannot leak across
    // projects after moves or failed reindexes (fail closed).
    let mut stmt = db.conn().prepare(
        "SELECT
            message_id,
            conversation_id,
            role,
            title,
            snippet(message_fts, 4, '', '', '…', 32) AS snippet,
            bm25(message_fts) AS rank
         FROM message_fts
         WHERE project_id = ?1
           AND message_fts MATCH ?2
           AND EXISTS (
             SELECT 1 FROM conversations AS c
             WHERE c.id = message_fts.conversation_id
               AND c.project_id = ?1
           )
         ORDER BY rank
         LIMIT ?3",
    )?;

    let rows = stmt.query_map(params![project_id, fts_query, limit], |row| {
        let snippet: String = row.get(4)?;
        let rank: f64 = row.get(5)?;
        Ok(ProjectContextHit {
            message_id: row.get(0)?,
            conversation_id: row.get(1)?,
            conversation_title: row.get(3)?,
            role: row.get(2)?,
            snippet: bound_snippet(&snippet),
            rank,
        })
    })?;

    let mut hits = Vec::new();
    for row in rows {
        let hit = row?;
        if hit.conversation_id.is_empty() {
            continue;
        }
        hits.push(hit);
    }
    Ok(hits)
}

fn bound_snippet(snippet: &str) -> String {
    if snippet.chars().count() <= MAX_SNIPPET_CHARS {
        snippet.to_string()
    } else {
        let truncated: String = snippet
            .chars()
            .take(MAX_SNIPPET_CHARS.saturating_sub(1))
            .collect();
        format!("{truncated}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_conversation, insert_message, Database, DEFAULT_WORKSPACE_ID};
    use crate::projects::{create_project, CreateProjectInput};
    use tempfile::tempdir;

    #[test]
    fn fts_isolation_between_projects() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("ret.db")).unwrap();

        let p1 = create_project(
            &mut db,
            &CreateProjectInput {
                name: "One".into(),
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
                name: "Two".into(),
                description: None,
                icon_key: None,
                instructions: None,
                pinned: None,
            },
        )
        .unwrap();

        let c1 = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "A", Some(&p1.id)).unwrap();
        let c2 = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "B", Some(&p2.id)).unwrap();
        insert_message(&mut db, &c1.id, "user", "unicorn alpha", None).unwrap();
        insert_message(&mut db, &c2.id, "user", "unicorn beta", None).unwrap();

        let hits = search_project_context(&db, &p1.id, "unicorn", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].conversation_id, c1.id);

        let other = search_project_context(&db, &p2.id, "unicorn", 10).unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].conversation_id, c2.id);
        assert_ne!(hits[0].message_id, other[0].message_id);
    }

    #[test]
    fn stale_fts_rows_do_not_cross_projects() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("stale.db")).unwrap();

        let p1 = create_project(
            &mut db,
            &CreateProjectInput {
                name: "One".into(),
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
                name: "Two".into(),
                description: None,
                icon_key: None,
                instructions: None,
                pinned: None,
            },
        )
        .unwrap();

        let c1 = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "A", Some(&p1.id)).unwrap();
        let msg = insert_message(&mut db, &c1.id, "user", "sharedunique token", None).unwrap();

        // Simulate a stale FTS row that still claims project p1 after the chat moved.
        db.conn()
            .execute(
                "UPDATE conversations SET project_id = ?1 WHERE id = ?2",
                rusqlite::params![p2.id, c1.id],
            )
            .unwrap();

        let stale = search_project_context(&db, &p1.id, "sharedunique", 10).unwrap();
        assert!(
            stale.is_empty(),
            "stale FTS must not return chats no longer in the project"
        );

        // After rebuild, project 2 can find it.
        crate::projects::rebuild_project_index(&db, &p2.id).unwrap();
        let fresh = search_project_context(&db, &p2.id, "sharedunique", 10).unwrap();
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].message_id, msg.id);
    }
}
