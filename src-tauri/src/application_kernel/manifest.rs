//! Declarative Application Manifest — versioned, validated, recoverable.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use crate::runtime_v2::limits::MAX_MANIFEST_JSON_BYTES;
use crate::runtime_v2::operations::AppOperation;
use crate::security::assert_not_protected;
use rusqlite::params;

use super::permissions::validate_declared_permissions;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationManifest {
    pub schema_version: String,
    pub application_id: String,
    pub instance_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub surfaces: Vec<ManifestSurfaceRef>,
    #[serde(default)]
    pub routes: Vec<ManifestRoute>,
    #[serde(default)]
    pub data_models: Vec<ManifestDataModelRef>,
    #[serde(default)]
    pub settings: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub tests: Vec<String>,
    #[serde(default)]
    pub search_keywords: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub agent_description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ownership: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSurfaceRef {
    pub surface_id: String,
    #[serde(default)]
    pub placement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRoute {
    pub route_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestDataModelRef {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRecord {
    pub id: String,
    pub application_id: String,
    pub instance_id: String,
    pub schema_version: String,
    pub current_version: i64,
    pub last_known_good_version: Option<i64>,
    pub manifest: ApplicationManifest,
    pub health_state: String,
    pub lifecycle_state: String,
    pub disabled: bool,
    pub crash_count: i64,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn validate_manifest(m: &ApplicationManifest) -> Result<(), String> {
    assert_not_protected(&m.application_id)?;
    if m.application_id.trim().is_empty() || m.instance_id.trim().is_empty() {
        return Err("applicationId and instanceId are required".into());
    }
    if m.name.trim().is_empty() {
        return Err("name is required".into());
    }
    if m.schema_version != "1" {
        return Err(format!("unsupported manifest schemaVersion: {}", m.schema_version));
    }
    // Reject secrets / paths / executable blobs
    let raw = serde_json::to_string(m).map_err(|e| e.to_string())?;
    if raw.len() > MAX_MANIFEST_JSON_BYTES {
        return Err("manifest exceeds size limit".into());
    }
    let lower = raw.to_lowercase();
    if lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("authorization")
        || lower.contains("/users/")
        || lower.contains("file://")
    {
        return Err("manifest must not contain secrets or absolute paths".into());
    }
    if lower.contains("<script") || lower.contains("javascript:") {
        return Err("manifest must not contain executable content".into());
    }
    validate_declared_permissions(&m.permissions)?;
    for route in &m.routes {
        let id = route.route_id.to_lowercase();
        if id.contains("settings") && id.contains("core")
            || id.starts_with("coreside.")
            || id == "settings"
            || id == "recovery"
            || id == "providers"
        {
            return Err(format!(
                "route collides with protected Coreside navigation: {}",
                route.route_id
            ));
        }
    }
    for cap in &m.capabilities {
        if cap.starts_with("cdn.") || cap.contains("://") {
            return Err(format!("invalid capability pack id: {cap}"));
        }
    }
    Ok(())
}

pub fn upsert_manifest(db: &mut Database, mut m: ApplicationManifest) -> DbResult<ManifestRecord> {
    validate_manifest(&m).map_err(DbError::Invalid)?;
    if m.version <= 0 {
        m.version = 1;
    }
    let now = now_rfc3339();
    let json = serde_json::to_string(&m)?;
    let existing: Option<(String, i64)> = db
        .conn()
        .query_row(
            "SELECT id, current_version FROM application_manifests WHERE application_id = ?1",
            [&m.application_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(DbError::Sqlite)?;

    let (_id, next_version) = if let Some((id, cur)) = existing {
        let next = cur + 1;
        m.version = next;
        let json = serde_json::to_string(&m)?;
        db.conn().execute(
            "UPDATE application_manifests SET
                instance_id = ?2, schema_version = ?3, current_version = ?4,
                manifest_json = ?5, project_id = ?6, conversation_id = ?7,
                health_state = 'testing', updated_at = ?8
             WHERE application_id = ?1",
            params![
                m.application_id,
                m.instance_id,
                m.schema_version,
                next,
                json,
                m.project_id,
                m.conversation_id,
                now
            ],
        )?;
        (id, next)
    } else {
        let id = format!("app-row-{}", Uuid::new_v4());
        db.conn().execute(
            "INSERT INTO application_manifests (
                id, application_id, instance_id, schema_version, current_version,
                manifest_json, health_state, lifecycle_state, project_id, conversation_id,
                created_at, updated_at
             ) VALUES (?1,?2,?3,?4,?5,?6,'testing','active',?7,?8,?9,?9)",
            params![
                id,
                m.application_id,
                m.instance_id,
                m.schema_version,
                m.version,
                json,
                m.project_id,
                m.conversation_id,
                now
            ],
        )?;
        (id, m.version)
    };

    let version_id = format!("appver-{}", Uuid::new_v4());
    let json = serde_json::to_string(&m)?;
    db.conn().execute(
        "INSERT INTO application_manifest_versions (
            id, application_id, version, manifest_json, validation_status, test_status, created_at
         ) VALUES (?1,?2,?3,?4,'pending','pending',?5)",
        params![version_id, m.application_id, next_version, json, now],
    )?;

    get_manifest(db, &m.application_id)
}

use rusqlite::OptionalExtension;

pub fn get_manifest(db: &Database, application_id: &str) -> DbResult<ManifestRecord> {
    db.conn()
        .query_row(
            "SELECT id, application_id, instance_id, schema_version, current_version,
                    last_known_good_version, manifest_json, health_state, lifecycle_state,
                    disabled, crash_count, project_id, conversation_id, created_at, updated_at
             FROM application_manifests WHERE application_id = ?1",
            [application_id],
            |row| {
                let mj: String = row.get(6)?;
                let manifest: ApplicationManifest =
                    serde_json::from_str(&mj).unwrap_or_else(|_| ApplicationManifest {
                        schema_version: "1".into(),
                        application_id: application_id.into(),
                        instance_id: row.get::<_, String>(2).unwrap_or_default(),
                        name: "Invalid".into(),
                        description: String::new(),
                        version: row.get(4).unwrap_or(1),
                        surfaces: vec![],
                        routes: vec![],
                        data_models: vec![],
                        settings: vec![],
                        capabilities: vec![],
                        permissions: vec![],
                        events: vec![],
                        tests: vec![],
                        search_keywords: vec![],
                        tags: vec![],
                        agent_description: String::new(),
                        project_id: None,
                        conversation_id: None,
                        organization_id: None,
                        ownership: None,
                    });
                Ok(ManifestRecord {
                    id: row.get(0)?,
                    application_id: row.get(1)?,
                    instance_id: row.get(2)?,
                    schema_version: row.get(3)?,
                    current_version: row.get(4)?,
                    last_known_good_version: row.get(5)?,
                    manifest,
                    health_state: row.get(7)?,
                    lifecycle_state: row.get(8)?,
                    disabled: row.get::<_, i64>(9)? != 0,
                    crash_count: row.get(10)?,
                    project_id: row.get(11)?,
                    conversation_id: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("application {application_id}"))
            }
            other => DbError::Sqlite(other),
        })
}

pub fn list_manifests(db: &Database) -> DbResult<Vec<ManifestRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT application_id FROM application_manifests ORDER BY updated_at DESC",
    )?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    let mut out = Vec::new();
    for id in ids {
        out.push(get_manifest(db, &id)?);
    }
    Ok(out)
}

pub fn mark_last_known_good(db: &mut Database, application_id: &str) -> DbResult<()> {
    let rec = get_manifest(db, application_id)?;
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE application_manifests SET
            last_known_good_version = current_version,
            health_state = 'healthy',
            updated_at = ?2
         WHERE application_id = ?1",
        params![application_id, now],
    )?;
    db.conn().execute(
        "UPDATE application_manifest_versions SET is_known_good = 1, validation_status = 'passed', test_status = 'passed'
         WHERE application_id = ?1 AND version = ?2",
        params![application_id, rec.current_version],
    )?;
    Ok(())
}

pub fn restore_last_known_good(db: &mut Database, application_id: &str) -> DbResult<ManifestRecord> {
    let rec = get_manifest(db, application_id)?;
    let lkg = rec
        .last_known_good_version
        .ok_or_else(|| DbError::Invalid("no last-known-good version".into()))?;
    let json: String = db.conn().query_row(
        "SELECT manifest_json FROM application_manifest_versions
         WHERE application_id = ?1 AND version = ?2 AND is_known_good = 1",
        params![application_id, lkg],
        |row| row.get(0),
    )?;
    let mut m: ApplicationManifest = serde_json::from_str(&json)?;
    m.version = lkg;
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE application_manifests SET
            current_version = ?2, manifest_json = ?3, health_state = 'healthy',
            lifecycle_state = 'restored', disabled = 0, crash_count = 0, updated_at = ?4
         WHERE application_id = ?1",
        params![application_id, lkg, json, now],
    )?;
    get_manifest(db, application_id)
}

pub fn record_crash(db: &mut Database, application_id: &str) -> DbResult<ManifestRecord> {
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE application_manifests SET
            crash_count = crash_count + 1,
            lifecycle_state = CASE WHEN crash_count + 1 >= 3 THEN 'suspended' ELSE lifecycle_state END,
            health_state = CASE WHEN crash_count + 1 >= 3 THEN 'suspended' ELSE 'failed' END,
            updated_at = ?2
         WHERE application_id = ?1",
        params![application_id, now],
    )?;
    get_manifest(db, application_id)
}

