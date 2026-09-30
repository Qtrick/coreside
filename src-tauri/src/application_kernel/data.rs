//! Safe generated data models — declarative schemas compiled to namespaced CRUD.
//! No arbitrary SQL from the agent.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use crate::runtime_v2::limits::MAX_GENERATED_RECORDS_PER_QUERY;
use crate::runtime_v2::operations::AppOperation;
use rusqlite::{params, OptionalExtension};
use std::collections::HashSet;

use super::permissions::assert_can_write_data;

/// Drop query meta keys and undeclared fields before validate_record / persist.
fn sanitize_record_payload(data: &mut Value, known: &HashSet<&str>) {
    if let Some(obj) = data.as_object_mut() {
        obj.remove("_id");
        obj.remove("_version");
        obj.remove("_createdAt");
        obj.remove("_updatedAt");
        obj.retain(|k, _| known.contains(k.as_str()));
    }
}

fn known_field_ids(def: &DataModelDefinition) -> HashSet<&str> {
    def.fields.iter().map(|f| f.field_id.as_str()).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DataModelDefinition {
    pub model_id: String,
    pub display_name: String,
    pub fields: Vec<DataField>,
    #[serde(default)]
    pub schema_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DataField {
    pub field_id: String,
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

const ALLOWED_FIELD_TYPES: &[&str] = &[
    "text",
    "long_text",
    "integer",
    "decimal",
    "boolean",
    "date",
    "time",
    "date_time",
    "duration",
    "enum",
    "reference",
    "media_ref",
    "surface_ref",
    "tool_ref",
    "json",
    "created_at",
    "updated_at",
];

pub fn validate_model(def: &DataModelDefinition) -> Result<(), String> {
    if def.model_id.trim().is_empty() || def.model_id.contains(' ') {
        return Err("modelId must be a non-empty identifier".into());
    }
    if def.model_id.contains(';') || def.model_id.contains("--") {
        return Err("invalid modelId".into());
    }
    if def.fields.is_empty() {
        return Err("model must declare fields".into());
    }
    for f in &def.fields {
        if !ALLOWED_FIELD_TYPES.contains(&f.field_type.as_str()) {
            return Err(format!("unsupported field type: {}", f.field_type));
        }
        if f.field_id.contains(';') || f.field_id.contains("--") {
            return Err("invalid fieldId".into());
        }
        if f.field_type == "enum" && f.enum_values.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
            return Err(format!("enum field {} needs enumValues", f.field_id));
        }
    }
    Ok(())
}

pub fn validate_record(def: &DataModelDefinition, data: &Value) -> Result<(), String> {
    let obj = data
        .as_object()
        .ok_or_else(|| "record must be an object".to_string())?;
    // Strict schema: reject undeclared fields. Extension data requires an explicit
    // schema field (e.g. type "json") rather than silent open-object storage.
    // Migrations must sanitize payloads (see sanitize_record_payload) before update.
    let known = known_field_ids(def);
    for key in obj.keys() {
        if !known.contains(key.as_str()) {
            return Err(format!("unknown field: {key}"));
        }
    }
    for f in &def.fields {
        let val = obj.get(&f.field_id);
        if f.required && (val.is_none() || val == Some(&Value::Null)) && f.default.is_none() {
            return Err(format!("missing required field: {}", f.field_id));
        }
        if let Some(v) = val {
            match f.field_type.as_str() {
                "integer" if !v.is_i64() && !v.is_u64() => {
                    return Err(format!("{} must be integer", f.field_id));
                }
                "decimal" if !v.is_number() => {
                    return Err(format!("{} must be number", f.field_id));
                }
                "boolean" if !v.is_boolean() => {
                    return Err(format!("{} must be boolean", f.field_id));
                }
                "text" | "long_text" | "date" | "time" | "date_time" | "duration" | "reference"
                | "media_ref" | "surface_ref" | "tool_ref"
                    if !v.is_string() && !v.is_null() =>
                {
                    return Err(format!("{} must be string", f.field_id));
                }
                "enum" => {
                    if let Some(ev) = &f.enum_values {
                        if let Some(s) = v.as_str() {
                            if !ev.iter().any(|x| x == s) {
                                return Err(format!("invalid enum value for {}", f.field_id));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub fn upsert_model(
    db: &mut Database,
    application_id: &str,
    mut def: DataModelDefinition,
) -> DbResult<()> {
    assert_can_write_data(db, application_id)?;
    validate_model(&def).map_err(DbError::Invalid)?;
    if def.schema_version <= 0 {
        def.schema_version = 1;
    }
    let id = format!("dm-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let json = serde_json::to_string(&def)?;
    db.conn().execute(
        "INSERT INTO generated_data_models (
            id, application_id, model_id, schema_version, definition_json,
            current_migration_version, created_at, updated_at
         ) VALUES (?1,?2,?3,?4,?5,?4,?6,?6)
         ON CONFLICT(application_id, model_id) DO UPDATE SET
            definition_json = excluded.definition_json,
            schema_version = excluded.schema_version,
            updated_at = excluded.updated_at",
        params![
            id,
            application_id,
            def.model_id,
            def.schema_version,
            json,
            now
        ],
    )?;
    Ok(())
}

pub fn get_model(
    db: &Database,
    application_id: &str,
    model_id: &str,
) -> DbResult<DataModelDefinition> {
    let json: String = db
        .conn()
        .query_row(
            "SELECT definition_json FROM generated_data_models
             WHERE application_id = ?1 AND model_id = ?2",
            params![application_id, model_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!(
                "model_missing: application='{application_id}' model='{model_id}'. \
                 Call data.model_upsert for this model before data.record_create/update."
            )),
            other => DbError::Sqlite(other),
        })?;
    serde_json::from_str(&json).map_err(|e| DbError::Invalid(e.to_string()))
}

pub fn list_models(db: &Database, application_id: &str) -> DbResult<Vec<DataModelDefinition>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT definition_json FROM generated_data_models WHERE application_id = ?1")?;
    let rows = stmt.query_map([application_id], |r| {
        let json_str: String = r.get(0)?;
        serde_json::from_str(&json_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn create_record(
    db: &mut Database,
    application_id: &str,
    model_id: &str,
    mut data: Value,
) -> DbResult<String> {
    assert_can_write_data(db, application_id)?;
    let def = get_model(db, application_id, model_id)?;
    // Apply defaults
    if let Some(obj) = data.as_object_mut() {
        for f in &def.fields {
            if !obj.contains_key(&f.field_id) {
                if let Some(d) = &f.default {
                    obj.insert(f.field_id.clone(), d.clone());
                } else if f.field_type == "created_at" || f.field_type == "updated_at" {
                    obj.insert(f.field_id.clone(), json!(now_rfc3339()));
                }
            }
        }
    }
    validate_record(&def, &data).map_err(DbError::Invalid)?;
    let id = format!("rec-{}", Uuid::new_v4());
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO generated_data_records (
            id, application_id, model_id, record_version, data_json, created_at, updated_at
         ) VALUES (?1,?2,?3,1,?4,?5,?5)",
        params![id, application_id, model_id, data.to_string(), now],
    )?;
    Ok(id)
}

pub fn update_record(
    db: &mut Database,
    application_id: &str,
    record_id: &str,
    data: Value,
    base_version: Option<i64>,
) -> DbResult<()> {
    assert_can_write_data(db, application_id)?;
    let (model_id, version): (String, i64) = db
        .conn()
        .query_row(
            "SELECT model_id, record_version FROM generated_data_records
         WHERE id = ?1 AND application_id = ?2",
            params![record_id, application_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("record {record_id}"))
            }
            other => DbError::Sqlite(other),
        })?;
    if let Some(base) = base_version {
        if base != version {
            return Err(DbError::Invalid(format!(
                "revision_conflict: expected {base}, found {version}"
            )));
        }
    }
    let def = get_model(db, application_id, &model_id)?;
    validate_record(&def, &data).map_err(DbError::Invalid)?;
    db.conn().execute(
        "UPDATE generated_data_records SET data_json = ?3, record_version = record_version + 1, updated_at = ?4
         WHERE id = ?1 AND application_id = ?2",
        params![record_id, application_id, data.to_string(), now_rfc3339()],
    )?;
    Ok(())
}

pub fn delete_record(db: &mut Database, application_id: &str, record_id: &str) -> DbResult<()> {
    assert_can_write_data(db, application_id)?;
    let removed = db.conn().execute(
        "DELETE FROM generated_data_records WHERE id = ?1 AND application_id = ?2",
        params![record_id, application_id],
    )?;
    if removed == 0 {
        return Err(DbError::NotFound(format!("record {record_id}")));
    }
    Ok(())
}

pub fn query_records(
    db: &Database,
    application_id: &str,
    model_id: &str,
    limit: usize,
) -> DbResult<Vec<Value>> {
    let lim = limit.min(MAX_GENERATED_RECORDS_PER_QUERY);
    let mut stmt = db.conn().prepare(
        "SELECT id, record_version, data_json, created_at, updated_at FROM generated_data_records
         WHERE application_id = ?1 AND model_id = ?2
         ORDER BY updated_at DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![application_id, model_id, lim as i64], |row| {
        let id: String = row.get(0)?;
        let version: i64 = row.get(1)?;
        let data_s: String = row.get(2)?;
        let created: String = row.get(3)?;
        let updated: String = row.get(4)?;
        let mut data: Value = serde_json::from_str(&data_s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?;
        if let Some(obj) = data.as_object_mut() {
            obj.insert("_id".into(), json!(id));
            obj.insert("_version".into(), json!(version));
            obj.insert("_createdAt".into(), json!(created));
            obj.insert("_updatedAt".into(), json!(updated));
        }
        Ok(data)
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Fork a manifest-backed application for a conversation branch.
///
/// Copies models, records (new IDs), granted permissions, and a `tools` row
/// (surfaces.tool_id FK) into a new `application_id` bound to `new_conversation_id`.
/// Branch mutations must not mutate the source application's generated data.
pub fn fork_application_for_branch(
    db: &mut Database,
    source_application_id: &str,
    new_conversation_id: &str,
) -> DbResult<String> {
    use super::manifest::{get_manifest, upsert_manifest};

    let sp_name = format!("sp_fork_{}", Uuid::new_v4().simple());
    db.conn()
        .execute_batch(&format!("SAVEPOINT {sp_name};"))?;

    let res = (|| -> DbResult<String> {
        let source = get_manifest(db, source_application_id)?;
        let new_app_id = format!("app-branch-{}", Uuid::new_v4());
        let app_name = source.manifest.name.clone();
        let mut manifest = source.manifest;
        manifest.application_id = new_app_id.clone();
        manifest.instance_id = format!("inst-{}", Uuid::new_v4());
        manifest.conversation_id = Some(new_conversation_id.to_string());
        // Fresh INSERT gets lifecycle active / health testing (not source crash state).
        upsert_manifest(db, manifest)?;
        ensure_tools_row_for_forked_app(db, source_application_id, &new_app_id, &app_name)?;

        for grant in super::permissions::list_permissions(db, source_application_id)? {
            if grant.status == "granted" {
                super::permissions::grant_permission(
                    db,
                    &new_app_id,
                    &grant.permission,
                    grant.scope,
                    "branch_fork",
                )?;
            }
        }

        for model in list_models(db, source_application_id)? {
            let record_count: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM generated_data_records
                 WHERE application_id = ?1 AND model_id = ?2",
                params![source_application_id, model.model_id],
                |row| row.get(0),
            )?;
            if record_count as usize > MAX_GENERATED_RECORDS_PER_QUERY {
                return Err(DbError::Invalid(format!(
                    "cannot fork application '{source_application_id}': model '{}' has {record_count} records (max {MAX_GENERATED_RECORDS_PER_QUERY})",
                    model.model_id
                )));
            }
            upsert_model(db, &new_app_id, model.clone())?;
            let known = known_field_ids(&model);
            let records = query_records(
                db,
                source_application_id,
                &model.model_id,
                MAX_GENERATED_RECORDS_PER_QUERY,
            )?;
            for mut rec in records {
                sanitize_record_payload(&mut rec, &known);
                create_record(db, &new_app_id, &model.model_id, rec)?;
            }
        }

        Ok(new_app_id)
    })();

    match res {
        Ok(id) => {
            db.conn()
                .execute_batch(&format!("RELEASE SAVEPOINT {sp_name};"))?;
            Ok(id)
        }
        Err(err) => {
            let _ = db
                .conn()
                .execute_batch(&format!("ROLLBACK TO SAVEPOINT {sp_name};"));
            let _ = db
                .conn()
                .execute_batch(&format!("RELEASE SAVEPOINT {sp_name};"));
            Err(err)
        }
    }
}

/// `surfaces.tool_id` FK → `tools(id)`; forked application ids must exist as tools.
fn ensure_tools_row_for_forked_app(
    db: &mut Database,
    source_application_id: &str,
    new_application_id: &str,
    name: &str,
) -> DbResult<()> {
    let now = now_rfc3339();
    let source_tool: Option<(String, String, String, String, String)> = db
        .conn()
        .query_row(
            "SELECT workspace_id, name, description, layout, definition_json
             FROM tools WHERE id = ?1",
            [source_application_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(DbError::Sqlite)?;

    if let Some((workspace_id, src_name, description, layout, definition_json)) = source_tool {
        db.conn().execute(
            "INSERT INTO tools (
                id, workspace_id, name, description, layout, definition_json,
                current_version, created_at, updated_at
             ) VALUES (?1,?2,?3,?4,?5,?6,1,?7,?7)",
            params![
                new_application_id,
                workspace_id,
                format!("{src_name} (branch)"),
                description,
                layout,
                definition_json,
                now
            ],
        )?;
    } else {
        db.conn().execute(
            "INSERT INTO tools (
                id, workspace_id, name, description, layout, definition_json,
                current_version, created_at, updated_at
             ) VALUES (?1,?2,?3,'','stack','{}',1,?4,?4)",
            params![
                new_application_id,
                crate::db::DEFAULT_WORKSPACE_ID,
                name,
                now
            ],
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataMigration {
    pub migration_type: String,
    pub field_id: Option<String>,
    pub new_field_id: Option<String>,
    pub field_type: Option<String>,
    pub default: Option<Value>,
    pub required: Option<bool>,
}

pub fn apply_migration(
    db: &mut Database,
    application_id: &str,
    model_id: &str,
    migration: &DataMigration,
    confirm_destructive: bool,
) -> DbResult<String> {
    assert_can_write_data(db, application_id)?;
    let sp_name = format!("sp_mig_{}", Uuid::new_v4().simple());
    db.conn().execute_batch(&format!("SAVEPOINT {sp_name};"))?;

    let res = (|| -> DbResult<String> {
        let mut def = get_model(db, application_id, model_id)?;
        let from = def.schema_version;
        let impact = match migration.migration_type.as_str() {
            "add_field" => {
                let fid = migration
                    .field_id
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("fieldId required".into()))?;
                let ftype = migration
                    .field_type
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("fieldType required".into()))?;
                if def.fields.iter().any(|f| f.field_id == fid) {
                    return Err(DbError::Invalid(format!("field {fid} already exists")));
                }
                def.fields.push(DataField {
                    field_id: fid.into(),
                    field_type: ftype.into(),
                    required: migration.required.unwrap_or(false),
                    default: migration.default.clone(),
                    enum_values: None,
                });
                format!("Adds field {fid} ({ftype})")
            }
            "rename_field" => {
                let old = migration
                    .field_id
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("fieldId required".into()))?;
                let new = migration
                    .new_field_id
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("newFieldId required".into()))?;
                let f = def
                    .fields
                    .iter_mut()
                    .find(|f| f.field_id == old)
                    .ok_or_else(|| DbError::Invalid(format!("field {old} not found")))?;
                f.field_id = new.into();
                format!("Renames {old} to {new}")
            }
            "remove_field" => {
                if !confirm_destructive {
                    return Err(DbError::Invalid(
                        "destructive migration requires confirmation".into(),
                    ));
                }
                let fid = migration
                    .field_id
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("fieldId required".into()))?;
                def.fields.retain(|f| f.field_id != fid);
                format!("Removes field {fid} (destructive)")
            }
            other => {
                return Err(DbError::Invalid(format!(
                    "unsupported migration type: {other}"
                )))
            }
        };

        def.schema_version = from + 1;
        validate_model(&def).map_err(DbError::Invalid)?;
        let mid = format!("mig-{}", Uuid::new_v4());
        let now = now_rfc3339();
        // Backup snapshot in rollback_json
        let backup = serde_json::to_string(&get_model(db, application_id, model_id)?)?;
        db.conn().execute(
            "INSERT INTO generated_data_migrations (
                id, application_id, model_id, from_version, to_version, migration_json,
                impact_summary, status, rollback_json, applied_at, created_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,'applied',?8,?9,?9)",
            params![
                mid,
                application_id,
                model_id,
                from,
                def.schema_version,
                serde_json::to_string(migration)?,
                impact,
                backup,
                now
            ],
        )?;
        // Persist schema before rewriting records so update_record validates against the new model.
        upsert_model(db, application_id, def)?;
        let updated_def = get_model(db, application_id, model_id)?;
        let known = known_field_ids(&updated_def);
        match migration.migration_type.as_str() {
            "add_field" => {
                if let (Some(fid), Some(default)) = (&migration.field_id, &migration.default) {
                    let records = query_records(
                        db,
                        application_id,
                        model_id,
                        MAX_GENERATED_RECORDS_PER_QUERY,
                    )?;
                    for mut rec in records {
                        if let Some(obj) = rec.as_object_mut() {
                            if !obj.contains_key(fid) {
                                let id = obj
                                    .get("_id")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| DbError::Invalid("record missing _id".into()))?
                                    .to_string();
                                obj.insert(fid.clone(), default.clone());
                                sanitize_record_payload(&mut rec, &known);
                                update_record(db, application_id, &id, rec, None)?;
                            }
                        }
                    }
                }
            }
            "rename_field" => {
                let old = migration.field_id.as_deref().unwrap_or("");
                let new = migration.new_field_id.as_deref().unwrap_or("");
                let records = query_records(
                    db,
                    application_id,
                    model_id,
                    MAX_GENERATED_RECORDS_PER_QUERY,
                )?;
                for mut rec in records {
                    if let Some(obj) = rec.as_object_mut() {
                        if let Some(v) = obj.remove(old) {
                            obj.insert(new.into(), v);
                        }
                        let id = obj
                            .get("_id")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| DbError::Invalid("record missing _id".into()))?
                            .to_string();
                        sanitize_record_payload(&mut rec, &known);
                        update_record(db, application_id, &id, rec, None)?;
                    }
                }
            }
            "remove_field" => {
                // Drop removed / legacy undeclared keys so strict validate_record accepts round-trips.
                let records = query_records(
                    db,
                    application_id,
                    model_id,
                    MAX_GENERATED_RECORDS_PER_QUERY,
                )?;
                for mut rec in records {
                    if let Some(obj) = rec.as_object_mut() {
                        let id = obj
                            .get("_id")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| DbError::Invalid("record missing _id".into()))?
                            .to_string();
                        sanitize_record_payload(&mut rec, &known);
                        update_record(db, application_id, &id, rec, None)?;
                    }
                }
            }
            _ => {}
        }
        Ok(impact)
    })();

    match res {
        Ok(impact) => {
            db.conn()
                .execute_batch(&format!("RELEASE SAVEPOINT {sp_name};"))?;
            Ok(impact)
        }
        Err(err) => {
            let _ = db
                .conn()
                .execute_batch(&format!("ROLLBACK TO SAVEPOINT {sp_name};"));
            let _ = db
                .conn()
                .execute_batch(&format!("RELEASE SAVEPOINT {sp_name};"));
            Err(err)
        }
    }
}

pub fn apply_kernel_operations(db: &mut Database, ops: &[AppOperation]) -> DbResult<()> {
    for op in ops {
        match op.op_type.as_str() {
            "data.model_upsert" => {
                let app = op
                    .target
                    .application_id
                    .as_deref()
                    .or_else(|| op.payload.get("applicationId").and_then(|v| v.as_str()))
                    .ok_or_else(|| DbError::Invalid("applicationId required".into()))?;
                assert_can_write_data(db, app)?;
                let def: DataModelDefinition = serde_json::from_value(
                    op.payload
                        .get("model")
                        .cloned()
                        .unwrap_or_else(|| op.payload.clone()),
                )
                .map_err(|e| DbError::Invalid(e.to_string()))?;
                upsert_model(db, app, def)?;
            }
            "data.record_create" => {
                let app = op
                    .target
                    .application_id
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("applicationId required".into()))?;
                let model = op
                    .target
                    .model_id
                    .as_deref()
                    .or_else(|| op.payload.get("modelId").and_then(|v| v.as_str()))
                    .ok_or_else(|| DbError::Invalid("modelId required".into()))?;
                let data = op
                    .payload
                    .get("data")
                    .cloned()
                    .unwrap_or_else(|| op.payload.clone());
                create_record(db, app, model, data)?;
            }
            "data.record_update" => {
                let app = op
                    .target
                    .application_id
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("applicationId required".into()))?;
                let rid = op
                    .payload
                    .get("recordId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| DbError::Invalid("recordId required".into()))?;
                let data = op
                    .payload
                    .get("data")
                    .cloned()
                    .ok_or_else(|| DbError::Invalid("data required".into()))?;
                let base = op.payload.get("baseVersion").and_then(|v| v.as_i64());
                update_record(db, app, rid, data, base)?;
            }
            "data.record_delete" => {
                let app = op
                    .target
                    .application_id
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("applicationId required".into()))?;
                let rid = op
                    .payload
                    .get("recordId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| DbError::Invalid("recordId required".into()))?;
                delete_record(db, app, rid)?;
            }
            "data.migrate" => {
                let app = op
                    .target
                    .application_id
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("applicationId required".into()))?;
                let model = op
                    .target
                    .model_id
                    .as_deref()
                    .or_else(|| op.payload.get("modelId").and_then(|v| v.as_str()))
                    .ok_or_else(|| DbError::Invalid("modelId required".into()))?;
                let mig: DataMigration = serde_json::from_value(
                    op.payload
                        .get("migration")
                        .cloned()
                        .unwrap_or_else(|| op.payload.clone()),
                )
                .map_err(|e| DbError::Invalid(e.to_string()))?;
                let confirm = op
                    .payload
                    .get("confirmDestructive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                apply_migration(db, app, model, &mig, confirm)?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn habit_model() -> DataModelDefinition {
        DataModelDefinition {
            model_id: "habit".into(),
            display_name: "Habit".into(),
            schema_version: 1,
            fields: vec![
                DataField {
                    field_id: "name".into(),
                    field_type: "text".into(),
                    required: true,
                    default: None,
                    enum_values: None,
                },
                DataField {
                    field_id: "frequency".into(),
                    field_type: "enum".into(),
                    required: true,
                    default: Some(json!("daily")),
                    enum_values: Some(vec!["daily".into(), "weekly".into()]),
                },
                DataField {
                    field_id: "completionDate".into(),
                    field_type: "date".into(),
                    required: false,
                    default: None,
                    enum_values: None,
                },
                DataField {
                    field_id: "notes".into(),
                    field_type: "long_text".into(),
                    required: false,
                    default: None,
                    enum_values: None,
                },
            ],
        }
    }

    #[test]
    fn validates_habit_model() {
        assert!(validate_model(&habit_model()).is_ok());
    }

    #[test]
    fn rejects_sql_injection_model_id() {
        let mut m = habit_model();
        m.model_id = "x; DROP TABLE tools--".into();
        assert!(validate_model(&m).is_err());
    }

    #[test]
    fn rejects_bad_enum() {
        let m = habit_model();
        assert!(validate_record(&m, &json!({"name":"x","frequency":"hourly"})).is_err());
    }

    #[test]
    fn get_model_missing_is_typed_for_agent_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Database::open_path(&dir.path().join("t.db")).unwrap();
        let err = get_model(&db, "app-missing", "tasks").expect_err("missing model");
        let msg = err.to_string();
        assert!(
            msg.contains("model_missing"),
            "expected typed model_missing, got {msg}"
        );
        assert!(
            msg.contains("data.model_upsert"),
            "error should tell agent to upsert first: {msg}"
        );
    }

    #[test]
    fn record_update_denies_cross_application_ownership() {
        use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
        use std::collections::HashMap;

        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::db::Database::open_path(&dir.path().join("own.db")).unwrap();

        for app in ["app-a", "app-b"] {
            upsert_manifest(
                &mut db,
                ApplicationManifest {
                    schema_version: "1".into(),
                    application_id: app.into(),
                    instance_id: format!("inst-{app}"),
                    name: app.into(),
                    description: String::new(),
                    version: 1,
                    surfaces: vec![],
                    routes: vec![],
                    data_models: vec![],
                    settings: vec![],
                    capabilities: vec!["coreside.core".into()],
                    permissions: vec!["local_data.write".into(), "local_data.read".into()],
                    events: vec![],
                    tests: vec![],
                    search_keywords: vec![],
                    tags: vec![],
                    agent_description: String::new(),
                    project_id: None,
                    conversation_id: None,
                    organization_id: None,
                    ownership: None,
                    application_action_access: vec!["local_data.write".into()],
                    surface_action_access: HashMap::new(),
                    component_action_access: HashMap::new(),
                    action_descriptor_hashes: HashMap::new(),
                },
            )
            .unwrap();
            crate::application_kernel::permissions::grant_permission(
                &mut db,
                app,
                "local_data.write",
                json!({}),
                "test",
            )
            .unwrap();
            upsert_model(&mut db, app, habit_model()).unwrap();
        }

        let rec_id = create_record(
            &mut db,
            "app-a",
            "habit",
            json!({"name": "Water", "frequency": "daily"}),
        )
        .unwrap();

        let err = update_record(
            &mut db,
            "app-b",
            &rec_id,
            json!({"name": "Hacked", "frequency": "daily"}),
            None,
        )
        .expect_err("cross-app update must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("not found") || msg.contains("NotFound") || msg.contains("record"),
            "expected ownership denial, got {msg}"
        );

        let still = query_records(&db, "app-a", "habit", 10).unwrap();
        assert_eq!(still.len(), 1);
        assert_eq!(still[0]["name"], "Water");
    }

    #[test]
    fn record_delete_denies_cross_application_ownership() {
        use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
        use std::collections::HashMap;

        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::db::Database::open_path(&dir.path().join("del.db")).unwrap();

        for app in ["app-a", "app-b"] {
            upsert_manifest(
                &mut db,
                ApplicationManifest {
                    schema_version: "1".into(),
                    application_id: app.into(),
                    instance_id: format!("inst-{app}"),
                    name: app.into(),
                    description: String::new(),
                    version: 1,
                    surfaces: vec![],
                    routes: vec![],
                    data_models: vec![],
                    settings: vec![],
                    capabilities: vec!["coreside.core".into()],
                    permissions: vec!["local_data.write".into(), "local_data.read".into()],
                    events: vec![],
                    tests: vec![],
                    search_keywords: vec![],
                    tags: vec![],
                    agent_description: String::new(),
                    project_id: None,
                    conversation_id: None,
                    organization_id: None,
                    ownership: None,
                    application_action_access: vec!["local_data.write".into()],
                    surface_action_access: HashMap::new(),
                    component_action_access: HashMap::new(),
                    action_descriptor_hashes: HashMap::new(),
                },
            )
            .unwrap();
            crate::application_kernel::permissions::grant_permission(
                &mut db,
                app,
                "local_data.write",
                json!({}),
                "test",
            )
            .unwrap();
            upsert_model(&mut db, app, habit_model()).unwrap();
        }

        let rec_id = create_record(
            &mut db,
            "app-a",
            "habit",
            json!({"name": "Water", "frequency": "daily"}),
        )
        .unwrap();

        let err = delete_record(&mut db, "app-b", &rec_id).expect_err("cross-app delete");
        let msg = err.to_string();
        assert!(
            msg.contains("not found") || msg.contains("NotFound") || msg.contains("record"),
            "expected ownership denial, got {msg}"
        );

        let still = query_records(&db, "app-a", "habit", 10).unwrap();
        assert_eq!(still.len(), 1);
    }

    /// Vertical slice: Task model CRUD with schema evolution and restart survival.
    #[test]
    fn task_tracker_data_vertical_slice_crud_migrate_survive() {
        use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
        use std::collections::HashMap;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tasks.db");
        let mut db = crate::db::Database::open_path(&db_path).unwrap();

        upsert_manifest(
            &mut db,
            ApplicationManifest {
                schema_version: "1".into(),
                application_id: "app-tasks".into(),
                instance_id: "inst-tasks".into(),
                name: "Task Tracker".into(),
                description: String::new(),
                version: 1,
                surfaces: vec![],
                routes: vec![],
                data_models: vec![],
                settings: vec![],
                capabilities: vec!["coreside.core".into()],
                permissions: vec!["local_data.write".into(), "local_data.read".into()],
                events: vec![],
                tests: vec![],
                search_keywords: vec![],
                tags: vec![],
                agent_description: String::new(),
                project_id: None,
                conversation_id: None,
                organization_id: None,
                ownership: None,
                application_action_access: vec!["local_data.write".into()],
                surface_action_access: HashMap::new(),
                component_action_access: HashMap::new(),
                action_descriptor_hashes: HashMap::new(),
            },
        )
        .unwrap();
        crate::application_kernel::permissions::grant_permission(
            &mut db,
            "app-tasks",
            "local_data.write",
            json!({}),
            "test",
        )
        .unwrap();

        let v1 = DataModelDefinition {
            model_id: "Task".into(),
            display_name: "Task".into(),
            schema_version: 1,
            fields: vec![
                DataField {
                    field_id: "title".into(),
                    field_type: "text".into(),
                    required: true,
                    default: None,
                    enum_values: None,
                },
                DataField {
                    field_id: "status".into(),
                    field_type: "enum".into(),
                    required: true,
                    default: Some(json!("pending")),
                    enum_values: Some(vec!["pending".into(), "completed".into()]),
                },
            ],
        };
        upsert_model(&mut db, "app-tasks", v1).unwrap();

        let id = create_record(
            &mut db,
            "app-tasks",
            "Task",
            json!({"title": "Buy groceries", "status": "pending"}),
        )
        .unwrap();
        // v1 update without new fields
        update_record(
            &mut db,
            "app-tasks",
            &id,
            json!({"title": "Buy groceries", "status": "completed"}),
            None,
        )
        .unwrap();

        let v2 = DataModelDefinition {
            model_id: "Task".into(),
            display_name: "Task".into(),
            schema_version: 2,
            fields: vec![
                DataField {
                    field_id: "title".into(),
                    field_type: "text".into(),
                    required: true,
                    default: None,
                    enum_values: None,
                },
                DataField {
                    field_id: "description".into(),
                    field_type: "long_text".into(),
                    required: false,
                    default: None,
                    enum_values: None,
                },
                DataField {
                    field_id: "status".into(),
                    field_type: "enum".into(),
                    required: true,
                    default: Some(json!("pending")),
                    enum_values: Some(vec!["pending".into(), "completed".into()]),
                },
                DataField {
                    field_id: "priority".into(),
                    field_type: "enum".into(),
                    required: false,
                    default: Some(json!("normal")),
                    enum_values: Some(vec![
                        "low".into(),
                        "normal".into(),
                        "high".into(),
                    ]),
                },
                DataField {
                    field_id: "due_date".into(),
                    field_type: "date".into(),
                    required: false,
                    default: None,
                    enum_values: None,
                },
                DataField {
                    field_id: "tags".into(),
                    field_type: "text".into(),
                    required: false,
                    default: None,
                    enum_values: None,
                },
            ],
        };
        upsert_model(&mut db, "app-tasks", v2).unwrap();

        update_record(
            &mut db,
            "app-tasks",
            &id,
            json!({
                "title": "Buy groceries",
                "status": "pending",
                "priority": "high",
                "tags": "errands"
            }),
            None,
        )
        .unwrap();

        let listed = query_records(&db, "app-tasks", "Task", 20).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["title"], "Buy groceries");
        assert_eq!(listed[0]["priority"], "high");

        drop(db);
        let db2 = crate::db::Database::open_path(&db_path).unwrap();
        let after = query_records(&db2, "app-tasks", "Task", 20).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0]["title"], "Buy groceries");
        assert_eq!(after[0]["_id"], id);

        let mut db2 = crate::db::Database::open_path(&db_path).unwrap();
        delete_record(&mut db2, "app-tasks", &id).unwrap();
        assert!(query_records(&db2, "app-tasks", "Task", 20)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn validate_record_rejects_unknown_fields() {
        let m = habit_model();
        let err = validate_record(
            &m,
            &json!({"name": "x", "frequency": "daily", "secretExtra": true}),
        )
        .expect_err("unknown fields must fail");
        assert!(
            err.contains("unknown field"),
            "expected unknown field error, got {err}"
        );
    }

    #[test]
    fn fork_application_isolates_records_from_source() {
        use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
        use std::collections::HashMap;

        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::db::Database::open_path(&dir.path().join("fork.db")).unwrap();

        upsert_manifest(
            &mut db,
            ApplicationManifest {
                schema_version: "1".into(),
                application_id: "app-src".into(),
                instance_id: "inst-src".into(),
                name: "Source".into(),
                description: String::new(),
                version: 1,
                surfaces: vec![],
                routes: vec![],
                data_models: vec![],
                settings: vec![],
                capabilities: vec!["coreside.core".into()],
                permissions: vec!["local_data.write".into(), "local_data.read".into()],
                events: vec![],
                tests: vec![],
                search_keywords: vec![],
                tags: vec![],
                agent_description: String::new(),
                project_id: None,
                conversation_id: Some("conv-src".into()),
                organization_id: None,
                ownership: None,
                application_action_access: vec!["local_data.write".into()],
                surface_action_access: HashMap::new(),
                component_action_access: HashMap::new(),
                action_descriptor_hashes: HashMap::new(),
            },
        )
        .unwrap();
        crate::application_kernel::permissions::grant_permission(
            &mut db,
            "app-src",
            "local_data.write",
            json!({}),
            "test",
        )
        .unwrap();
        upsert_model(&mut db, "app-src", habit_model()).unwrap();
        create_record(
            &mut db,
            "app-src",
            "habit",
            json!({"name": "Water", "frequency": "daily"}),
        )
        .unwrap();

        let forked = fork_application_for_branch(&mut db, "app-src", "conv-branch").unwrap();
        assert_ne!(forked, "app-src");

        create_record(
            &mut db,
            &forked,
            "habit",
            json!({"name": "Branch Only", "frequency": "weekly"}),
        )
        .unwrap();

        let src = query_records(&db, "app-src", "habit", 20).unwrap();
        let br = query_records(&db, &forked, "habit", 20).unwrap();
        assert_eq!(src.len(), 1);
        assert_eq!(src[0]["name"], "Water");
        assert_eq!(br.len(), 2);
        assert!(br.iter().any(|r| r["name"] == "Branch Only"));
        assert!(br.iter().any(|r| r["name"] == "Water"));
    }

    fn seed_writable_habit_app(db: &mut crate::db::Database, app_id: &str) {
        use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
        use std::collections::HashMap;

        upsert_manifest(
            db,
            ApplicationManifest {
                schema_version: "1".into(),
                application_id: app_id.into(),
                instance_id: format!("inst-{app_id}"),
                name: app_id.into(),
                description: String::new(),
                version: 1,
                surfaces: vec![],
                routes: vec![],
                data_models: vec![],
                settings: vec![],
                capabilities: vec!["coreside.core".into()],
                permissions: vec!["local_data.write".into(), "local_data.read".into()],
                events: vec![],
                tests: vec![],
                search_keywords: vec![],
                tags: vec![],
                agent_description: String::new(),
                project_id: None,
                conversation_id: None,
                organization_id: None,
                ownership: None,
                application_action_access: vec!["local_data.write".into()],
                surface_action_access: HashMap::new(),
                component_action_access: HashMap::new(),
                action_descriptor_hashes: HashMap::new(),
            },
        )
        .unwrap();
        crate::application_kernel::permissions::grant_permission(
            db,
            app_id,
            "local_data.write",
            json!({}),
            "test",
        )
        .unwrap();
        upsert_model(db, app_id, habit_model()).unwrap();
    }

    #[test]
    fn suspended_application_cannot_mutate_generated_records() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::db::Database::open_path(&dir.path().join("sus.db")).unwrap();
        seed_writable_habit_app(&mut db, "app-sus");

        let id = create_record(
            &mut db,
            "app-sus",
            "habit",
            json!({"name": "Water", "frequency": "daily"}),
        )
        .unwrap();

        db.conn()
            .execute(
                "UPDATE application_manifests SET lifecycle_state = 'suspended', health_state = 'suspended'
                 WHERE application_id = ?1",
                ["app-sus"],
            )
            .unwrap();

        let create_err = create_record(
            &mut db,
            "app-sus",
            "habit",
            json!({"name": "Nope", "frequency": "daily"}),
        )
        .expect_err("suspended create");
        assert!(
            create_err.to_string().contains("suspended"),
            "expected suspended denial, got {create_err}"
        );

        let update_err = update_record(
            &mut db,
            "app-sus",
            &id,
            json!({"name": "Hacked", "frequency": "daily"}),
            None,
        )
        .expect_err("suspended update");
        assert!(
            update_err.to_string().contains("suspended"),
            "expected suspended denial, got {update_err}"
        );

        let delete_err = delete_record(&mut db, "app-sus", &id).expect_err("suspended delete");
        assert!(
            delete_err.to_string().contains("suspended"),
            "expected suspended denial, got {delete_err}"
        );

        let still = query_records(&db, "app-sus", "habit", 10).unwrap();
        assert_eq!(still.len(), 1);
        assert_eq!(still[0]["name"], "Water");
    }

    #[test]
    fn create_record_rejects_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::db::Database::open_path(&dir.path().join("unk.db")).unwrap();
        seed_writable_habit_app(&mut db, "app-unk");

        let err = create_record(
            &mut db,
            "app-unk",
            "habit",
            json!({"name": "x", "frequency": "daily", "secretExtra": true}),
        )
        .expect_err("unknown field on create");
        assert!(
            err.to_string().contains("unknown field"),
            "expected unknown field error, got {err}"
        );
        assert!(query_records(&db, "app-unk", "habit", 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn forked_branch_update_does_not_mutate_source_records() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::db::Database::open_path(&dir.path().join("iso.db")).unwrap();
        seed_writable_habit_app(&mut db, "app-src");

        create_record(
            &mut db,
            "app-src",
            "habit",
            json!({"name": "Water", "frequency": "daily"}),
        )
        .unwrap();

        let forked = fork_application_for_branch(&mut db, "app-src", "conv-branch").unwrap();
        let br = query_records(&db, &forked, "habit", 20).unwrap();
        let branch_id = br[0]["_id"].as_str().unwrap().to_string();

        update_record(
            &mut db,
            &forked,
            &branch_id,
            json!({"name": "Branch Edited", "frequency": "weekly"}),
            None,
        )
        .unwrap();

        let src_after_update = query_records(&db, "app-src", "habit", 20).unwrap();
        assert_eq!(src_after_update.len(), 1);
        assert_eq!(src_after_update[0]["name"], "Water");
        assert_eq!(src_after_update[0]["frequency"], "daily");

        delete_record(&mut db, &forked, &branch_id).unwrap();
        let src = query_records(&db, "app-src", "habit", 20).unwrap();
        assert_eq!(src.len(), 1);
        assert_eq!(src[0]["name"], "Water");
        assert!(query_records(&db, &forked, "habit", 20)
            .unwrap()
            .is_empty());
    }
}
