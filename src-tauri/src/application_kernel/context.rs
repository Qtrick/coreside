//! Token-efficient application context for the agent.

use serde_json::{json, Value};

use crate::db::{Database, DbResult};

use super::data::{get_model, query_records};
use super::manifest::get_manifest;

pub fn application_summary(db: &Database, application_id: &str) -> DbResult<Value> {
    let m = get_manifest(db, application_id)?;
    let record_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM generated_data_records WHERE application_id = ?1",
            [application_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(json!({
        "applicationId": m.application_id,
        "name": m.manifest.name,
        "purpose": m.manifest.description,
        "version": m.current_version,
        "lastKnownGood": m.last_known_good_version,
        "health": m.health_state,
        "lifecycle": m.lifecycle_state,
        "surfaces": m.manifest.surfaces.iter().map(|s| json!({
            "surfaceId": s.surface_id,
            "placement": s.placement,
        })).collect::<Vec<_>>(),
        "settings": m.manifest.settings,
        "dataModels": m.manifest.data_models,
        "capabilities": m.manifest.capabilities,
        "permissions": m.manifest.permissions,
        "recordCount": record_count,
        "hash": simple_hash(&format!("{}:{}", m.application_id, m.current_version)),
    }))
}

/// Formal bounded application context for evolution (model-visible, no secrets).
pub fn evolution_context(db: &Database, application_id: &str) -> DbResult<Value> {
    let summary = application_summary(db, application_id)?;
    // Read-only DB binding — never invent surf-*, never gate on mutation lifecycle
    // (suspended apps still need accurate surfaceId / revision for OCC guidance).
    let surface_id =
        super::lineage::lookup_bound_application_surface_id(db, application_id).unwrap_or_default();
    let (surface_revision, components, state_contracts, action_contracts) = match (!surface_id
        .is_empty())
    .then(|| crate::runtime_v2::surfaces::get_surface(db, &surface_id).ok())
    .flatten()
    {
        Some(surface) => {
            let doc = crate::runtime_v2::SoftwareDocument::from_value(&surface.definition)
                .unwrap_or_else(|_| {
                    crate::runtime_v2::SoftwareDocument::new(
                        application_id,
                        summary
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(application_id),
                    )
                });
            let comps: Vec<Value> = doc
                .flatten_components()
                .into_iter()
                .map(|c| {
                    json!({
                        "id": c.id,
                        "type": c.component_type,
                        "valueKey": c.value_key,
                    })
                })
                .collect();
            (
                surface.current_revision,
                comps,
                doc.state_contracts
                    .iter()
                    .map(|sc| serde_json::to_value(sc).unwrap_or(json!({})))
                    .collect::<Vec<_>>(),
                doc.action_contracts
                    .iter()
                    .map(|ac| serde_json::to_value(ac).unwrap_or(json!({})))
                    .collect::<Vec<_>>(),
            )
        }
        None => (0, Vec::new(), Vec::new(), Vec::new()),
    };

    Ok(json!({
        "applicationId": application_id,
        "applicationRevision": summary.get("version").cloned().unwrap_or(json!(0)),
        "surfaceId": surface_id,
        "surfaceRevision": surface_revision,
        "baseRevisionRequired": surface_revision,
        "components": components,
        "stateContracts": state_contracts,
        "actionContracts": action_contracts,
        "routes": summary.get("surfaces").cloned().unwrap_or(json!([])),
        "dataModels": summary.get("dataModels").cloned().unwrap_or(json!([])),
        "lifecycle": summary.get("lifecycle").cloned().unwrap_or(json!("unknown")),
        "recordCount": summary.get("recordCount").cloned().unwrap_or(json!(0)),
        "note": "Fresh DB snapshot for this turn. Do not reuse across turns. Evolve plans must set baseRevision to surfaceRevision.",
    }))
}

pub fn data_model_summary(db: &Database, application_id: &str, model_id: &str) -> DbResult<Value> {
    let def = get_model(db, application_id, model_id)?;
    let sample = query_records(db, application_id, model_id, 5)?;
    Ok(json!({
        "modelId": def.model_id,
        "displayName": def.display_name,
        "schemaVersion": def.schema_version,
        "fields": def.fields,
        "sampleRecords": sample,
        "note": "Full datasets are not sent by default",
    }))
}

pub fn recent_transactions_summary(
    db: &Database,
    application_id: &str,
    limit: usize,
) -> DbResult<Value> {
    // Provenance joined lightly
    let mut stmt = db.conn().prepare(
        "SELECT transaction_id, validation_status, test_status, created_at
         FROM operation_provenance
         ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |row| {
        Ok(json!({
            "transactionId": row.get::<_, String>(0)?,
            "validation": row.get::<_, Option<String>>(1)?,
            "tests": row.get::<_, Option<String>>(2)?,
            "createdAt": row.get::<_, String>(3)?,
        }))
    })?;
    let items: Vec<Value> = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "applicationId": application_id,
        "recent": items,
    }))
}

