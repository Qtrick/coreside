//! Persist Exa usage to SQLite (costs only — never API keys).

use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{Database, DbError, DbResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEntry {
    pub id: String,
    pub created_at: String,
    pub month_key: String,
    pub request_id: Option<String>,
    pub search_mode: String,
    pub result_count: i64,
    pub actual_cost: Option<f64>,
    pub estimated_cost: Option<f64>,
    pub cache_hit: bool,
    pub status: String,
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub month_key: String,
    pub request_count: i64,
    pub result_count: i64,
    pub actual_cost_total: f64,
    pub estimated_cost_total: f64,
    pub cache_hits: i64,
}

pub fn month_key_now() -> String {
    let now = Utc::now();
    format!("{:04}-{:02}", now.year(), now.month())
}

pub fn record_usage(
    db: &mut Database,
    search_mode: &str,
    result_count: usize,
    request_id: Option<&str>,
    actual_cost: Option<f64>,
    estimated_cost: Option<f64>,
    cache_hit: bool,
    status: &str,
    conversation_id: Option<&str>,
) -> DbResult<UsageEntry> {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let month_key = month_key_now();
    let entry = UsageEntry {
        id: id.clone(),
        created_at: created_at.clone(),
        month_key: month_key.clone(),
        request_id: request_id.map(str::to_string),
        search_mode: search_mode.to_string(),
        result_count: result_count as i64,
        actual_cost,
        estimated_cost,
        cache_hit,
        status: status.to_string(),
        conversation_id: conversation_id.map(str::to_string),
    };

    db.conn().execute(
        "INSERT INTO exa_usage_ledger (
            id, created_at, month_key, request_id, search_mode, result_count,
            actual_cost, estimated_cost, cache_hit, status, conversation_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            entry.id,
            entry.created_at,
            entry.month_key,
            entry.request_id,
            entry.search_mode,
            entry.result_count,
            entry.actual_cost,
            entry.estimated_cost,
            if entry.cache_hit { 1 } else { 0 },
            entry.status,
            entry.conversation_id,
        ],
    )?;
    Ok(entry)
}

pub fn sum_month_actual_cost(db: &Database, month_key: &str) -> DbResult<f64> {
    let total: f64 = db.conn().query_row(
        "SELECT COALESCE(SUM(COALESCE(actual_cost, estimated_cost, 0)), 0)
         FROM exa_usage_ledger
         WHERE month_key = ?1 AND status = 'ok' AND cache_hit = 0",
        [month_key],
        |row| row.get(0),
    )?;
    Ok(total)
}

pub fn usage_summary(db: &Database, month_key: Option<&str>) -> DbResult<UsageSummary> {
    let key = month_key
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(month_key_now);

    let row = db.conn().query_row(
        "SELECT
            COUNT(*),
            COALESCE(SUM(result_count), 0),
            COALESCE(SUM(actual_cost), 0),
            COALESCE(SUM(estimated_cost), 0),
            COALESCE(SUM(cache_hit), 0)
         FROM exa_usage_ledger
         WHERE month_key = ?1",
        [&key],
        |row| {
            Ok(UsageSummary {
                month_key: key.clone(),
                request_count: row.get(0)?,
                result_count: row.get(1)?,
                actual_cost_total: row.get(2)?,
                estimated_cost_total: row.get(3)?,
                cache_hits: row.get(4)?,
            })
        },
    );

    match row {
        Ok(summary) => Ok(summary),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(UsageSummary {
            month_key: key,
            request_count: 0,
            result_count: 0,
            actual_cost_total: 0.0,
            estimated_cost_total: 0.0,
            cache_hits: 0,
        }),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

pub fn list_recent_usage(db: &Database, limit: usize) -> DbResult<Vec<UsageEntry>> {
    let limit = limit.clamp(1, 200) as i64;
    let mut stmt = db.conn().prepare(
        "SELECT id, created_at, month_key, request_id, search_mode, result_count,
                actual_cost, estimated_cost, cache_hit, status, conversation_id
         FROM exa_usage_ledger
         ORDER BY created_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(UsageEntry {
            id: row.get(0)?,
            created_at: row.get(1)?,
            month_key: row.get(2)?,
            request_id: row.get(3)?,
            search_mode: row.get(4)?,
            result_count: row.get(5)?,
            actual_cost: row.get(6)?,
            estimated_cost: row.get(7)?,
            cache_hit: row.get::<_, i64>(8)? != 0,
            status: row.get(9)?,
            conversation_id: row.get(10)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
