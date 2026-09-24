//! Declarative Application Manifest — versioned, validated, recoverable.

use std::collections::HashMap;

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
    /// Registered actions this application may call at all.
    #[serde(default)]
    pub application_action_access: Vec<String>,
    /// Per-surface narrowing, keyed by surface id. Must be a subset of
    /// `application_action_access`.
    #[serde(default)]
    pub surface_action_access: HashMap<String, Vec<String>>,
    /// Per-component narrowing, keyed by component id. Must be a subset of the
    /// owning surface's list where one is declared, otherwise of the
    /// application list.
    #[serde(default)]
    pub component_action_access: HashMap<String, Vec<String>>,
    /// Expected action descriptor hashes for tamper detection, keyed by action name.
    /// If present, the gateway verifies that the runtime action descriptor hash
    /// matches this value before invocation.
    #[serde(default)]
    pub action_descriptor_hashes: HashMap<String, String>,
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
        return Err(format!(
            "unsupported manifest schemaVersion: {}",
            m.schema_version
        ));
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
    validate_action_access(m)?;
    Ok(())
}

/// Declared action access must reference real actions, must be covered by the
/// declared permissions, and must narrow rather than widen: every surface and
/// component entry has to be a subset of the application list. Component ⊆
/// surface is enforced at call time by the gateway, which requires both the
/// surface and the component list to allow the action.
fn validate_action_access(m: &ApplicationManifest) -> Result<(), String> {
    use crate::application_kernel::registered_actions::descriptor::find_action;

    for name in &m.application_action_access {
        let descriptor =
            find_action(name).ok_or_else(|| format!("unknown registered action: {name}"))?;
        if !m
            .permissions
            .iter()
            .any(|p| p == &descriptor.permission_category)
        {
            return Err(format!(
                "action {name} needs the {} permission to be declared",
                descriptor.permission_category
            ));
        }
    }
    for (surface_id, actions) in &m.surface_action_access {
        for name in actions {
            if !m.application_action_access.contains(name) {
                return Err(format!(
                    "surface {surface_id} declares {name}, which the application does not declare"
                ));
            }
        }
    }
    for (component_id, actions) in &m.component_action_access {
        for name in actions {
            if !m.application_action_access.contains(name) {
                return Err(format!(
                    "component {component_id} declares {name}, which the application does not declare"
                ));
            }
        }
    }
    for (name, _) in &m.action_descriptor_hashes {
        if !m.application_action_access.is_empty() && !m.application_action_access.contains(name) {
            return Err(format!(
                "descriptor hash registered for {name}, which the application does not declare"
            ));
        }
    }
    Ok(())
}