fn simple_hash(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::plan_fixtures::task_tracker_create_plan;
    use crate::application_kernel::{apply_change, decide_proposal, ChangeRequest};
    use crate::db::{create_conversation, Database, DEFAULT_WORKSPACE_ID};
    use tempfile::tempdir;

    #[test]
    fn evolution_context_includes_surface_revision_and_component_ids() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("evo-ctx.db")).unwrap();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Evolution ctx", None).unwrap();
        let plan = task_tracker_create_plan();
        let ops = crate::application_kernel::application_plan::compile_plan(&plan)
            .unwrap()
            .compiled
            .operations;
        let prop = apply_change(
            &mut db,
            None,
            ChangeRequest {
                conversation_id: Some(conv.id),
                summary: "Create".into(),
                operations: ops,
                source_type: "agent".into(),
                ..Default::default()
            },
        )
        .unwrap()
        .proposal_id
        .unwrap();
        assert!(decide_proposal(&mut db, None, &prop, true)
            .unwrap()
            .is_committed());

        let ctx = evolution_context(&db, "tool-task-tracker").expect("context");
        assert_eq!(
            ctx.get("applicationId").and_then(|v| v.as_str()),
            Some("tool-task-tracker")
        );
        let surface_revision = ctx
            .get("surfaceRevision")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert!(surface_revision >= 1);
        assert_eq!(
            ctx.get("baseRevisionRequired").and_then(|v| v.as_i64()),
            Some(surface_revision)
        );
        let components = ctx
            .get("components")
            .and_then(|v| v.as_array())
            .expect("components");
        assert!(components
            .iter()
            .any(|c| c.get("id") == Some(&json!("tm-list"))));
    }

    #[test]
    fn evolution_context_surface_id_from_lineage_when_canonical_surf_row_absent() {
        use crate::runtime_v2::surfaces::{bind_surface_tool_id, get_surface};

        const APP_ID: &str = "tool-task-tracker";

        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("evo-ctx-lineage.db")).unwrap();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Evolution ctx lineage", None)
                .unwrap();

        let manifest = r#"{"schemaVersion":"1","applicationId":"tool-task-tracker","instanceId":"inst-tt","name":"Task Tracker","description":"","version":1,"surfaces":[{"surfaceId":"surf-tool-task-tracker","placement":"tool_canvas"}],"routes":[],"dataModels":[],"settings":[],"capabilities":["coreside.core"],"permissions":["local_data.read"],"events":[],"tests":[],"searchKeywords":[],"tags":[],"agentDescription":"","applicationActionAccess":["local_data.query"],"surfaceActionAccess":{},"componentActionAccess":{}}"#;
        db.conn()
            .execute(
                "INSERT INTO application_manifests (
                    id, application_id, instance_id, schema_version, current_version,
                    manifest_json, health_state, lifecycle_state, created_at, updated_at
                 ) VALUES ('man-tt', ?1, 'inst-tt', '1', 1, ?2, 'healthy', 'active', datetime('now'), datetime('now'))",
                rusqlite::params![APP_ID, manifest],
            )
            .unwrap();

        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, ?2, ?1, '', 'stack', '{}', 1, datetime('now'), datetime('now'))",
                rusqlite::params![APP_ID, DEFAULT_WORKSPACE_ID],
            )
            .unwrap();

        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Task Tracker",
            &json!({
                "id": "doc-tt",
                "name": "Task Tracker",
                "layout": "stack",
                "components": [{"id": "tm-list", "type": "text", "props": {"text": "Tasks"}}]
            }),
            &[],
        )
        .unwrap();
        bind_surface_tool_id(&mut db, &surf.id, APP_ID).unwrap();

        let canonical = format!("surf-{APP_ID}");
        assert_ne!(surf.id, canonical);
        assert!(get_surface(&db, &canonical).is_err());

        let ctx = evolution_context(&db, APP_ID).expect("context");
        assert_eq!(
            ctx.get("surfaceId").and_then(|v| v.as_str()),
            Some(surf.id.as_str()),
            "surfaceId must come from DB lineage, not manifest/canonical surf-* naming"
        );
        assert_eq!(
            ctx.get("surfaceRevision").and_then(|v| v.as_i64()),
            Some(get_surface(&db, &surf.id).unwrap().current_revision)
        );
    }

    #[test]
    fn evolution_context_keeps_surface_id_when_application_suspended() {
        use crate::runtime_v2::surfaces::{bind_surface_tool_id, get_surface};

        const APP_ID: &str = "tool-task-tracker";

        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("evo-ctx-suspended.db")).unwrap();
        let conv = create_conversation(
            &mut db,
            DEFAULT_WORKSPACE_ID,
            "Evolution ctx suspended",
            None,
        )
        .unwrap();

        let manifest = r#"{"schemaVersion":"1","applicationId":"tool-task-tracker","instanceId":"inst-tt","name":"Task Tracker","description":"","version":1,"surfaces":[{"surfaceId":"surf-tool-task-tracker","placement":"tool_canvas"}],"routes":[],"dataModels":[],"settings":[],"capabilities":["coreside.core"],"permissions":["local_data.read"],"events":[],"tests":[],"searchKeywords":[],"tags":[],"agentDescription":"","applicationActionAccess":["local_data.query"],"surfaceActionAccess":{},"componentActionAccess":{}}"#;
        db.conn()
            .execute(
                "INSERT INTO application_manifests (
                    id, application_id, instance_id, schema_version, current_version,
                    manifest_json, health_state, lifecycle_state, created_at, updated_at
                 ) VALUES ('man-tt', ?1, 'inst-tt', '1', 1, ?2, 'healthy', 'suspended', datetime('now'), datetime('now'))",
                rusqlite::params![APP_ID, manifest],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES (?1, ?2, ?1, '', 'stack', '{}', 1, datetime('now'), datetime('now'))",
                rusqlite::params![APP_ID, DEFAULT_WORKSPACE_ID],
            )
            .unwrap();
        let surf = crate::runtime_v2::surfaces::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Task Tracker",
            &json!({
                "id": "doc-tt",
                "name": "Task Tracker",
                "layout": "stack",
                "components": [{"id": "tm-list", "type": "text", "props": {"text": "Tasks"}}]
            }),
            &[],
        )
        .unwrap();
        bind_surface_tool_id(&mut db, &surf.id, APP_ID).unwrap();

        // Mutate-path resolve must fail closed for suspended apps.
        assert!(
            crate::application_kernel::lineage::resolve_application_surface(
                &db, APP_ID, None, None
            )
            .is_err()
        );

        let ctx = evolution_context(&db, APP_ID).expect("read context still available");
        assert_eq!(
            ctx.get("surfaceId").and_then(|v| v.as_str()),
            Some(surf.id.as_str()),
            "suspended apps must still expose DB-bound surfaceId for OCC guidance"
        );
        assert_eq!(
            ctx.get("surfaceRevision").and_then(|v| v.as_i64()),
            Some(get_surface(&db, &surf.id).unwrap().current_revision)
        );
    }
}
