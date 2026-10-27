//! Bounded authoritative queries for generated application records.
//!
//! No raw SQL from the model. Filters/sort/search are validated against the
//! declared data model and executed with bound parameters.

use rusqlite::{params_from_iter, types::Value as SqlValue};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::db::{Database, DbError, DbResult};
use crate::runtime_v2::limits::MAX_GENERATED_RECORDS_PER_QUERY;

use super::data::{get_model, is_valid_iso_date, DataModelDefinition};

pub const MAX_QUERY_FILTERS: usize = 8;
pub const MAX_QUERY_AGGREGATES: usize = 16;
pub const MAX_SEARCH_LEN: usize = 200;
pub const MAX_FILTER_VALUE_LEN: usize = 256;
pub const MAX_ENUM_IN_VALUES: usize = 16;
pub const MAX_QUERY_OFFSET: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordQuery {
    pub model_id: String,
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
    #[serde(default)]
    pub filters: Vec<QueryFilter>,
    #[serde(default)]
    pub sort: Option<QuerySort>,
    #[serde(default)]
    pub search: Option<String>,
    /// When empty and search is set, all text-like model fields are searched.
    #[serde(default)]
    pub search_fields: Vec<String>,
    #[serde(default)]
    pub aggregates: Vec<AggregateSpec>,
}

impl Default for RecordQuery {
    fn default() -> Self {
        Self {
            model_id: String::new(),
            limit: default_limit(),
            offset: 0,
            filters: Vec::new(),
            sort: None,
            search: None,
            search_fields: Vec::new(),
            aggregates: Vec::new(),
        }
    }
}

fn default_limit() -> u64 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueryFilter {
    pub field: String,
    /// eq | neq | lt | lte | gt | gte | in | not_in | contains
    pub op: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuerySort {
    pub field: String,
    /// asc | desc — default desc when omitted downstream
    #[serde(default = "default_sort_dir")]
    pub direction: String,
}

fn default_sort_dir() -> String {
    "desc".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AggregateSpec {
    pub key: String,
    /// count | countWhere | countOverdue
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_equals: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_status: Option<String>,
    /// YYYY-MM-DD as-of date for overdue (defaults to UTC calendar date).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub records: Vec<Value>,
    /// Compatibility: number of records in this page (same as returnedCount).
    pub count: usize,
    pub returned_count: usize,
    pub total_count: usize,
    pub has_more: bool,
    pub offset: usize,
    pub limit: usize,
    pub model_id: String,
    pub revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregates: Option<Map<String, Value>>,
}

impl QueryResult {
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({}))
    }
}

/// SQL for bumping the monotonic per-model revision (same connection/txn as the write).
pub(crate) fn bump_model_revision_sql(
    conn: &rusqlite::Connection,
    application_id: &str,
    model_id: &str,
) -> DbResult<()> {
    conn.execute(
        "INSERT INTO generated_data_revisions (application_id, model_id, revision)
         VALUES (?1, ?2, 1)
         ON CONFLICT(application_id, model_id) DO UPDATE SET revision = revision + 1",
        rusqlite::params![application_id, model_id],
    )?;
    Ok(())
}

/// Bump the monotonic per-model revision after a mutation (standalone helper).
pub fn bump_model_revision(
    db: &mut Database,
    application_id: &str,
    model_id: &str,
) -> DbResult<i64> {
    bump_model_revision_sql(db.conn(), application_id, model_id)?;
    model_revision(db, application_id, model_id)
}

