//! Fixture-based migration upgrade tests.
//!
//! Each test freezes a temporary database at an older schema, seeds anonymized
//! user data that could exist at that version, then reopens through the normal
//! `Database::open_path` path so the remaining migrations run exactly as they
//! would for a real user upgrade.

#[cfg(test)]
mod tests {
    use crate::db::{Database, DEFAULT_WORKSPACE_ID, LATEST_MIGRATION};
    use rusqlite::params;
    use tempfile::tempdir;

    fn assert_fk_ok(db: &Database) {
        let violations: Vec<(String, i64, String, i64)> = {
            let mut stmt = db
                .conn()
                .prepare("PRAGMA foreign_key_check")
                .expect("fk check prepare");
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .expect("fk check query")
            .collect::<Result<_, _>>()
            .expect("fk check rows")
        };
        assert!(
            violations.is_empty(),
            "foreign key violations after upgrade: {violations:?}"
        );
    }

    fn assert_latest(db: &Database) {
        let applied = db.applied_migrations().unwrap();
        assert_eq!(
            applied.last().map(String::as_str),
            Some(LATEST_MIGRATION),
            "expected latest migration {LATEST_MIGRATION}, got {applied:?}"
        );
        assert_eq!(
            applied.len(),
            crate::db::MIGRATIONS.len(),
            "expected {} migrations, got {applied:?}",
            crate::db::MIGRATIONS.len()
        );
        // Re-applying must be a no-op.
        let again = Database::open_path(db.path()).unwrap();
        assert_eq!(again.applied_migrations().unwrap(), applied);
    }

    fn table_exists(db: &Database, name: &str) -> bool {
        let n: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .unwrap();
        n == 1
    }

    fn count(db: &Database, sql: &str) -> i64 {
        db.conn().query_row(sql, [], |r| r.get(0)).unwrap()
    }