/// Propagate SoftwareDocument action contracts into an ApplicationManifest.
///
/// Ensures component-level scoping (`component_action_access`) and tamper-detection
/// descriptor hashes (`action_descriptor_hashes`) are synchronized into the manifest.
pub fn sync_software_document_contracts(
    manifest: &mut ApplicationManifest,
    doc: &crate::runtime_v2::SoftwareDocument,
) {
    for contract in &doc.action_contracts {
        if let Some(ref comp_id) = contract.component_id {
            let list = manifest
                .component_action_access
                .entry(comp_id.clone())
                .or_default();
            if !list.contains(&contract.action_name) {
                list.push(contract.action_name.clone());
            }
        }
        if let Some(ref hash) = contract.descriptor_hash {
            manifest
                .action_descriptor_hashes
                .insert(contract.action_name.clone(), hash.clone());
        }
        if !manifest.application_action_access.is_empty()
            && !manifest
                .application_action_access
                .contains(&contract.action_name)
        {
            manifest
                .application_action_access
                .push(contract.action_name.clone());
        }
    }
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
    // SECURITY: we extract raw fields first, then parse manifest_json separately.
    // A malformed manifest must NEVER silently become an empty "Invalid" manifest — an
    // empty manifest has no permissions/surfaces/routes, causing false negatives in
    // authorization checks rather than hard errors.
    struct RawRow {
        id: String,
        instance_id: String,
        schema_version: String,
        current_version: i64,
        last_known_good_version: Option<i64>,
        manifest_json: String,
        health_state: String,
        lifecycle_state: String,
        disabled: i64,
        crash_count: i64,
        project_id: Option<String>,
        conversation_id: Option<String>,
        created_at: String,
        updated_at: String,
    }

    let raw: Option<RawRow> = db
        .conn()
        .query_row(
            "SELECT id, application_id, instance_id, schema_version, current_version,
                    last_known_good_version, manifest_json, health_state, lifecycle_state,
                    disabled, crash_count, project_id, conversation_id, created_at, updated_at
             FROM application_manifests WHERE application_id = ?1",
            [application_id],
            |row| {
                Ok(RawRow {
                    id: row.get(0)?,
                    // column 1 = application_id (unused; supplied by caller)
                    instance_id: row.get(2)?,
                    schema_version: row.get(3)?,
                    current_version: row.get(4)?,
                    last_known_good_version: row.get(5)?,
                    manifest_json: row.get(6)?,
                    health_state: row.get(7)?,
                    lifecycle_state: row.get(8)?,
                    disabled: row.get(9)?,
                    crash_count: row.get(10)?,
                    project_id: row.get(11)?,
                    conversation_id: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            },
        )
        .optional()
        .map_err(DbError::Sqlite)?;

    let raw = raw.ok_or_else(|| DbError::NotFound(format!("application {application_id}")))?;

    // Fail closed: malformed manifest JSON is a hard error, not a fallback.
    let manifest: ApplicationManifest = serde_json::from_str(&raw.manifest_json).map_err(|e| {
        DbError::Corrupted(format!(
            "manifest JSON for application '{}' is malformed: {}",
            application_id, e
        ))
    })?;

    Ok(ManifestRecord {
        id: raw.id,
        application_id: application_id.to_string(),
        instance_id: raw.instance_id,
        schema_version: raw.schema_version,
        current_version: raw.current_version,
        last_known_good_version: raw.last_known_good_version,
        manifest,
        health_state: raw.health_state,
        lifecycle_state: raw.lifecycle_state,
        disabled: raw.disabled != 0,
        crash_count: raw.crash_count,
        project_id: raw.project_id,
        conversation_id: raw.conversation_id,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    })
}

pub fn list_manifests(db: &Database) -> DbResult<Vec<ManifestRecord>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT application_id FROM application_manifests ORDER BY updated_at DESC")?;
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

pub fn restore_last_known_good(
    db: &mut Database,
    application_id: &str,
) -> DbResult<ManifestRecord> {
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
    let updated = db.conn().execute(
        "UPDATE application_manifests SET
            crash_count = crash_count + 1,
            lifecycle_state = CASE WHEN crash_count + 1 >= 3 THEN 'suspended' ELSE lifecycle_state END,
            health_state = CASE WHEN crash_count + 1 >= 3 THEN 'suspended' ELSE 'failed' END,
            updated_at = ?2
         WHERE application_id = ?1",
        params![application_id, now],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("application {application_id}")));
    }
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
    let permissions = vec!["local_data.read".into()];
    let application_action_access =
        super::registered_actions::descriptor::default_action_access_for_permissions(&permissions);
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
        permissions,
        events: vec![],
        tests: vec![],
        search_keywords: vec![tool_name.into()],
        tags: vec!["tool".into()],
        agent_description: format!("Personal tool: {tool_name}"),
        project_id: None,
        conversation_id: None,
        organization_id: None,
        ownership: None,
        application_action_access,
        surface_action_access: HashMap::new(),
        component_action_access: HashMap::new(),
        action_descriptor_hashes: HashMap::new(),
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
            application_action_access: vec![],
            surface_action_access: HashMap::new(),
            component_action_access: HashMap::new(),
            action_descriptor_hashes: HashMap::new(),
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

    #[test]
    fn accepts_declared_actions_backed_by_permissions() {
        let mut m = sample();
        m.application_action_access = vec!["local_data.query".into(), "local_data.write".into()];
        m.surface_action_access
            .insert("surface-1".into(), vec!["local_data.query".into()]);
        m.component_action_access
            .insert("component-1".into(), vec!["local_data.query".into()]);
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn ensure_manifest_declares_actions_for_default_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::db::Database::open_path(&dir.path().join("m.db")).unwrap();
        let record =
            ensure_manifest_for_tool(&mut db, "tool-1", "Notes", "surface-tool-1").unwrap();
        assert!(record
            .manifest
            .application_action_access
            .contains(&"local_data.query".to_string()));
        assert!(!record.manifest.application_action_access.is_empty());
    }

    #[test]
    fn non_retryable_build_failures_suspend_after_three_strikes() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::db::Database::open_path(&dir.path().join("crash.db")).unwrap();
        let app_id = ensure_manifest_for_tool(&mut db, "tool-crash", "Notes", "surface-tool-crash")
            .unwrap()
            .application_id;
        mark_last_known_good(&mut db, &app_id).unwrap();

        // Retryable failures are transient and must not count as crashes.
        crate::application_kernel::lifecycle::record_build_failure(
            &mut db, &app_id, "flaky", true, None,
        )
        .unwrap();
        assert_eq!(get_manifest(&db, &app_id).unwrap().crash_count, 0);

        for _ in 0..2 {
            crate::application_kernel::lifecycle::record_build_failure(
                &mut db, &app_id, "broken", false, None,
            )
            .unwrap();
        }
        let two = get_manifest(&db, &app_id).unwrap();
        assert_eq!(two.crash_count, 2);
        assert_ne!(two.lifecycle_state, "suspended");

        crate::application_kernel::lifecycle::record_build_failure(
            &mut db, &app_id, "broken", false, None,
        )
        .unwrap();
        let three = get_manifest(&db, &app_id).unwrap();
        assert_eq!(three.crash_count, 3);
        assert_eq!(three.lifecycle_state, "suspended");
        assert_eq!(three.health_state, "suspended");

        // Restoring the last known good version clears the strike count.
        let restored = restore_last_known_good(&mut db, &app_id).unwrap();
        assert_eq!(restored.crash_count, 0);
        assert_ne!(restored.lifecycle_state, "suspended");
    }

    #[test]
    fn rejects_unknown_action() {
        let mut m = sample();
        m.application_action_access = vec!["shell.exec".into()];
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn rejects_action_without_matching_permission() {
        let mut m = sample();
        // `media.read` requires the media.read permission, which is not declared.
        m.application_action_access = vec!["media.read".into()];
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn rejects_surface_widening_beyond_application() {
        let mut m = sample();
        m.application_action_access = vec!["local_data.query".into()];
        m.surface_action_access
            .insert("surface-1".into(), vec!["local_data.write".into()]);
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn rejects_component_widening_beyond_application() {
        let mut m = sample();
        m.application_action_access = vec!["local_data.query".into()];
        m.component_action_access
            .insert("component-1".into(), vec!["local_data.write".into()]);
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn get_manifest_fails_closed_on_corrupted_json() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Database::open_path(&dir.path().join("corrupt.db")).unwrap();
        let app_id = "app-corrupted-test";
        let now = now_rfc3339();

        // Directly insert malformed manifest JSON into the table
        db.conn()
            .execute(
                "INSERT INTO application_manifests (
                    id, application_id, instance_id, schema_version, current_version,
                    manifest_json, health_state, lifecycle_state, project_id, conversation_id,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, '1', 1, '{\"invalid\": [unterminated', 'healthy', 'active', NULL, NULL, ?4, ?4)",
                rusqlite::params!["rec-1", app_id, "inst-1", now],
            )
            .unwrap();

        let res = get_manifest(&db, app_id);
        assert!(res.is_err());
        match res.unwrap_err() {
            crate::db::DbError::Corrupted(msg) => {
                assert!(msg.contains("is malformed"));
            }
            other => panic!("expected DbError::Corrupted, got: {other:?}"),
        }
    }

    #[test]
    fn sync_software_document_contracts_propagates_contracts() {
        let mut m = sample();
        let mut doc = crate::runtime_v2::SoftwareDocument::new("doc-1", "Test Doc");
        doc.action_contracts = vec![crate::runtime_v2::ActionContract {
            action_id: "act-1".into(),
            action_name: "local_data.query".into(),
            component_id: Some("comp-table".into()),
            descriptor_hash: Some("sha256:abc1234".into()),
            description: None,
            result_key: None,
            input_from_state: None,
        }];

        sync_software_document_contracts(&mut m, &doc);

        assert_eq!(
            m.component_action_access.get("comp-table"),
            Some(&vec!["local_data.query".to_string()])
        );
        assert_eq!(
            m.action_descriptor_hashes.get("local_data.query"),
            Some(&"sha256:abc1234".to_string())
        );
    }
}