pub fn model_revision(db: &Database, application_id: &str, model_id: &str) -> DbResult<i64> {
    match db.conn().query_row(
        "SELECT revision FROM generated_data_revisions
         WHERE application_id = ?1 AND model_id = ?2",
        rusqlite::params![application_id, model_id],
        |row| row.get(0),
    ) {
        Ok(rev) => Ok(rev),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

pub fn execute_query(
    db: &Database,
    application_id: &str,
    query: &RecordQuery,
) -> DbResult<QueryResult> {
    let needs_schema = !query.filters.is_empty()
        || query.sort.is_some()
        || query.search.as_ref().is_some_and(|s| !s.trim().is_empty())
        || !query.aggregates.is_empty()
        || !query.search_fields.is_empty();

    let def = match get_model(db, application_id, &query.model_id) {
        Ok(d) => d,
        Err(DbError::NotFound(_)) if !needs_schema => {
            // Compat with legacy query_records: unknown model → empty page, not an error.
            let limit = (query.limit as usize).clamp(1, MAX_GENERATED_RECORDS_PER_QUERY);
            let offset = query.offset as usize;
            return Ok(QueryResult {
                records: vec![],
                count: 0,
                returned_count: 0,
                total_count: 0,
                has_more: false,
                offset,
                limit,
                model_id: query.model_id.clone(),
                revision: model_revision(db, application_id, &query.model_id)?,
                aggregates: None,
            });
        }
        Err(e) => return Err(e),
    };
    let plan = plan_query(&def, query)?;
    // Revision + count + page + aggregates share one deferred snapshot so another
    // connection cannot interleave commits between those reads. Same-process
    // callers are already serialized by AppState's DB mutex; this covers WAL
    // writers on a second connection (inspector / future multi-conn paths).
    with_read_snapshot(db, || {
        let revision = model_revision(db, application_id, &query.model_id)?;
        let total_count = count_matching(db, application_id, &query.model_id, &plan)?;
        let records = fetch_page(db, application_id, &query.model_id, &plan)?;
        let returned = records.len();
        let has_more = plan.offset + returned < total_count;

        let aggregates = if plan.aggregates.is_empty() {
            None
        } else {
            Some(compute_aggregates(
                db,
                application_id,
                &query.model_id,
                &plan,
                &def,
            )?)
        };

        Ok(QueryResult {
            count: returned,
            returned_count: returned,
            total_count,
            has_more,
            offset: plan.offset,
            limit: plan.limit,
            model_id: query.model_id.clone(),
            revision,
            records,
            aggregates,
        })
    })
}

/// Open a deferred read transaction when the connection is idle so multi-statement
/// queries observe one committed snapshot. Nested callers reuse the outer txn.
fn with_read_snapshot<T, F>(db: &Database, f: F) -> DbResult<T>
where
    F: FnOnce() -> DbResult<T>,
{
    let conn = db.conn();
    let started = if conn.is_autocommit() {
        conn.execute_batch("BEGIN DEFERRED")?;
        true
    } else {
        false
    };
    let result = f();
    if started {
        match &result {
            Ok(_) => {
                if let Err(e) = conn.execute_batch("COMMIT") {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(DbError::Sqlite(e));
                }
            }
            Err(_) => {
                let _ = conn.execute_batch("ROLLBACK");
            }
        }
    }
    result
}

struct PlannedQuery {
    limit: usize,
    offset: usize,
    where_sql: String,
    where_params: Vec<SqlValue>,
    order_sql: String,
    aggregates: Vec<PlannedAggregate>,
    /// Filters already applied in WHERE; aggregates that need a full scan still use these.
    base_where_sql: String,
    base_where_params: Vec<SqlValue>,
}

struct PlannedAggregate {
    key: String,
    kind: PlannedAggKind,
}

enum PlannedAggKind {
    Count,
    CountWhere {
        field: String,
        field_type: String,
        equals: Option<String>,
        not_equals: Option<String>,
    },
    CountOverdue {
        field: String,
        status_field: Option<String>,
        exclude_status: Option<String>,
        as_of: String,
    },
}

fn plan_query(def: &DataModelDefinition, query: &RecordQuery) -> DbResult<PlannedQuery> {
    if query.model_id.trim().is_empty() {
        return Err(DbError::Invalid("modelId is required".into()));
    }
    if query.filters.len() > MAX_QUERY_FILTERS {
        return Err(DbError::Invalid(format!(
            "at most {MAX_QUERY_FILTERS} filters allowed"
        )));
    }
    if query.aggregates.len() > MAX_QUERY_AGGREGATES {
        return Err(DbError::Invalid(format!(
            "at most {MAX_QUERY_AGGREGATES} aggregates allowed"
        )));
    }
    let limit = (query.limit as usize).clamp(1, MAX_GENERATED_RECORDS_PER_QUERY);
    if query.limit == 0 {
        return Err(DbError::Invalid("limit must be at least 1".into()));
    }
    if query.limit as usize > MAX_GENERATED_RECORDS_PER_QUERY {
        return Err(DbError::Invalid(format!(
            "limit exceeds maximum of {MAX_GENERATED_RECORDS_PER_QUERY}"
        )));
    }
    let offset = query.offset as usize;
    if offset > MAX_QUERY_OFFSET {
        return Err(DbError::Invalid(format!(
            "offset exceeds maximum of {MAX_QUERY_OFFSET}"
        )));
    }

    // Predicates beyond application_id / model_id; params are bound at execute time.
    let mut extra_parts: Vec<String> = Vec::new();
    let mut extra_params: Vec<SqlValue> = Vec::new();

    for filter in &query.filters {
        let field = validate_field_id(def, &filter.field)?;
        let field_type = field_type_of(def, &field);
        // Lowercase once so camelCase ops like notEquals match after normalize.
        let op = filter.op.trim().to_lowercase();
        assert_op_for_field_type(field_type, &op)?;
        let expr = json_extract_expr(&field);
        match op.as_str() {
            "eq" | "equals" => {
                extra_parts.push(format!("{expr} = ?"));
                extra_params.push(bind_typed_value(field_type, &filter.value, "filter value")?);
            }
            "neq" | "notequals" | "not_equals" => {
                extra_parts.push(format!("({expr} IS NULL OR {expr} != ?)"));
                extra_params.push(bind_typed_value(field_type, &filter.value, "filter value")?);
            }
            "lt" | "lte" | "gt" | "gte" => {
                let sql_op = match op.as_str() {
                    "lt" => "<",
                    "lte" => "<=",
                    "gt" => ">",
                    _ => ">=",
                };
                extra_parts.push(format!("{expr} {sql_op} ?"));
                extra_params.push(bind_typed_value(field_type, &filter.value, "filter value")?);
            }
            "contains" => {
                let s = scalar_string(&filter.value, "filter value")?;
                if s.chars().count() > MAX_FILTER_VALUE_LEN {
                    return Err(DbError::Invalid("filter value too long".into()));
                }
                extra_parts.push(format!("{expr} LIKE ? ESCAPE '\\'"));
                extra_params.push(SqlValue::Text(format!("%{}%", escape_like(&s))));
            }
            "in" => {
                let arr = filter
                    .value
                    .as_array()
                    .ok_or_else(|| DbError::Invalid("in filter requires an array".into()))?;
                if arr.is_empty() || arr.len() > MAX_ENUM_IN_VALUES {
                    return Err(DbError::Invalid(format!(
                        "in filter requires 1..{MAX_ENUM_IN_VALUES} values"
                    )));
                }
                let mut placeholders = Vec::new();
                for v in arr {
                    placeholders.push("?");
                    extra_params.push(bind_typed_value(field_type, v, "in value")?);
                }
                extra_parts.push(format!("{expr} IN ({})", placeholders.join(", ")));
            }
            "not_in" | "notin" | "not_in_ci" => {
                // Case-insensitive exclusion; missing/NULL field values count as matching
                // (treated as unfinished for status-style filters).
                let arr = filter
                    .value
                    .as_array()
                    .ok_or_else(|| DbError::Invalid("not_in filter requires an array".into()))?;
                if arr.is_empty() || arr.len() > MAX_ENUM_IN_VALUES {
                    return Err(DbError::Invalid(format!(
                        "not_in filter requires 1..{MAX_ENUM_IN_VALUES} values"
                    )));
                }
                let mut placeholders = Vec::new();
                for v in arr {
                    let s = scalar_string(v, "not_in value")?.to_ascii_lowercase();
                    if s.chars().count() > MAX_FILTER_VALUE_LEN {
                        return Err(DbError::Invalid("filter value too long".into()));
                    }
                    placeholders.push("?");
                    extra_params.push(SqlValue::Text(s));
                }
                extra_parts.push(format!(
                    "({expr} IS NULL OR LOWER(CAST({expr} AS TEXT)) NOT IN ({}))",
                    placeholders.join(", ")
                ));
            }
            _ => {
                return Err(DbError::Invalid(format!("unsupported filter op: {op}")));
            }
        }
    }

    if let Some(raw_search) = query.search.as_ref() {
        let q = raw_search.trim();
        if !q.is_empty() {
            if q.chars().count() > MAX_SEARCH_LEN {
                return Err(DbError::Invalid(format!(
                    "search exceeds maximum length of {MAX_SEARCH_LEN}"
                )));
            }
            let fields: Vec<String> = if query.search_fields.is_empty() {
                def.fields
                    .iter()
                    .filter(|f| {
                        matches!(
                            f.field_type.as_str(),
                            "text" | "long_text" | "enum" | "date" | "time" | "date_time"
                        )
                    })
                    .map(|f| f.field_id.clone())
                    .collect()
            } else {
                let mut out = Vec::new();
                for f in &query.search_fields {
                    out.push(validate_field_id(def, f)?);
                }
                out
            };
            if fields.is_empty() {
                return Err(DbError::Invalid(
                    "search requires at least one searchable field".into(),
                ));
            }
            let pattern = format!("%{}%", escape_like(q));
            let mut ors = Vec::new();
            for f in &fields {
                ors.push(format!("{} LIKE ? ESCAPE '\\'", json_extract_expr(f)));
                extra_params.push(SqlValue::Text(pattern.clone()));
            }
            extra_parts.push(format!("({})", ors.join(" OR ")));
        }
    }

    let order_sql = plan_order(def, query.sort.as_ref())?;

    let mut aggregates = Vec::new();
    let mut seen_agg_keys = std::collections::HashSet::new();
    for agg in &query.aggregates {
        let planned = plan_aggregate(def, agg)?;
        if !seen_agg_keys.insert(planned.key.clone()) {
            return Err(DbError::Invalid(format!(
                "duplicate aggregate key: {}",
                planned.key
            )));
        }
        aggregates.push(planned);
    }

    let base_where_sql = if extra_parts.is_empty() {
        String::new()
    } else {
        format!(" AND {}", extra_parts.join(" AND "))
    };

    Ok(PlannedQuery {
        limit,
        offset,
        where_sql: base_where_sql.clone(),
        where_params: extra_params.clone(),
        order_sql,
        aggregates,
        base_where_sql,
        base_where_params: extra_params,
    })
}

fn plan_order(def: &DataModelDefinition, sort: Option<&QuerySort>) -> DbResult<String> {
    // Stable total order: primary key + id DESC tie-breaker.
    let (field, dir) = match sort {
        None => ("_updatedAt".to_string(), "DESC".to_string()),
        Some(s) => {
            let d = match s.direction.trim().to_lowercase().as_str() {
                "asc" => "ASC".to_string(),
                "desc" => "DESC".to_string(),
                other => {
                    return Err(DbError::Invalid(format!(
                        "sort direction must be asc or desc, got {other}"
                    )));
                }
            };
            let f = s.field.trim();
            if f == "_updatedAt" || f == "updatedAt" || f == "_createdAt" || f == "createdAt" {
                (f.to_string(), d)
            } else {
                (validate_field_id(def, f)?, d)
            }
        }
    };

    let primary = match field.as_str() {
        "_updatedAt" | "updatedAt" => format!("updated_at {dir}"),
        "_createdAt" | "createdAt" => format!("created_at {dir}"),
        other => {
            let field_def = def
                .fields
                .iter()
                .find(|f| f.field_id == other)
                .ok_or_else(|| DbError::Invalid(format!("unknown sort field: {other}")))?;
            let expr = json_extract_expr(other);
            let id_lower = other.to_ascii_lowercase();
            let is_priority_named =
                matches!(id_lower.as_str(), "priority" | "urgency" | "importance");
            match field_def.field_type.as_str() {
                "integer" | "decimal" | "duration" => {
                    format!("CAST({expr} AS REAL) {dir}")
                }
                "enum" => {
                    // Preserve declared enum order when possible.
                    if let Some(values) = &field_def.enum_values {
                        let mut cases = String::from("CASE ");
                        for (i, v) in values.iter().enumerate() {
                            cases.push_str(&format!("WHEN {expr} = {} THEN {i} ", sql_quote(v)));
                        }
                        cases.push_str(&format!("ELSE {n} END {dir}", n = values.len()));
                        cases
                    } else if is_priority_named {
                        priority_text_rank_sql(&expr, &dir)
                    } else {
                        format!("{expr} {dir}")
                    }
                }
                "text" | "string" | "long_text" if is_priority_named => {
                    // Avoid lexicographic high < low < medium; map known ranks.
                    priority_text_rank_sql(&expr, &dir)
                }
                _ => format!("{expr} {dir}"),
            }
        }
    };

    Ok(format!("{primary}, id DESC"))
}

/// Rank common priority labels so DESC puts high before medium before low.
fn priority_text_rank_sql(expr: &str, dir: &str) -> String {
    format!(
        "CASE LOWER(CAST({expr} AS TEXT)) \
         WHEN 'critical' THEN 4 WHEN 'urgent' THEN 4 WHEN 'high' THEN 3 \
         WHEN 'medium' THEN 2 WHEN 'normal' THEN 2 WHEN 'low' THEN 1 \
         ELSE 0 END {dir}"
    )
}

fn plan_aggregate(def: &DataModelDefinition, agg: &AggregateSpec) -> DbResult<PlannedAggregate> {
    let key = agg.key.trim();
    if key.is_empty()
        || key.len() > 64
        || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(DbError::Invalid("invalid aggregate key".into()));
    }
    let kind = match agg.op.trim() {
        "count" => PlannedAggKind::Count,
        "countWhere" => {
            let field = validate_field_id(
                def,
                agg.field
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("countWhere requires field".into()))?,
            )?;
            if agg.equals.is_none() && agg.not_equals.is_none() {
                return Err(DbError::Invalid(
                    "countWhere requires equals or notEquals".into(),
                ));
            }
            let field_type = field_type_of(def, &field).to_string();
            PlannedAggKind::CountWhere {
                field,
                field_type,
                equals: agg.equals.clone(),
                not_equals: agg.not_equals.clone(),
            }
        }
        "countOverdue" => {
            let field = validate_field_id(
                def,
                agg.field
                    .as_deref()
                    .ok_or_else(|| DbError::Invalid("countOverdue requires field".into()))?,
            )?;
            let status_field = match &agg.status_field {
                Some(s) => Some(validate_field_id(def, s)?),
                None => None,
            };
            let as_of = match &agg.as_of {
                Some(d) => {
                    if !is_valid_iso_date(d) {
                        return Err(DbError::Invalid(
                            "asOf must be a valid YYYY-MM-DD calendar date".into(),
                        ));
                    }
                    d.clone()
                }
                None => chrono_today_utc(),
            };
            PlannedAggKind::CountOverdue {
                field,
                status_field,
                exclude_status: agg.exclude_status.clone(),
                as_of,
            }
        }
        other => {
            return Err(DbError::Invalid(format!(
                "unsupported aggregate op: {other}"
            )));
        }
    };
    Ok(PlannedAggregate {
        key: key.to_string(),
        kind,
    })
}

