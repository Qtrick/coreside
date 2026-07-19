//! SQLite database access for Coreside.

mod repositories;

pub use repositories::*;

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use thiserror::Error;

const MIGRATION_001: &str = include_str!("../../migrations/001_initial.sql");
const MIGRATION_002: &str = include_str!("../../migrations/002_added_settings.sql");
const MIGRATION_003: &str = include_str!("../../migrations/003_provider_connections.sql");
const MIGRATION_004: &str = include_str!("../../migrations/004_action_log.sql");
const MIGRATION_005: &str = include_str!("../../migrations/005_automations.sql");
const MIGRATION_006: &str = include_str!("../../migrations/006_projects.sql");
const MIGRATION_007: &str = include_str!("../../migrations/007_media_search.sql");
const MIGRATION_008: &str = include_str!("../../migrations/008_media_thumbnails.sql");
const MIGRATION_009: &str = include_str!("../../migrations/009_crawler.sql");
const MIGRATION_010: &str = include_str!("../../migrations/010_exa_wallpapers.sql");
const MIGRATION_011: &str = include_str!("../../migrations/011_action_log_mode.sql");
const MIGRATION_012: &str = include_str!("../../migrations/012_runtime_v2.sql");
const MIGRATION_013: &str = include_str!("../../migrations/013_application_kernel.sql");
const MIGRATION_014: &str = include_str!("../../migrations/014_continuity_scheduler.sql");

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

        self.apply_migration("001_initial", MIGRATION_001)?;
        self.apply_migration("002_added_settings", MIGRATION_002)?;
        self.apply_migration("003_provider_connections", MIGRATION_003)?;
        self.apply_migration("004_action_log", MIGRATION_004)?;
        self.apply_migration("005_automations", MIGRATION_005)?;
        self.apply_migration("006_projects", MIGRATION_006)?;
        self.apply_migration("007_media_search", MIGRATION_007)?;
        self.apply_migration("008_media_thumbnails", MIGRATION_008)?;
        self.apply_migration("009_crawler", MIGRATION_009)?;
        self.apply_migration("010_exa_wallpapers", MIGRATION_010)?;
        self.apply_migration("011_action_log_mode", MIGRATION_011)?;
        self.apply_migration("012_runtime_v2", MIGRATION_012)?;
        self.apply_migration("013_application_kernel", MIGRATION_013)?;
        self.apply_migration("014_continuity_scheduler", MIGRATION_014)?;

        Ok(())
    }

    fn apply_migration(&self, name: &str, sql: &str) -> DbResult<()> {
        let applied: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM _migrations WHERE name = ?1",
                [name],
                |row| row.get(0),
            )
            .optional()?;

        if applied.is_none() {
            // Apply DDL + migration bookkeeping atomically so a mid-script
            // failure cannot leave partial schema (e.g. half-added columns)
            // without a recorded migration version.
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.execute("INSERT INTO _migrations (name) VALUES (?1)", [name])?;
            tx.commit()?;
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

        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Hello", None).unwrap();
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
    fn added_settings_roundtrip_and_rejects_core() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("t.db")).unwrap();

        let bad = crate::settings::UpsertAddedSettingInput {
            id: "core.settings.appearance".into(),
            owner_tool_id: None,
            label: "Nope".into(),
            description: None,
            setting_type: "string".into(),
            default_value: None,
            current_value: None,
            constraints: None,
        };
        assert!(upsert_added_setting(&mut db, &bad).is_err());

        let ok = crate::settings::UpsertAddedSettingInput {
            id: "tool.water.units".into(),
            owner_tool_id: None,
            label: "Units".into(),
            description: Some("ml or oz".into()),
            setting_type: "string".into(),
            default_value: Some(serde_json::json!("ml")),
            current_value: Some(serde_json::json!("oz")),
            constraints: Some(serde_json::json!({"enum": ["ml", "oz"]})),
        };
        let saved = upsert_added_setting(&mut db, &ok).unwrap();
        assert_eq!(saved.id, "tool.water.units");
        assert_eq!(saved.version, 1);

        let listed = list_added_settings(&db, None).unwrap();
        assert_eq!(listed.len(), 1);
        delete_added_setting(&mut db, "tool.water.units").unwrap();
        assert!(list_added_settings(&db, None).unwrap().is_empty());
    }

    #[test]
    fn renames_default_conversation_title() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("t.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "New chat", None).unwrap();
        let renamed =
            maybe_rename_conversation_from_message(&mut db, &conv.id, "Track my water").unwrap();
        assert_eq!(renamed.as_deref(), Some("Track my water"));
        let again =
            maybe_rename_conversation_from_message(&mut db, &conv.id, "Something else").unwrap();
        assert!(again.is_none());
        let loaded = get_conversation(&db, &conv.id).unwrap();
        assert_eq!(loaded.title, "Track my water");
    }

    #[test]
    fn provider_connections_schema_has_no_api_key_column() {
        let migration = include_str!("../../migrations/003_provider_connections.sql").to_lowercase();
        assert!(
            !migration.contains("api_key"),
            "migration must not define an api_key column"
        );
        assert!(migration.contains("keyring_account"));

        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("provider.db")).unwrap();
        let mut stmt = db
            .conn()
            .prepare("PRAGMA table_info(provider_connections)")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            !cols.iter().any(|c| c.eq_ignore_ascii_case("api_key")),
            "provider_connections must not have api_key; got {cols:?}"
        );
        assert!(cols.iter().any(|c| c == "keyring_account"));
        assert!(cols.iter().any(|c| c == "is_active"));
    }

    #[test]
    fn provider_connection_roundtrip_stores_keyring_account_only() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("pc.db")).unwrap();
        let now = "2026-01-01T00:00:00Z".to_string();
        let row = ProviderConnection {
            id: "conn-1".into(),
            provider: "openai".into(),
            label: "Work".into(),
            base_url: None,
            model_default: Some("gpt-4.1-mini".into()),
            keyring_account: "coreside:conn-1".into(),
            is_active: false,
            last_status: None,
            last_tested_at: None,
            created_at: now.clone(),
            updated_at: now,
        };
        let saved = upsert_provider_connection(&mut db, &row).unwrap();
        assert_eq!(saved.keyring_account, "coreside:conn-1");
        let active = set_active_provider_connection(&mut db, "conn-1").unwrap();
        assert!(active.is_active);
        let fetched = get_active_provider_connection(&db).unwrap().unwrap();
        assert_eq!(fetched.id, "conn-1");
    }

    #[test]
    fn crawler_migration_seeds_resource_profile() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("crawler.db")).unwrap();
        let profile: String = db
            .conn()
            .query_row(
                "SELECT value FROM crawler_settings WHERE key = 'webResearchResourceProfile'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(profile, "balanced");
        let tables: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (
                    'crawler_runtime','crawler_settings','crawl_sources','crawl_jobs',
                    'crawl_job_sources','domain_crawl_state','crawler_cache_metadata'
                 )",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 7);
    }

    #[test]
    fn exa_migration_seeds_profile_and_ledger() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("exa.db")).unwrap();
        let profile: String = db
            .conn()
            .query_row(
                "SELECT value FROM settings WHERE key = 'searchProfile'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(profile, "saver");
        let soft: String = db
            .conn()
            .query_row(
                "SELECT value FROM settings WHERE key = 'exaBudgetSoftPercent'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(soft, "75");
        let tables: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (
                    'exa_usage_ledger','wallpaper_templates'
                 )",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 2);
    }
}
