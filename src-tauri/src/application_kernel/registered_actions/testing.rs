//! Shared test fixtures for the registered action runtime.

use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
use crate::application_kernel::permissions::grant_permission;
use crate::db::{now_rfc3339, Database};

use super::context::{ActionRunContext, Presence, Venue};

/// A migrated database in a temp dir. Keep the `TempDir` alive for the test.
pub fn test_db() -> (Database, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = Database::open_path(&dir.path().join("registered-actions.db")).expect("open db");
    (db, dir)
}

/// Register an application that declares `actions` and holds the matching
/// permissions, so gateway tests start from a realistic authorized baseline.
pub fn seed_application(db: &mut Database, application_id: &str, actions: &[&str]) {
    let manifest = ApplicationManifest {
        schema_version: "1".into(),
        application_id: application_id.into(),
        instance_id: format!("instance-{application_id}"),
        name: "Test Application".into(),
        description: String::new(),
        version: 1,
        surfaces: vec![],
        routes: vec![],
        data_models: vec![],
        settings: vec![],
        capabilities: vec!["coreside.core".into()],
        permissions: permissions_for(actions),
        events: vec![],
        tests: vec![],
        search_keywords: vec![],
        tags: vec![],
        agent_description: String::new(),
        project_id: None,
        conversation_id: None,
        organization_id: None,
        ownership: None,
        application_action_access: actions.iter().map(|a| a.to_string()).collect(),
        surface_action_access: Default::default(),
        component_action_access: Default::default(),
    };
    upsert_manifest(db, manifest).expect("manifest");
    for permission in permissions_for(actions) {
        grant_permission(
            db,
            application_id,
            &permission,
            json!({ "scope": "application" }),
            "user",
        )
        .expect("permission");
    }
}

fn permissions_for(actions: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = actions
        .iter()
        .filter_map(|name| super::descriptor::find_action(name))
        .map(|d| d.permission_category.clone())
        .collect();
    out.sort();
    out.dedup();
    out
}

pub fn app_ctx(application_id: &str) -> ActionRunContext {
    ActionRunContext {
        actor: "user".into(),
        venue: Venue::Application,
        presence: Presence::Present,
        application_id: Some(application_id.into()),
        project_id: None,
        conversation_id: None,
        session_id: "session-test".into(),
        run_id: format!("run-{}", Uuid::new_v4()),
        trigger: None,
        surface_id: None,
        component_id: None,
        depth: 0,
    }
}

/// Insert an audit row directly so breaker tests can build history cheaply.
pub fn insert_audit_row(
    db: &mut Database,
    application_id: &str,
    action_name: &str,
    outcome: &str,
    risk: &str,
    run_id: &str,
) {
    db.conn()
        .execute(
            "INSERT INTO runtime_audit_events (
                id, kind, actor, venue, presence, application_id, run_id,
                action_name, outcome, risk, created_at
             ) VALUES (?1,'action','user','application','present',?2,?3,?4,?5,?6,?7)",
            rusqlite::params![
                format!("audit-{}", Uuid::new_v4()),
                application_id,
                run_id,
                action_name,
                outcome,
                risk,
                now_rfc3339()
            ],
        )
        .expect("insert audit row");
}