    /// Seed the common user-facing rows that existed by migration 006.
    fn seed_core_user_data(db: &Database) {
        db.conn()
            .execute(
                "INSERT INTO conversations (id, workspace_id, title, project_id)
                 VALUES ('conv-biology', ?1, 'Cell structure notes', NULL)",
                [DEFAULT_WORKSPACE_ID],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO messages (id, conversation_id, role, content)
                 VALUES ('msg-1', 'conv-biology', 'user', 'Explain mitosis briefly')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO messages (id, conversation_id, role, content)
                 VALUES ('msg-2', 'conv-biology', 'assistant', 'Mitosis is cell division.')",
                [],
            )
            .unwrap();
        let definition = r#"{"id":"tool-counter","name":"Study Counter","description":"Tracks study sessions","layout":{"type":"single-column"},"components":[]}"#;
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version)
                 VALUES ('tool-counter', ?1, 'Study Counter', 'Tracks study sessions', 'stack', ?2, 1)",
                params![DEFAULT_WORKSPACE_ID, definition],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO tool_state (tool_id, state_json)
                 VALUES ('tool-counter', ?1)",
                params![r#"{"count":3}"#],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO projects (id, name, description, instructions)
                 VALUES ('proj-bio', 'Biology', 'Study project', 'Prefer concise answers')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "UPDATE conversations SET project_id = 'proj-bio' WHERE id = 'conv-biology'",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('theme', 'dark')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [],
            )
            .unwrap();
    }

    fn seed_automation(db: &Database) {
        db.conn()
            .execute(
                "INSERT INTO automations (
                    id, workspace_id, name, enabled, trigger_type, trigger_json,
                    action_type, action_json, next_run_at
                 ) VALUES (
                    'auto-rotate', ?1, 'Rotate wallpaper', 1, 'daily',
                    ?2, 'wallpaper.rotate', '{}',
                    '2026-08-02T13:00:00Z'
                 )",
                params![DEFAULT_WORKSPACE_ID, r#"{"time":"09:00"}"#],
            )
            .unwrap();
    }

    fn seed_manifest_corpus(db: &Database) {
        let manifest = r#"{"schemaVersion":"1","applicationId":"app-counter","instanceId":"inst-counter","name":"Study Counter","description":"","version":1,"surfaces":[],"routes":[],"dataModels":[],"settings":[],"capabilities":["coreside.core"],"permissions":["local_data.read"],"events":[],"tests":[],"searchKeywords":[],"tags":[],"agentDescription":"","applicationActionAccess":["local_data.query"],"surfaceActionAccess":{},"componentActionAccess":{}}"#;
        db.conn()
            .execute(
                "INSERT INTO application_manifests (
                    id, application_id, instance_id, schema_version, current_version,
                    last_known_good_version, manifest_json, health_state, lifecycle_state,
                    disabled, crash_count, conversation_id
                 ) VALUES (
                    'man-1', 'app-counter', 'inst-counter', '1', 1, 1, ?1,
                    'healthy', 'active', 0, 0, 'conv-biology'
                 )",
                params![manifest],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO application_manifests (
                    id, application_id, instance_id, schema_version, current_version,
                    last_known_good_version, manifest_json, health_state, lifecycle_state,
                    disabled, crash_count
                 ) VALUES (
                    'man-failed', 'app-failed', 'inst-failed', '1', 2, 1, ?1,
                    'failed', 'suspended', 0, 3
                 )",
                params![manifest
                    .replace("app-counter", "app-failed")
                    .replace("inst-counter", "inst-failed")],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO application_manifest_versions (
                    id, application_id, version, manifest_json, is_known_good,
                    validation_status, test_status
                 ) VALUES (
                    'manver-1', 'app-counter', 1, ?1, 1, 'passed', 'passed'
                 )",
                params![manifest],
            )
            .unwrap();
    }

    fn seed_approvals_and_grants(db: &Database) {
        db.conn()
            .execute(
                "INSERT INTO runtime_approvals (
                    id, application_id, action_name, action_title, risk, critical,
                    input_preview, input_json, call_hash, descriptor_hash,
                    venue, presence, status, expires_at
                 ) VALUES (
                    'appr-1', 'app-counter', 'local_data.write', 'Write note', 'write', 0,
                    '{}', '{\"text\":\"hi\"}', 'call-hash-1', 'desc-hash-1',
                    'application', 'present', 'pending', '2099-01-01T00:00:00Z'
                 )",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO runtime_action_grants (
                    id, application_id, action_name, descriptor_hash, scope_kind,
                    scope_json, duration, status
                 ) VALUES (
                    'grant-1', 'app-counter', 'local_data.query', 'desc-hash-1',
                    'application', '{}', 'always', 'active'
                 )",
                [],
            )
            .unwrap();
    }

    #[test]
    fn fresh_database_reaches_latest_migration() {
        let dir = tempdir().unwrap();
        let db = Database::open_path(&dir.path().join("fresh.db")).unwrap();
        assert_latest(&db);
        assert_fk_ok(&db);
        assert!(table_exists(&db, "runtime_approvals"));
        assert!(table_exists(&db, "application_manifests"));
        assert!(table_exists(&db, "projects"));
    }

    #[test]
    fn upgrade_from_006_preserves_projects_and_messages() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("from006.db");
        {
            let db = Database::open_path_through(&path, "006_projects").unwrap();
            seed_core_user_data(&db);
            assert_eq!(db.applied_migrations().unwrap().len(), 6);
            assert!(!table_exists(&db, "runtime_approvals"));
        }
        let db = Database::open_path(&path).unwrap();
        assert_latest(&db);
        assert_fk_ok(&db);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM conversations"), 1);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM messages"), 2);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM projects"), 1);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM tools"), 1);
        let title: String = db
            .conn()
            .query_row(
                "SELECT title FROM conversations WHERE id = 'conv-biology'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "Cell structure notes");
        let state: String = db
            .conn()
            .query_row(
                "SELECT state_json FROM tool_state WHERE tool_id = 'tool-counter'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(state.contains("\"count\":3"));
        let project: String = db
            .conn()
            .query_row(
                "SELECT project_id FROM conversations WHERE id = 'conv-biology'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(project, "proj-bio");
    }

    #[test]
    fn upgrade_from_011_preserves_automations() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("from011.db");
        {
            let db = Database::open_path_through(&path, "011_action_log_mode").unwrap();
            seed_core_user_data(&db);
            seed_automation(&db);
            assert_eq!(db.applied_migrations().unwrap().len(), 11);
            assert!(!table_exists(&db, "surfaces"));
        }
        let db = Database::open_path(&path).unwrap();
        assert_latest(&db);
        assert_fk_ok(&db);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM automations"), 1);
        let name: String = db
            .conn()
            .query_row(
                "SELECT name FROM automations WHERE id = 'auto-rotate'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "Rotate wallpaper");
    }

    #[test]
    fn upgrade_from_012_preserves_runtime_v2_tables() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("from012.db");
        {
            let db = Database::open_path_through(&path, "012_runtime_v2").unwrap();
            seed_core_user_data(&db);
            assert!(table_exists(&db, "surfaces"));
            assert!(!table_exists(&db, "application_manifests"));
        }
        let db = Database::open_path(&path).unwrap();
        assert_latest(&db);
        assert_fk_ok(&db);
        assert!(table_exists(&db, "application_manifests"));
        assert!(table_exists(&db, "runtime_approvals"));
        assert_eq!(count(&db, "SELECT COUNT(*) FROM messages"), 2);
    }

    #[test]
    fn upgrade_from_013_preserves_failed_and_healthy_manifests() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("from013.db");
        {
            let db = Database::open_path_through(&path, "013_application_kernel").unwrap();
            seed_core_user_data(&db);
            seed_manifest_corpus(&db);
            assert!(!table_exists(&db, "runtime_approvals"));
            assert_eq!(
                count(
                    &db,
                    "SELECT COUNT(*) FROM application_manifests WHERE crash_count >= 3"
                ),
                1
            );
        }
        let db = Database::open_path(&path).unwrap();
        assert_latest(&db);
        assert_fk_ok(&db);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM application_manifests"), 2);
        let crashed: i64 = db
            .conn()
            .query_row(
                "SELECT crash_count FROM application_manifests WHERE application_id = 'app-failed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(crashed, 3);
        let healthy: String = db
            .conn()
            .query_row(
                "SELECT health_state FROM application_manifests WHERE application_id = 'app-counter'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(healthy, "healthy");
    }

    #[test]
    fn upgrade_from_014_adds_registered_action_tables() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("from014.db");
        {
            let db = Database::open_path_through(&path, "014_continuity_scheduler").unwrap();
            seed_core_user_data(&db);
            seed_manifest_corpus(&db);
            assert!(!table_exists(&db, "runtime_approvals"));
        }
        let db = Database::open_path(&path).unwrap();
        assert_latest(&db);
        assert_fk_ok(&db);
        assert!(table_exists(&db, "runtime_approvals"));
        assert!(table_exists(&db, "runtime_action_grants"));
        assert!(table_exists(&db, "runtime_audit_events"));
        assert!(table_exists(&db, "application_build_failures"));
        assert_eq!(count(&db, "SELECT COUNT(*) FROM application_manifests"), 2);
    }

    #[test]
    fn upgrade_from_015_is_idempotent_with_approvals_and_grants() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("from015.db");
        {
            let db = Database::open_path_through(&path, "015_registered_actions").unwrap();
            seed_core_user_data(&db);
            seed_automation(&db);
            seed_manifest_corpus(&db);
            seed_approvals_and_grants(&db);
        }
        let db = Database::open_path(&path).unwrap();
        assert_latest(&db);
        assert_fk_ok(&db);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM runtime_approvals"), 1);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM runtime_action_grants"), 1);
        let status: String = db
            .conn()
            .query_row(
                "SELECT status FROM runtime_approvals WHERE id = 'appr-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "pending");
    }

    #[test]
    fn realistic_corpus_survives_full_upgrade_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("corpus.db");
        // Simulate a user who upgraded progressively: freeze at 011 with data,
        // then jump to current. This is the common support case.
        {
            let db = Database::open_path_through(&path, "011_action_log_mode").unwrap();
            seed_core_user_data(&db);
            seed_automation(&db);
            db.conn()
                .execute(
                    "INSERT INTO provider_connections (
                        id, provider, label, keyring_account, is_active, created_at, updated_at
                     ) VALUES (
                        'conn-1', 'openai', 'Personal', 'coreside:conn-1', 1,
                        datetime('now'), datetime('now')
                     )",
                    [],
                )
                .unwrap();
        }
        let db = Database::open_path(&path).unwrap();
        assert_latest(&db);
        assert_fk_ok(&db);

        // Repository reads still work after the jump.
        let conv = crate::db::get_conversation(&db, "conv-biology").unwrap();
        assert_eq!(conv.title, "Cell structure notes");
        let messages = crate::db::get_messages(&db, "conv-biology").unwrap();
        assert_eq!(messages.len(), 2);
        let tools = crate::db::list_tools(&db, Some(DEFAULT_WORKSPACE_ID)).unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "Study Counter");

        // Provider metadata survives; secrets never lived in SQLite.
        let cols: Vec<String> = {
            let mut stmt = db
                .conn()
                .prepare("PRAGMA table_info(provider_connections)")
                .unwrap();
            stmt.query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert!(!cols.iter().any(|c| c == "api_key"));
        let account: String = db
            .conn()
            .query_row(
                "SELECT keyring_account FROM provider_connections WHERE id = 'conn-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(account, "coreside:conn-1");
    }

    #[test]
    fn unknown_migration_stop_point_is_rejected() {
        let dir = tempdir().unwrap();
        let err = match Database::open_path_through(&dir.path().join("bad.db"), "999_future") {
            Err(e) => e,
            Ok(_) => panic!("expected unknown migration stop point to fail"),
        };
        assert!(err.to_string().contains("unknown migration stop point"));
    }

    #[test]
    fn migration_list_is_contiguous_and_matches_files() {
        // Guard against the class of drift where docs say 15 but a SQL file is
        // missing from MIGRATIONS, or numbering skips.
        let names = crate::db::MIGRATIONS
            .iter()
            .map(|(n, _)| *n)
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 20);
        for (i, name) in names.iter().enumerate() {
            let expected = format!("{:03}_", i + 1);
            assert!(
                name.starts_with(&expected),
                "migration {i} should start with {expected}, got {name}"
            );
        }
        assert_eq!(names[19], LATEST_MIGRATION);
    }
}
