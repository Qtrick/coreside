//! SQLite database access for Coreside.

mod repositories;

pub use repositories::*;

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use thiserror::Error;

const MIGRATION_001: &str = include_str!("../../migrations/001_initial.sql");

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid data: {0}")]
    Invalid(String),
}

pub type DbResult<T> = Result<T, DbError>;

pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    /// Open DB at the default app data path and run migrations.
    pub fn open_default() -> DbResult<Self> {
        let path = default_db_path()?;
        Self::open_path(&path)
    }

    /// Open DB at an explicit path (tests / overrides).
    pub fn open_path(path: &Path) -> DbResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DbError::Invalid(format!("Failed to create DB directory: {e}"))
            })?;
        }

        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let db = Self {
            conn,
            path: path.to_path_buf(),
        };
        tracing::debug!(db_path = %db.path().display(), "opened Coreside database");
        let mut db = db;
        db.migrate()?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn migrate(&mut self) -> DbResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )?;

        let applied: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM _migrations WHERE name = ?1",
                ["001_initial"],
                |row| row.get(0),
            )
            .optional()?;

        if applied.is_none() {
            self.conn.execute_batch(MIGRATION_001)?;
            self.conn.execute(
                "INSERT INTO _migrations (name) VALUES (?1)",
                ["001_initial"],
            )?;
        }

        Ok(())
    }

    pub fn with_transaction<T, F>(&mut self, f: F) -> DbResult<T>
    where
        F: FnOnce(&Connection) -> DbResult<T>,
    {
        let tx = self.conn.unchecked_transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}

fn default_db_path() -> DbResult<PathBuf> {
    // Explicit override for tests / constrained environments.
    if let Ok(override_path) = std::env::var("CORESIDE_DB_PATH") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    // Prefer the platform application data directory.
    if let Some(base) = dirs::data_dir() {
        let candidate = base.join("coreside").join("coreside.db");
        if let Some(parent) = candidate.parent() {
            if std::fs::create_dir_all(parent).is_ok() {
                return Ok(candidate);
            }
        }
    }

    // Fall back to a project-local data directory so the app still launches
    // when the system data dir is unavailable.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest_dir.parent() {
        let local = root.join(".coreside").join("coreside.db");
        if let Some(parent) = local.parent() {
            if std::fs::create_dir_all(parent).is_ok() {
                tracing::warn!(
                    path = %local.display(),
                    "using project-local database path"
                );
                return Ok(local);
            }
        }
    }

    Err(DbError::Invalid(
        "Could not create a writable directory for the Coreside database".into(),
    ))
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub const DEFAULT_WORKSPACE_ID: &str = "ws-personal-default";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migrates_and_seeds_workspace() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let db = Database::open_path(&path).unwrap();
        let name: String = db
            .conn()
            .query_row(
                "SELECT name FROM workspaces WHERE id = ?1",
                [DEFAULT_WORKSPACE_ID],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "Personal");
    }

    #[test]
    fn conversation_message_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let mut db = Database::open_path(&path).unwrap();

        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Hello").unwrap();
        let msg = insert_message(
            &mut db,
            &conv.id,
            "user",
            "hi there",
            None,
        )
        .unwrap();

        let messages = get_messages(&db, &conv.id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, msg.id);
        assert_eq!(messages[0].content, "hi there");
    }

    #[test]
    fn tool_apply_and_undo() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let mut db = Database::open_path(&path).unwrap();

        let def = crate::ai::ToolDefinition {
            id: "tool-1".into(),
            name: "Counter".into(),
            description: "A counter".into(),
            layout: serde_json::json!({ "type": "single-column" }),
            components: vec![],
        };

        let tool = apply_tool_change(
            &mut db,
            DEFAULT_WORKSPACE_ID,
            &def,
            "create",
            None,
            "initial",
        )
        .unwrap();
        assert_eq!(tool.current_version, 1);

        let def2 = crate::ai::ToolDefinition {
            id: tool.id.clone(),
            name: "Counter v2".into(),
            description: "Updated".into(),
            layout: serde_json::json!({ "type": "single-column" }),
            components: vec![],
        };
        let tool2 = apply_tool_change(
            &mut db,
            DEFAULT_WORKSPACE_ID,
            &def2,
            "update",
            Some(&tool.id),
            "rename",
        )
        .unwrap();
        assert_eq!(tool2.current_version, 2);
        assert_eq!(tool2.name, "Counter v2");

        let undone = undo_tool_change(&mut db, &tool.id).unwrap();
        assert_eq!(undone.current_version, 1);
        assert_eq!(undone.name, "Counter");
    }

    #[test]
    fn settings_roundtrip() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("t.db")).unwrap();
        set_setting(&mut db, "theme", "dark").unwrap();
        let all = get_settings(&db).unwrap();
        assert_eq!(all.get("theme").map(String::as_str), Some("dark"));
    }

    #[test]
    fn renames_default_conversation_title() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("t.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "New chat").unwrap();
        let renamed =
            maybe_rename_conversation_from_message(&mut db, &conv.id, "Track my water").unwrap();
        assert_eq!(renamed.as_deref(), Some("Track my water"));
        let again =
            maybe_rename_conversation_from_message(&mut db, &conv.id, "Something else").unwrap();
        assert!(again.is_none());
        let loaded = get_conversation(&db, &conv.id).unwrap();
        assert_eq!(loaded.title, "Track my water");
    }
}
