//! Safe generated data models — declarative schemas compiled to namespaced CRUD.
//! No arbitrary SQL from the agent.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use crate::runtime_v2::limits::MAX_GENERATED_RECORDS_PER_QUERY;
use crate::runtime_v2::operations::AppOperation;
use rusqlite::params;

use super::permissions::assert_can_write_data;

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
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("model {application_id}/{model_id}"))
            }
            other => DbError::Sqlite(other),
        })?;
    serde_json::from_str(&json).map_err(|e| DbError::Invalid(e.to_string()))
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
    let (model_id, version): (String, i64) = db.conn().query_row(
        "SELECT model_id, record_version FROM generated_data_records
         WHERE id = ?1 AND application_id = ?2",
        params![record_id, application_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
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
    db.conn().execute(
        "DELETE FROM generated_data_records WHERE id = ?1 AND application_id = ?2",
        params![record_id, application_id],
    )?;
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
        let mut data: Value = serde_json::from_str(&data_s).unwrap_or(json!({}));
        if let Some(obj) = data.as_object_mut() {
            obj.insert("_id".into(), json!(id));
            obj.insert("_version".into(), json!(version));
            obj.insert("_createdAt".into(), json!(created));
            obj.insert("_updatedAt".into(), json!(updated));
        }
        Ok(data)
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
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
            def.fields.push(DataField {
                field_id: fid.into(),
                field_type: ftype.into(),
                required: migration.required.unwrap_or(false),
                default: migration.default.clone(),
                enum_values: None,
            });
            format!("Adds field {fid} to model {model_id}")
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
            for f in &mut def.fields {
                if f.field_id == old {
                    f.field_id = new.into();
                }
            }
            // Rewrite records
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
                    obj.remove("_id");
                    obj.remove("_version");
                    obj.remove("_createdAt");
                    obj.remove("_updatedAt");
                    update_record(db, application_id, &id, json!(obj), None)?;
                }
            }
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
    upsert_model(db, application_id, def)?;
    // Apply defaults to existing records for add_field
    if migration.migration_type == "add_field" {
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
                        // A record without `_id` cannot be rewritten; skip it
                        // rather than panicking part-way through a migration.
                        let Some(id) = obj.get("_id").and_then(|v| v.as_str()).map(str::to_string)
                        else {
                            tracing::warn!(
                                application_id,
                                model_id,
                                "skipped add_field backfill for a record with no id"
                            );
                            continue;
                        };
                        obj.insert(fid.clone(), default.clone());
                        obj.remove("_id");
                        obj.remove("_version");
                        obj.remove("_createdAt");
                        obj.remove("_updatedAt");
                        let _ = update_record(db, application_id, &id, json!(obj), None);
                    }
                }
            }
        }
    }
    Ok(impact)
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
}