fn count_matching(
    db: &Database,
    application_id: &str,
    model_id: &str,
    plan: &PlannedQuery,
) -> DbResult<usize> {
    let sql = format!(
        "SELECT COUNT(*) FROM generated_data_records
         WHERE application_id = ?1 AND model_id = ?2{}",
        plan.where_sql
    );
    let mut params: Vec<SqlValue> = vec![
        SqlValue::Text(application_id.to_string()),
        SqlValue::Text(model_id.to_string()),
    ];
    params.extend(plan.where_params.iter().cloned());
    let n: i64 = db
        .conn()
        .query_row(&sql, params_from_iter(params), |row| row.get(0))?;
    Ok(n as usize)
}

fn fetch_page(
    db: &Database,
    application_id: &str,
    model_id: &str,
    plan: &PlannedQuery,
) -> DbResult<Vec<Value>> {
    let sql = format!(
        "SELECT id, record_version, data_json, created_at, updated_at
         FROM generated_data_records
         WHERE application_id = ?1 AND model_id = ?2{}
         ORDER BY {}
         LIMIT ? OFFSET ?",
        plan.where_sql, plan.order_sql
    );
    let mut params: Vec<SqlValue> = vec![
        SqlValue::Text(application_id.to_string()),
        SqlValue::Text(model_id.to_string()),
    ];
    params.extend(plan.where_params.iter().cloned());
    params.push(SqlValue::Integer(plan.limit as i64));
    params.push(SqlValue::Integer(plan.offset as i64));

    let mut stmt = db.conn().prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params), |row| {
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

fn compute_aggregates(
    db: &Database,
    application_id: &str,
    model_id: &str,
    plan: &PlannedQuery,
    _def: &DataModelDefinition,
) -> DbResult<Map<String, Value>> {
    let mut map = Map::new();
    for agg in &plan.aggregates {
        let n = match &agg.kind {
            PlannedAggKind::Count => count_matching(db, application_id, model_id, plan)?,
            PlannedAggKind::CountWhere {
                field,
                field_type,
                equals,
                not_equals,
            } => {
                let expr = json_extract_expr(field);
                let mut extra = plan.base_where_sql.clone();
                let mut params = plan.base_where_params.clone();
                if let Some(eq) = equals {
                    extra.push_str(&format!(" AND {expr} = ?"));
                    params.push(bind_typed_value(
                        field_type,
                        &Value::String(eq.clone()),
                        "aggregate equals",
                    )?);
                }
                if let Some(neq) = not_equals {
                    extra.push_str(&format!(" AND ({expr} IS NULL OR {expr} != ?)"));
                    params.push(bind_typed_value(
                        field_type,
                        &Value::String(neq.clone()),
                        "aggregate notEquals",
                    )?);
                }
                count_with_extra(db, application_id, model_id, &extra, &params)?
            }
            PlannedAggKind::CountOverdue {
                field,
                status_field,
                exclude_status,
                as_of,
            } => {
                let date_expr = json_extract_expr(field);
                let mut extra = plan.base_where_sql.clone();
                let mut params = plan.base_where_params.clone();
                // date-only compare: substr of stored value vs as_of
                extra.push_str(&format!(
                    " AND length(substr(trim({date_expr}), 1, 10)) = 10 \
                     AND substr(trim({date_expr}), 1, 10) < ?"
                ));
                params.push(SqlValue::Text(as_of.clone()));
                if let (Some(sf), Some(ex)) = (status_field, exclude_status) {
                    let sexpr = json_extract_expr(sf);
                    extra.push_str(&format!(" AND ({sexpr} IS NULL OR {sexpr} != ?)"));
                    params.push(SqlValue::Text(ex.clone()));
                }
                count_with_extra(db, application_id, model_id, &extra, &params)?
            }
        };
        map.insert(agg.key.clone(), json!(n));
    }
    Ok(map)
}

fn count_with_extra(
    db: &Database,
    application_id: &str,
    model_id: &str,
    extra_where: &str,
    extra_params: &[SqlValue],
) -> DbResult<usize> {
    let sql = format!(
        "SELECT COUNT(*) FROM generated_data_records
         WHERE application_id = ?1 AND model_id = ?2{extra_where}"
    );
    let mut params: Vec<SqlValue> = vec![
        SqlValue::Text(application_id.to_string()),
        SqlValue::Text(model_id.to_string()),
    ];
    params.extend(extra_params.iter().cloned());
    let n: i64 = db
        .conn()
        .query_row(&sql, params_from_iter(params), |row| row.get(0))?;
    Ok(n as usize)
}

fn validate_field_id(def: &DataModelDefinition, field: &str) -> DbResult<String> {
    let f = field.trim();
    if f.is_empty()
        || f.len() > 64
        || f.starts_with('_')
        || !f.chars().enumerate().all(|(i, c)| {
            if i == 0 {
                c.is_ascii_alphabetic()
            } else {
                c.is_ascii_alphanumeric() || c == '_'
            }
        })
    {
        return Err(DbError::Invalid(format!("invalid field: {field}")));
    }
    if !def.fields.iter().any(|x| x.field_id == f) {
        return Err(DbError::Invalid(format!("unknown field: {f}")));
    }
    Ok(f.to_string())
}

fn json_extract_expr(field: &str) -> String {
    // field already allowlisted — safe to interpolate into json path
    format!("json_extract(data_json, '$.{field}')")
}

fn field_type_of<'a>(def: &'a DataModelDefinition, field: &str) -> &'a str {
    def.fields
        .iter()
        .find(|f| f.field_id == field)
        .map(|f| f.field_type.as_str())
        .unwrap_or("text")
}

