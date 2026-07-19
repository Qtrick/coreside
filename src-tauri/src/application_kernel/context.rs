//! Token-efficient application context for the agent.

use serde_json::{json, Value};

use crate::db::{Database, DbResult};

use super::manifest::get_manifest;
use super::data::{get_model, query_records};

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

pub fn recent_transactions_summary(db: &Database, application_id: &str, limit: usize) -> DbResult<Value> {
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
    let items: Vec<Value> = rows.filter_map(|r| r.ok()).collect();
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