pub fn apply_manifest_operations(db: &mut Database, ops: &[AppOperation]) -> DbResult<()> {
    for op in ops {
        match op.op_type.as_str() {
            "manifest.upsert" => {
                let m: ApplicationManifest = serde_json::from_value(op.payload.clone())
                    .map_err(|e| DbError::Invalid(e.to_string()))?;
                upsert_manifest(db, m)?;
            }
            "manifest.restore_last_known_good" => {
                let app_id = op
                    .target
                    .application_id
                    .as_deref()
                    .or_else(|| op.payload.get("applicationId").and_then(|v| v.as_str()))
                    .ok_or_else(|| DbError::Invalid("applicationId required".into()))?;
                restore_last_known_good(db, app_id)?;
            }
            "manifest.disable" => {
                let app_id = op
                    .target
                    .application_id
                    .as_deref()
                    .or_else(|| op.payload.get("applicationId").and_then(|v| v.as_str()))
                    .ok_or_else(|| DbError::Invalid("applicationId required".into()))?;
                db.conn().execute(
                    "UPDATE application_manifests SET disabled = 1, lifecycle_state = 'disabled', updated_at = ?2 WHERE application_id = ?1",
                    params![app_id, now_rfc3339()],
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Wrap an existing tool as a minimal application manifest (idempotent).
pub fn ensure_manifest_for_tool(
    db: &mut Database,
    tool_id: &str,
    tool_name: &str,
    surface_id: &str,
) -> DbResult<ManifestRecord> {
    if get_manifest(db, tool_id).is_ok() {
        return get_manifest(db, tool_id);
    }
    let m = ApplicationManifest {
        schema_version: "1".into(),
        application_id: tool_id.into(),
        instance_id: format!("instance-{tool_id}"),
        name: tool_name.into(),
        description: String::new(),
        version: 1,
        surfaces: vec![ManifestSurfaceRef {
            surface_id: surface_id.into(),
            placement: "tool_canvas".into(),
            definition_ref: None,
        }],
        routes: vec![],
        data_models: vec![],
        settings: vec![],
        capabilities: vec!["coreside.core".into()],
        permissions: vec!["local_data.read".into()],
        events: vec![],
        tests: vec![],
        search_keywords: vec![tool_name.into()],
        tags: vec!["tool".into()],
        agent_description: format!("Personal tool: {tool_name}"),
        project_id: None,
        conversation_id: None,
        organization_id: None,
        ownership: None,
    };
    upsert_manifest(db, m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ApplicationManifest {
        ApplicationManifest {
            schema_version: "1".into(),
            application_id: "app-study".into(),
            instance_id: "instance-study-1".into(),
            name: "Biology Study Dashboard".into(),
            description: "Tracks study sessions".into(),
            version: 1,
            surfaces: vec![],
            routes: vec![ManifestRoute {
                route_id: "overview".into(),
                title: "Overview".into(),
                surface_id: None,
            }],
            data_models: vec![],
            settings: vec!["setting.daily-goal".into()],
            capabilities: vec!["coreside.core".into(), "coreside.charts".into()],
            permissions: vec!["local_data.read".into(), "local_data.write".into()],
            events: vec![],
            tests: vec![],
            search_keywords: vec!["biology".into()],
            tags: vec![],
            agent_description: "Study dashboard".into(),
            project_id: None,
            conversation_id: None,
            organization_id: None,
            ownership: None,
        }
    }

    #[test]
    fn validates_ok() {
        assert!(validate_manifest(&sample()).is_ok());
    }

    #[test]
    fn rejects_core_id() {
        let mut m = sample();
        m.application_id = "core.recovery_mode".into();
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn rejects_forbidden_permission() {
        let mut m = sample();
        m.permissions.push("credential.read".into());
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn rejects_protected_route() {
        let mut m = sample();
        m.routes.push(ManifestRoute {
            route_id: "settings".into(),
            title: "Settings".into(),
            surface_id: None,
        });
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn rejects_secret_shaped() {
        let mut m = sample();
        m.description = "api_key=sk-test".into();
        assert!(validate_manifest(&m).is_err());
    }
}