/// Reject operators that would have surprising semantics for the declared type.
fn assert_op_for_field_type(field_type: &str, op: &str) -> DbResult<()> {
    let allowed = match field_type {
        "boolean" => matches!(op, "eq" | "equals" | "neq" | "notequals" | "not_equals"),
        "integer" | "decimal" | "duration" => matches!(
            op,
            "eq" | "equals"
                | "neq"
                | "notequals"
                | "not_equals"
                | "lt"
                | "lte"
                | "gt"
                | "gte"
                | "in"
                | "not_in"
                | "notin"
                | "not_in_ci"
        ),
        "date" | "time" | "date_time" | "created_at" | "updated_at" => matches!(
            op,
            "eq" | "equals" | "neq" | "notequals" | "not_equals" | "lt" | "lte" | "gt" | "gte"
        ),
        "enum" => matches!(
            op,
            "eq" | "equals"
                | "neq"
                | "notequals"
                | "not_equals"
                | "in"
                | "not_in"
                | "notin"
                | "not_in_ci"
        ),
        "text" | "long_text" | "reference" | "media_ref" | "surface_ref" | "tool_ref" => matches!(
            op,
            "eq" | "equals"
                | "neq"
                | "notequals"
                | "not_equals"
                | "contains"
                | "in"
                | "not_in"
                | "notin"
                | "not_in_ci"
        ),
        // Object/array/json never coerce to text for filters.
        "json" | "object" | "array" => false,
        _ => matches!(
            op,
            "eq" | "equals" | "neq" | "notequals" | "not_equals" | "not_in" | "notin" | "not_in_ci"
        ),
    };
    if !allowed {
        return Err(DbError::Invalid(format!(
            "operator '{op}' is not allowed for field type '{field_type}'"
        )));
    }
    Ok(())
}

/// Bind a filter/aggregate value with the SQL affinity matching the declared field type.
/// json_extract returns JSON numbers/booleans as INTEGER/REAL/0|1 — Text binds never match.
fn bind_typed_value(field_type: &str, value: &Value, label: &str) -> DbResult<SqlValue> {
    if value.is_null() {
        return Err(DbError::Invalid(format!("{label} must not be null")));
    }
    match field_type {
        "integer" | "duration" => {
            let n = match value {
                Value::Number(n) => n
                    .as_i64()
                    .or_else(|| n.as_u64().and_then(|u| i64::try_from(u).ok()))
                    .ok_or_else(|| DbError::Invalid(format!("{label} must be integer")))?,
                Value::String(s) => s
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| DbError::Invalid(format!("{label} must be integer")))?,
                _ => {
                    return Err(DbError::Invalid(format!("{label} must be integer")));
                }
            };
            Ok(SqlValue::Integer(n))
        }
        "decimal" => {
            let n = match value {
                Value::Number(n) => n
                    .as_f64()
                    .ok_or_else(|| DbError::Invalid(format!("{label} must be number")))?,
                Value::String(s) => s
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| DbError::Invalid(format!("{label} must be number")))?,
                _ => {
                    return Err(DbError::Invalid(format!("{label} must be number")));
                }
            };
            if !n.is_finite() {
                return Err(DbError::Invalid(format!("{label} must be a finite number")));
            }
            // ponytail: REAL affinity matches json_extract decimals; money-safe
            // fixed-point storage is a future schema extension if needed.
            Ok(SqlValue::Real(n))
        }
        "boolean" => {
            let b = match value {
                Value::Bool(b) => *b,
                Value::Number(n) => match n.as_i64() {
                    Some(0) => false,
                    Some(1) => true,
                    _ => {
                        return Err(DbError::Invalid(format!("{label} must be boolean")));
                    }
                },
                Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                    "true" | "1" => true,
                    "false" | "0" => false,
                    _ => {
                        return Err(DbError::Invalid(format!("{label} must be boolean")));
                    }
                },
                _ => {
                    return Err(DbError::Invalid(format!("{label} must be boolean")));
                }
            };
            // json_extract emits 0/1 for JSON booleans.
            Ok(SqlValue::Integer(i64::from(b)))
        }
        "date" => {
            let s = scalar_string(value, label)?;
            if !is_valid_iso_date(&s) {
                return Err(DbError::Invalid(format!(
                    "{label} must be a valid YYYY-MM-DD calendar date"
                )));
            }
            Ok(SqlValue::Text(s))
        }
        _ => Ok(SqlValue::Text(scalar_string(value, label)?)),
    }
}

fn scalar_string(v: &Value, label: &str) -> DbResult<String> {
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => {
            return Err(DbError::Invalid(format!("{label} must not be null")));
        }
        _ => {
            return Err(DbError::Invalid(format!("{label} must be a scalar")));
        }
    };
    if s.chars().count() > MAX_FILTER_VALUE_LEN {
        return Err(DbError::Invalid(format!("{label} too long")));
    }
    Ok(s)
}

fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn sql_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn chrono_today_utc() -> String {
    // Date-only overdue uses the host local calendar date when asOf is omitted
    // at the handler layer; this fallback is UTC YYYY-MM-DD.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    // Civil date from Unix day count (Howard Hinnant algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Parse registered-action input into a RecordQuery (aliases supported).
///
/// Omitted optional fields use defaults. Present-but-malformed fields are
/// rejected so a bad payload cannot silently widen into an unbound query.
pub fn parse_record_query(input: &Value) -> DbResult<RecordQuery> {
    let model_id = input
        .get("modelId")
        .or_else(|| input.get("model"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| DbError::Invalid("modelId is required".into()))?
        .to_string();

    let limit = parse_optional_u64(input, "limit", 100)?;
    let offset = parse_optional_u64(input, "offset", 0)?;

    let search = match input.get("search") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            return Err(DbError::Invalid("search must be a string".into()));
        }
    };

    let search_fields = match input.get("searchFields") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(arr)) => {
            let mut out = Vec::with_capacity(arr.len());
            for x in arr {
                let Some(s) = x.as_str() else {
                    return Err(DbError::Invalid(
                        "searchFields entries must be strings".into(),
                    ));
                };
                out.push(s.to_string());
            }
            out
        }
        Some(_) => {
            return Err(DbError::Invalid("searchFields must be an array".into()));
        }
    };

    let mut q = RecordQuery {
        model_id,
        limit,
        offset,
        filters: Vec::new(),
        sort: None,
        search,
        search_fields,
        aggregates: Vec::new(),
    };

    match input.get("filters") {
        None | Some(Value::Null) => {}
        Some(Value::Array(arr)) => {
            for item in arr {
                let filter: QueryFilter = serde_json::from_value(item.clone())
                    .map_err(|e| DbError::Invalid(format!("invalid filter: {e}")))?;
                q.filters.push(filter);
            }
        }
        Some(_) => {
            return Err(DbError::Invalid("filters must be an array".into()));
        }
    }

    if let Some(sort_v) = input.get("sort") {
        if !sort_v.is_null() {
            let sort: QuerySort = serde_json::from_value(sort_v.clone())
                .map_err(|e| DbError::Invalid(format!("invalid sort: {e}")))?;
            // Empty field from unbound UI state means "default order".
            if !sort.field.trim().is_empty() {
                q.sort = Some(sort);
            }
        }
    }
    if let Some(s) = q.search.as_ref() {
        if s.trim().is_empty() {
            q.search = None;
        }
    }

    match input.get("aggregates") {
        None | Some(Value::Null) => {}
        Some(Value::Array(arr)) => {
            for item in arr {
                let agg: AggregateSpec = serde_json::from_value(item.clone())
                    .map_err(|e| DbError::Invalid(format!("invalid aggregate: {e}")))?;
                q.aggregates.push(agg);
            }
        }
        Some(_) => {
            return Err(DbError::Invalid("aggregates must be an array".into()));
        }
    }

    Ok(q)
}

fn parse_optional_u64(input: &Value, key: &str, default: u64) -> DbResult<u64> {
    match input.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .ok_or_else(|| DbError::Invalid(format!("{key} must be a non-negative integer"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_kernel::data::{create_record, upsert_model, DataField};
    use crate::db::Database;
    use tempfile::tempdir;

    fn open_db() -> Database {
        let dir = tempdir().unwrap();
        Database::open_path(&dir.path().join("q.db")).unwrap()
    }

    fn seed_writable_app(db: &mut Database, app: &str) {
        use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
        use std::collections::HashMap;
        upsert_manifest(
            db,
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
                application_action_access: vec![
                    "local_data.query".into(),
                    "local_data.write".into(),
                    "local_data.delete".into(),
                ],
                surface_action_access: HashMap::new(),
                component_action_access: HashMap::new(),
                action_descriptor_hashes: HashMap::new(),
            },
        )
        .unwrap();
        crate::application_kernel::permissions::grant_permission(
            db,
            app,
            "local_data.write",
            json!({}),
            "test",
        )
        .unwrap();
        crate::application_kernel::permissions::grant_permission(
            db,
            app,
            "local_data.read",
            json!({}),
            "test",
        )
        .unwrap();
    }

    fn seed_tasks(db: &mut Database, app: &str, n: usize) {
        seed_writable_app(db, app);
        upsert_model(
            db,
            app,
            DataModelDefinition {
                model_id: "studyTasks".into(),
                display_name: "Tasks".into(),
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
                        default: Some(json!("todo")),
                        enum_values: Some(vec!["todo".into(), "done".into()]),
                    },
                    DataField {
                        field_id: "dueDate".into(),
                        field_type: "date".into(),
                        required: false,
                        default: None,
                        enum_values: None,
                    },
                    DataField {
                        field_id: "priority".into(),
                        field_type: "enum".into(),
                        required: false,
                        default: Some(json!("medium")),
                        enum_values: Some(vec!["low".into(), "medium".into(), "high".into()]),
                    },
                ],
            },
        )
        .unwrap();
        for i in 0..n {
            let status = if i % 5 == 0 { "done" } else { "todo" };
            let due = if i % 7 == 0 {
                "2020-01-01"
            } else {
                "2099-01-01"
            };
            let priority = if i % 3 == 0 { "high" } else { "medium" };
            create_record(
                db,
                app,
                "studyTasks",
                json!({
                    "title": format!("Task {i:03}"),
                    "status": status,
                    "dueDate": due,
                    "priority": priority,
                }),
            )
            .unwrap();
        }
    }

    #[test]
    fn total_count_exceeds_returned_page() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-q", 150);
        let result = execute_query(
            &db,
            "app-q",
            &RecordQuery {
                model_id: "studyTasks".into(),
                limit: 25,
                offset: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.returned_count, 25);
        assert_eq!(result.count, 25);
        assert_eq!(result.total_count, 150);
        assert!(result.has_more);
        assert_eq!(result.offset, 0);
        assert_eq!(result.limit, 25);
    }

    #[test]
    fn pagination_reaches_all_records_without_duplicates() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-page", 60);
        let mut seen = std::collections::HashSet::new();
        let mut offset = 0u64;
        loop {
            let page = execute_query(
                &db,
                "app-page",
                &RecordQuery {
                    model_id: "studyTasks".into(),
                    limit: 25,
                    offset,
                    ..Default::default()
                },
            )
            .unwrap();
            for r in &page.records {
                let id = r.get("_id").and_then(|v| v.as_str()).unwrap().to_string();
                assert!(seen.insert(id), "duplicate record across pages");
            }
            if !page.has_more {
                break;
            }
            offset += page.returned_count as u64;
        }
        assert_eq!(seen.len(), 60);
    }

    #[test]
    fn stable_order_with_identical_timestamps() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-ts", 5);
        // Force identical updated_at
        db.conn()
            .execute(
                "UPDATE generated_data_records SET updated_at = '2026-01-01T00:00:00Z'
                 WHERE application_id = 'app-ts'",
                [],
            )
            .unwrap();
        let a = execute_query(
            &db,
            "app-ts",
            &RecordQuery {
                model_id: "studyTasks".into(),
                limit: 5,
                ..Default::default()
            },
        )
        .unwrap();
        let b = execute_query(
            &db,
            "app-ts",
            &RecordQuery {
                model_id: "studyTasks".into(),
                limit: 5,
                ..Default::default()
            },
        )
        .unwrap();
        let ids_a: Vec<_> = a
            .records
            .iter()
            .map(|r| r.get("_id").unwrap().as_str().unwrap())
            .collect();
        let ids_b: Vec<_> = b
            .records
            .iter()
            .map(|r| r.get("_id").unwrap().as_str().unwrap())
            .collect();
        assert_eq!(ids_a, ids_b);
    }

    #[test]
    fn filter_sort_search_and_aggregates() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-agg", 100);
        let result = execute_query(
            &db,
            "app-agg",
            &RecordQuery {
                model_id: "studyTasks".into(),
                limit: 10,
                offset: 0,
                filters: vec![QueryFilter {
                    field: "status".into(),
                    op: "eq".into(),
                    value: json!("todo"),
                }],
                sort: Some(QuerySort {
                    field: "title".into(),
                    direction: "asc".into(),
                }),
                search: Some("Task 05".into()),
                search_fields: vec!["title".into()],
                aggregates: vec![
                    AggregateSpec {
                        key: "total".into(),
                        op: "count".into(),
                        field: None,
                        equals: None,
                        not_equals: None,
                        status_field: None,
                        exclude_status: None,
                        as_of: None,
                    },
                    AggregateSpec {
                        key: "done".into(),
                        op: "countWhere".into(),
                        field: Some("status".into()),
                        equals: Some("done".into()),
                        not_equals: None,
                        status_field: None,
                        exclude_status: None,
                        as_of: None,
                    },
                    AggregateSpec {
                        key: "overdue".into(),
                        op: "countOverdue".into(),
                        field: Some("dueDate".into()),
                        equals: None,
                        not_equals: None,
                        status_field: Some("status".into()),
                        exclude_status: Some("done".into()),
                        as_of: Some("2026-10-20".into()),
                    },
                ],
            },
        )
        .unwrap();
        // search narrows; aggregates still apply search/filter scope for count
        assert!(result.total_count <= 100);
        let aggs = result.aggregates.as_ref().unwrap();
        assert!(aggs.get("total").and_then(|v| v.as_u64()).unwrap() <= 100);
        assert!(aggs.contains_key("done"));
        assert!(aggs.contains_key("overdue"));
    }

    #[test]
    fn rejects_unknown_filter_field_and_forged_ops() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-bad", 3);
        let err = execute_query(
            &db,
            "app-bad",
            &RecordQuery {
                model_id: "studyTasks".into(),
                filters: vec![QueryFilter {
                    field: "hack'; DROP TABLE--".into(),
                    op: "eq".into(),
                    value: json!("x"),
                }],
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("field") || err.to_string().contains("invalid"),
            "unexpected filter rejection: {err}"
        );

        let err2 = execute_query(
            &db,
            "app-bad",
            &RecordQuery {
                model_id: "studyTasks".into(),
                filters: vec![QueryFilter {
                    field: "status".into(),
                    op: "raw_sql".into(),
                    value: json!("x"),
                }],
                ..Default::default()
            },
        )
        .expect_err("forged filter op must fail");
        assert!(
            err2.to_string().contains("unsupported")
                || err2.to_string().contains("not allowed")
                || err2.to_string().contains("filter"),
            "unexpected op rejection: {err2}"
        );
    }

    #[test]
    fn revision_bumps_on_write() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-rev", 1);
        let before = model_revision(&db, "app-rev", "studyTasks").unwrap();
        assert!(before >= 1);
        create_record(
            &mut db,
            "app-rev",
            "studyTasks",
            json!({"title": "extra", "status": "todo"}),
        )
        .unwrap();
        let after = model_revision(&db, "app-rev", "studyTasks").unwrap();
        assert!(after > before);
    }

    #[test]
    fn empty_and_single_record() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-empty", 0);
        let empty = execute_query(
            &db,
            "app-empty",
            &RecordQuery {
                model_id: "studyTasks".into(),
                limit: 25,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(empty.total_count, 0);
        assert_eq!(empty.returned_count, 0);
        assert!(!empty.has_more);

        seed_tasks(&mut db, "app-one", 1);
        let one = execute_query(
            &db,
            "app-one",
            &RecordQuery {
                model_id: "studyTasks".into(),
                limit: 25,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(one.total_count, 1);
        assert_eq!(one.returned_count, 1);
        assert!(!one.has_more);
    }

    #[test]
    fn parse_legacy_query_input() {
        let q = parse_record_query(&json!({ "modelId": "studyTasks", "limit": 100 })).unwrap();
        assert_eq!(q.model_id, "studyTasks");
        assert_eq!(q.limit, 100);
        assert_eq!(q.offset, 0);
        assert!(q.filters.is_empty());
    }

    fn seed_numeric_model(db: &mut Database, app: &str) {
        seed_writable_app(db, app);
        upsert_model(
            db,
            app,
            DataModelDefinition {
                model_id: "metrics".into(),
                display_name: "Metrics".into(),
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
                        field_id: "score".into(),
                        field_type: "integer".into(),
                        required: true,
                        default: None,
                        enum_values: None,
                    },
                    DataField {
                        field_id: "weight".into(),
                        field_type: "decimal".into(),
                        required: false,
                        default: None,
                        enum_values: None,
                    },
                    DataField {
                        field_id: "active".into(),
                        field_type: "boolean".into(),
                        required: true,
                        default: Some(json!(false)),
                        enum_values: None,
                    },
                ],
            },
        )
        .unwrap();
        for (name, score, weight, active) in [
            ("a", 10i64, 1.5f64, true),
            ("b", 20, 2.5, false),
            ("c", 30, 3.5, true),
            ("d", 40, 4.5, false),
        ] {
            create_record(
                db,
                app,
                "metrics",
                json!({ "name": name, "score": score, "weight": weight, "active": active }),
            )
            .unwrap();
        }
    }

    #[test]
    fn typed_filters_match_json_extract_numbers_and_booleans() {
        let mut db = open_db();
        seed_numeric_model(&mut db, "app-num");

        let eq = execute_query(
            &db,
            "app-num",
            &RecordQuery {
                model_id: "metrics".into(),
                filters: vec![QueryFilter {
                    field: "score".into(),
                    op: "eq".into(),
                    value: json!(20),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(eq.total_count, 1);
        assert_eq!(eq.records[0]["name"], json!("b"));

        let gt = execute_query(
            &db,
            "app-num",
            &RecordQuery {
                model_id: "metrics".into(),
                filters: vec![QueryFilter {
                    field: "score".into(),
                    op: "gt".into(),
                    value: json!(25),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(gt.total_count, 2);

        let lt = execute_query(
            &db,
            "app-num",
            &RecordQuery {
                model_id: "metrics".into(),
                filters: vec![QueryFilter {
                    field: "weight".into(),
                    op: "lt".into(),
                    value: json!(3.0),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(lt.total_count, 2);

        let active = execute_query(
            &db,
            "app-num",
            &RecordQuery {
                model_id: "metrics".into(),
                filters: vec![QueryFilter {
                    field: "active".into(),
                    op: "eq".into(),
                    value: json!(true),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(active.total_count, 2);

        let inactive = execute_query(
            &db,
            "app-num",
            &RecordQuery {
                model_id: "metrics".into(),
                filters: vec![QueryFilter {
                    field: "active".into(),
                    op: "notEquals".into(),
                    value: json!(true),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(inactive.total_count, 2);

        let inn = execute_query(
            &db,
            "app-num",
            &RecordQuery {
                model_id: "metrics".into(),
                filters: vec![QueryFilter {
                    field: "score".into(),
                    op: "in".into(),
                    value: json!([10, 40]),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(inn.total_count, 2);
    }

    #[test]
    fn count_where_binds_typed_boolean_and_integer() {
        let mut db = open_db();
        seed_numeric_model(&mut db, "app-cw");
        let result = execute_query(
            &db,
            "app-cw",
            &RecordQuery {
                model_id: "metrics".into(),
                limit: 10,
                aggregates: vec![
                    AggregateSpec {
                        key: "active".into(),
                        op: "countWhere".into(),
                        field: Some("active".into()),
                        equals: Some("true".into()),
                        not_equals: None,
                        status_field: None,
                        exclude_status: None,
                        as_of: None,
                    },
                    AggregateSpec {
                        key: "high".into(),
                        op: "countWhere".into(),
                        field: Some("score".into()),
                        equals: Some("40".into()),
                        not_equals: None,
                        status_field: None,
                        exclude_status: None,
                        as_of: None,
                    },
                ],
                ..Default::default()
            },
        )
        .unwrap();
        let aggs = result.aggregates.as_ref().unwrap();
        assert_eq!(aggs.get("active").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(aggs.get("high").and_then(|v| v.as_u64()), Some(1));
    }

    #[test]
    fn rejects_boolean_range_ops_and_impossible_dates() {
        let mut db = open_db();
        seed_numeric_model(&mut db, "app-ops");
        let err = execute_query(
            &db,
            "app-ops",
            &RecordQuery {
                model_id: "metrics".into(),
                filters: vec![QueryFilter {
                    field: "active".into(),
                    op: "lt".into(),
                    value: json!(true),
                }],
                ..Default::default()
            },
        )
        .expect_err("boolean lt must fail");
        assert!(err.to_string().contains("not allowed"), "unexpected: {err}");

        seed_tasks(&mut db, "app-date", 1);
        let err2 = execute_query(
            &db,
            "app-date",
            &RecordQuery {
                model_id: "studyTasks".into(),
                filters: vec![QueryFilter {
                    field: "dueDate".into(),
                    op: "eq".into(),
                    value: json!("2026-02-31"),
                }],
                ..Default::default()
            },
        )
        .expect_err("impossible date must fail");
        assert!(
            err2.to_string().contains("calendar") || err2.to_string().contains("date"),
            "unexpected: {err2}"
        );
    }

    #[test]
    fn rejects_duplicate_aggregate_keys_and_malformed_parse() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-dup", 3);
        let err = execute_query(
            &db,
            "app-dup",
            &RecordQuery {
                model_id: "studyTasks".into(),
                aggregates: vec![
                    AggregateSpec {
                        key: "total".into(),
                        op: "count".into(),
                        field: None,
                        equals: None,
                        not_equals: None,
                        status_field: None,
                        exclude_status: None,
                        as_of: None,
                    },
                    AggregateSpec {
                        key: "total".into(),
                        op: "countWhere".into(),
                        field: Some("status".into()),
                        equals: Some("done".into()),
                        not_equals: None,
                        status_field: None,
                        exclude_status: None,
                        as_of: None,
                    },
                ],
                ..Default::default()
            },
        )
        .expect_err("duplicate aggregate key");
        assert!(err.to_string().contains("duplicate"));

        assert!(parse_record_query(&json!({
            "modelId": "studyTasks",
            "offset": "invalid"
        }))
        .is_err());
        assert!(parse_record_query(&json!({
            "modelId": "studyTasks",
            "filters": "not-an-array"
        }))
        .is_err());
        assert!(parse_record_query(&json!({
            "modelId": "studyTasks",
            "searchFields": [1, 2, 3]
        }))
        .is_err());
        assert!(parse_record_query(&json!({
            "modelId": "studyTasks",
            "aggregates": "invalid"
        }))
        .is_err());
        // Omitted fields still default.
        let ok = parse_record_query(&json!({ "modelId": "studyTasks" })).unwrap();
        assert_eq!(ok.offset, 0);
        assert_eq!(ok.limit, 100);
    }

    #[test]
    fn budget_tracker_numeric_and_boolean_semantics() {
        let mut db = open_db();
        seed_writable_app(&mut db, "app-budget");
        upsert_model(
            &mut db,
            "app-budget",
            DataModelDefinition {
                model_id: "expenses".into(),
                display_name: "Expenses".into(),
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
                        field_id: "amount".into(),
                        field_type: "decimal".into(),
                        required: true,
                        default: None,
                        enum_values: None,
                    },
                    DataField {
                        field_id: "category".into(),
                        field_type: "enum".into(),
                        required: true,
                        default: None,
                        enum_values: Some(vec!["Groceries".into(), "Rent".into(), "Other".into()]),
                    },
                    DataField {
                        field_id: "paid".into(),
                        field_type: "boolean".into(),
                        required: true,
                        default: Some(json!(false)),
                        enum_values: None,
                    },
                    DataField {
                        field_id: "spentOn".into(),
                        field_type: "date".into(),
                        required: false,
                        default: None,
                        enum_values: None,
                    },
                ],
            },
        )
        .unwrap();
        for (title, amount, category, paid, spent) in [
            ("Coffee", 2.5, "Other", false, "2026-10-01"),
            ("Milk", 19.5, "Groceries", true, "2026-10-02"),
            ("Rent", 100.0, "Rent", true, "2026-10-03"),
            ("Snacks", 8.0, "Groceries", false, "2026-10-04"),
        ] {
            create_record(
                &mut db,
                "app-budget",
                "expenses",
                json!({
                    "title": title,
                    "amount": amount,
                    "category": category,
                    "paid": paid,
                    "spentOn": spent,
                }),
            )
            .unwrap();
        }

        let gt = execute_query(
            &db,
            "app-budget",
            &RecordQuery {
                model_id: "expenses".into(),
                filters: vec![QueryFilter {
                    field: "amount".into(),
                    op: "gt".into(),
                    value: json!(10),
                }],
                sort: Some(QuerySort {
                    field: "amount".into(),
                    direction: "asc".into(),
                }),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(gt.total_count, 2);
        let amounts: Vec<f64> = gt
            .records
            .iter()
            .map(|r| r["amount"].as_f64().unwrap())
            .collect();
        assert_eq!(amounts, vec![19.5, 100.0]);

        let paid = execute_query(
            &db,
            "app-budget",
            &RecordQuery {
                model_id: "expenses".into(),
                filters: vec![QueryFilter {
                    field: "paid".into(),
                    op: "eq".into(),
                    value: json!(true),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(paid.total_count, 2);

        let groceries = execute_query(
            &db,
            "app-budget",
            &RecordQuery {
                model_id: "expenses".into(),
                filters: vec![QueryFilter {
                    field: "category".into(),
                    op: "eq".into(),
                    value: json!("Groceries"),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(groceries.total_count, 2);

        let not_equals = execute_query(
            &db,
            "app-budget",
            &RecordQuery {
                model_id: "expenses".into(),
                filters: vec![QueryFilter {
                    field: "paid".into(),
                    op: "not_equals".into(),
                    value: json!(true),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(not_equals.total_count, 2);
    }

    #[test]
    fn query_snapshot_revision_matches_page_and_count() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-snap", 5);
        let result = execute_query(
            &db,
            "app-snap",
            &RecordQuery {
                model_id: "studyTasks".into(),
                limit: 2,
                aggregates: vec![AggregateSpec {
                    key: "total".into(),
                    op: "count".into(),
                    field: None,
                    equals: None,
                    not_equals: None,
                    status_field: None,
                    exclude_status: None,
                    as_of: None,
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.total_count, 5);
        assert_eq!(result.returned_count, 2);
        assert_eq!(
            result.aggregates.as_ref().unwrap().get("total"),
            Some(&json!(5))
        );
        assert_eq!(
            result.revision,
            model_revision(&db, "app-snap", "studyTasks").unwrap()
        );
    }

    #[test]
    fn missing_revision_row_is_zero_not_storage_error() {
        let db = open_db();
        assert_eq!(model_revision(&db, "no-app", "no-model").unwrap(), 0);
    }

    #[test]
    fn rejects_nan_and_infinity_decimal_filters() {
        let mut db = open_db();
        seed_numeric_model(&mut db, "app-nan");
        for bad in ["NaN", "nan", "Infinity", "-Infinity", "inf", "-inf"] {
            let err = execute_query(
                &db,
                "app-nan",
                &RecordQuery {
                    model_id: "metrics".into(),
                    filters: vec![QueryFilter {
                        field: "weight".into(),
                        op: "eq".into(),
                        value: json!(bad),
                    }],
                    ..Default::default()
                },
            )
            .expect_err("non-finite decimal must fail");
            assert!(
                err.to_string().contains("finite"),
                "expected finite rejection for {bad}, got {err}"
            );
        }
    }

    #[test]
    fn date_filters_accept_leap_day_and_reject_non_leap() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-leap", 1);

        // Leap-day filter value is calendar-valid and binds.
        let leap = execute_query(
            &db,
            "app-leap",
            &RecordQuery {
                model_id: "studyTasks".into(),
                filters: vec![QueryFilter {
                    field: "dueDate".into(),
                    op: "eq".into(),
                    value: json!("2024-02-29"),
                }],
                ..Default::default()
            },
        )
        .expect("2024-02-29 must be accepted");
        assert_eq!(leap.total_count, 0);

        let err = execute_query(
            &db,
            "app-leap",
            &RecordQuery {
                model_id: "studyTasks".into(),
                filters: vec![QueryFilter {
                    field: "dueDate".into(),
                    op: "eq".into(),
                    value: json!("2023-02-29"),
                }],
                ..Default::default()
            },
        )
        .expect_err("non-leap Feb 29 must fail");
        assert!(
            err.to_string().contains("calendar") || err.to_string().contains("date"),
            "unexpected: {err}"
        );

        // Century non-leap (1900) vs 400-year leap (2000).
        let err1900 = execute_query(
            &db,
            "app-leap",
            &RecordQuery {
                model_id: "studyTasks".into(),
                filters: vec![QueryFilter {
                    field: "dueDate".into(),
                    op: "eq".into(),
                    value: json!("1900-02-29"),
                }],
                ..Default::default()
            },
        )
        .expect_err("1900-02-29 must fail");
        assert!(
            err1900.to_string().contains("calendar") || err1900.to_string().contains("date"),
            "unexpected: {err1900}"
        );
        execute_query(
            &db,
            "app-leap",
            &RecordQuery {
                model_id: "studyTasks".into(),
                filters: vec![QueryFilter {
                    field: "dueDate".into(),
                    op: "eq".into(),
                    value: json!("2000-02-29"),
                }],
                ..Default::default()
            },
        )
        .expect("2000-02-29 must be accepted");
    }

    #[test]
    fn date_range_filters_match_lexicographic_iso_order() {
        let mut db = open_db();
        seed_writable_app(&mut db, "app-drange");
        upsert_model(
            &mut db,
            "app-drange",
            DataModelDefinition {
                model_id: "events".into(),
                display_name: "Events".into(),
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
                        field_id: "onDate".into(),
                        field_type: "date".into(),
                        required: true,
                        default: None,
                        enum_values: None,
                    },
                ],
            },
        )
        .unwrap();
        for (title, on) in [
            ("early", "2026-01-01"),
            ("mid", "2026-06-15"),
            ("late", "2026-12-31"),
        ] {
            create_record(
                &mut db,
                "app-drange",
                "events",
                json!({ "title": title, "onDate": on }),
            )
            .unwrap();
        }

        let gte = execute_query(
            &db,
            "app-drange",
            &RecordQuery {
                model_id: "events".into(),
                filters: vec![QueryFilter {
                    field: "onDate".into(),
                    op: "gte".into(),
                    value: json!("2026-06-15"),
                }],
                sort: Some(QuerySort {
                    field: "onDate".into(),
                    direction: "asc".into(),
                }),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(gte.total_count, 2);
        assert_eq!(gte.records[0]["title"], json!("mid"));
        assert_eq!(gte.records[1]["title"], json!("late"));

        let lt = execute_query(
            &db,
            "app-drange",
            &RecordQuery {
                model_id: "events".into(),
                filters: vec![QueryFilter {
                    field: "onDate".into(),
                    op: "lt".into(),
                    value: json!("2026-06-15"),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(lt.total_count, 1);
        assert_eq!(lt.records[0]["title"], json!("early"));
    }

    #[test]
    fn rejects_ops_disallowed_for_field_types() {
        let mut db = open_db();
        seed_numeric_model(&mut db, "app-optypes");
        seed_tasks(&mut db, "app-optypes-tasks", 1);

        let cases: &[(&str, &str, &str, Value)] = &[
            (
                "app-optypes",
                "metrics",
                "score",
                json!({
                    "field": "score", "op": "contains", "value": "1"
                }),
            ),
            (
                "app-optypes",
                "metrics",
                "active",
                json!({
                    "field": "active", "op": "contains", "value": "true"
                }),
            ),
            (
                "app-optypes-tasks",
                "studyTasks",
                "dueDate",
                json!({
                    "field": "dueDate", "op": "in", "value": ["2020-01-01"]
                }),
            ),
            (
                "app-optypes-tasks",
                "studyTasks",
                "status",
                json!({
                    "field": "status", "op": "gt", "value": "todo"
                }),
            ),
            (
                "app-optypes-tasks",
                "studyTasks",
                "status",
                json!({
                    "field": "status", "op": "contains", "value": "do"
                }),
            ),
        ];

        for (app, model, _field, filter_json) in cases {
            let filter: QueryFilter = serde_json::from_value(filter_json.clone()).unwrap();
            let err = execute_query(
                &db,
                app,
                &RecordQuery {
                    model_id: (*model).into(),
                    filters: vec![filter],
                    ..Default::default()
                },
            )
            .expect_err("disallowed op must fail");
            assert!(
                err.to_string().contains("not allowed"),
                "expected not allowed for {filter_json}, got {err}"
            );
        }
    }

    #[test]
    fn rejects_invalid_as_of_on_count_overdue() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-asof", 3);
        let err = execute_query(
            &db,
            "app-asof",
            &RecordQuery {
                model_id: "studyTasks".into(),
                aggregates: vec![AggregateSpec {
                    key: "overdue".into(),
                    op: "countOverdue".into(),
                    field: Some("dueDate".into()),
                    equals: None,
                    not_equals: None,
                    status_field: None,
                    exclude_status: None,
                    as_of: Some("2026-02-31".into()),
                }],
                ..Default::default()
            },
        )
        .expect_err("invalid asOf must fail");
        assert!(
            err.to_string().contains("YYYY-MM-DD") || err.to_string().contains("calendar"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn parse_record_query_rejects_malformed_optional_scalars() {
        assert!(parse_record_query(&json!({
            "modelId": "studyTasks",
            "search": 12
        }))
        .is_err());
        assert!(parse_record_query(&json!({
            "modelId": "studyTasks",
            "limit": -1
        }))
        .is_err());
        assert!(parse_record_query(&json!({
            "modelId": "studyTasks",
            "searchFields": "title"
        }))
        .is_err());
        // Present empty search string collapses to None (not an error).
        let q = parse_record_query(&json!({
            "modelId": "studyTasks",
            "search": "   "
        }))
        .unwrap();
        assert!(q.search.is_none());
    }

    #[test]
    fn nested_read_snapshot_reuses_outer_txn_and_error_restores_autocommit() {
        let mut db = open_db();
        seed_tasks(&mut db, "app-snap-nest", 3);

        // Nested: outer deferred txn stays open across execute_query.
        db.conn().execute_batch("BEGIN DEFERRED").unwrap();
        assert!(!db.conn().is_autocommit());
        let page = execute_query(
            &db,
            "app-snap-nest",
            &RecordQuery {
                model_id: "studyTasks".into(),
                limit: 2,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(page.returned_count, 2);
        assert!(
            !db.conn().is_autocommit(),
            "nested snapshot must not commit outer txn"
        );
        db.conn().execute_batch("COMMIT").unwrap();
        assert!(db.conn().is_autocommit());

        // Corrupt a row so fetch_page fails inside the snapshot callback.
        db.conn()
            .execute(
                "UPDATE generated_data_records SET data_json = 'not-json'
                 WHERE application_id = 'app-snap-nest'",
                [],
            )
            .unwrap();
        let err = execute_query(
            &db,
            "app-snap-nest",
            &RecordQuery {
                model_id: "studyTasks".into(),
                limit: 10,
                ..Default::default()
            },
        )
        .expect_err("corrupt json must fail inside snapshot");
        let _ = err;
        assert!(
            db.conn().is_autocommit(),
            "failed snapshot must rollback to autocommit"
        );
        // Connection remains usable for subsequent reads after rollback.
        assert!(model_revision(&db, "app-snap-nest", "studyTasks").unwrap() > 0);
    }

    #[test]
    fn boolean_string_and_integer_coercion_in_filters() {
        let mut db = open_db();
        seed_numeric_model(&mut db, "app-bool-coerce");
        for value in [json!("true"), json!("1"), json!(1)] {
            let r = execute_query(
                &db,
                "app-bool-coerce",
                &RecordQuery {
                    model_id: "metrics".into(),
                    filters: vec![QueryFilter {
                        field: "active".into(),
                        op: "eq".into(),
                        value,
                    }],
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(r.total_count, 2);
        }
        let bad = execute_query(
            &db,
            "app-bool-coerce",
            &RecordQuery {
                model_id: "metrics".into(),
                filters: vec![QueryFilter {
                    field: "active".into(),
                    op: "eq".into(),
                    value: json!("yes"),
                }],
                ..Default::default()
            },
        )
        .expect_err("non-boolean string must fail");
        assert!(bad.to_string().contains("boolean"), "unexpected: {bad}");
    }
}
