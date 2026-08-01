//! Generated application route state with same-route no-op navigation.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RouteState {
    pub id: String,
    pub application_id: String,
    pub window_id: String,
    pub current_route_id: Option<String>,
    pub route_params: Value,
    pub history: Vec<Value>,
    pub history_index: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NavigateResult {
    pub state: RouteState,
    pub changed: bool,
}

pub fn get_route_state(
    db: &Database,
    application_id: &str,
    window_id: &str,
) -> DbResult<RouteState> {
    db.conn()
        .query_row(
            "SELECT id, application_id, window_id, current_route_id, route_params_json,
                    history_json, history_index, updated_at
             FROM application_route_state WHERE application_id = ?1 AND window_id = ?2",
            params![application_id, window_id],
            parse_route_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("route {application_id}/{window_id}"))
            }
            other => DbError::Sqlite(other),
        })
}

pub fn set_route_state(
    db: &mut Database,
    application_id: &str,
    window_id: &str,
    current_route_id: Option<&str>,
    route_params: &Value,
    history: &[Value],
    history_index: i64,
) -> DbResult<RouteState> {
    let now = now_rfc3339();
    let id = format!("route-{}", Uuid::new_v4());
    let history_json = serde_json::to_string(history)?;
    let params_json = route_params.to_string();
    db.conn().execute(
        "INSERT INTO application_route_state (
            id, application_id, window_id, current_route_id, route_params_json,
            history_json, history_index, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(application_id, window_id) DO UPDATE SET
            current_route_id = excluded.current_route_id,
            route_params_json = excluded.route_params_json,
            history_json = excluded.history_json,
            history_index = excluded.history_index,
            updated_at = excluded.updated_at",
        params![
            id,
            application_id,
            window_id,
            current_route_id,
            params_json,
            history_json,
            history_index,
            now
        ],
    )?;
    get_route_state(db, application_id, window_id)
}

pub fn navigate_route(
    db: &mut Database,
    application_id: &str,
    window_id: &str,
    route_id: &str,
    route_params: &Value,
    push_history: bool,
) -> DbResult<NavigateResult> {
    let existing = get_route_state(db, application_id, window_id).ok();
    if let Some(ref state) = existing {
        let same_route = state.current_route_id.as_deref() == Some(route_id);
        let same_params = &state.route_params == route_params;
        if same_route && same_params {
            return Ok(NavigateResult {
                state: state.clone(),
                changed: false,
            });
        }
    }

    let mut history = existing
        .as_ref()
        .map(|s| s.history.clone())
        .unwrap_or_default();
    let mut index = existing.as_ref().map(|s| s.history_index).unwrap_or(0);
    if push_history {
        if let Some(ref state) = existing {
            if let Some(cur) = state.current_route_id.as_ref() {
                history.truncate((index + 1) as usize);
                history.push(json!({
                    "routeId": cur,
                    "params": state.route_params,
                }));
                index = history.len() as i64 - 1;
            }
        }
        history.push(json!({
            "routeId": route_id,
            "params": route_params,
        }));
        index = history.len() as i64 - 1;
    }

    let state = set_route_state(
        db,
        application_id,
        window_id,
        Some(route_id),
        route_params,
        &history,
        index,
    )?;
    Ok(NavigateResult {
        state,
        changed: true,
    })
}

fn parse_route_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RouteState> {
    let params_json: String = row.get(4)?;
    let history_json: String = row.get(5)?;
    Ok(RouteState {
        id: row.get(0)?,
        application_id: row.get(1)?,
        window_id: row.get(2)?,
        current_route_id: row.get(3)?,
        route_params: serde_json::from_str(&params_json).unwrap_or(json!({})),
        history: serde_json::from_str(&history_json).unwrap_or_default(),
        history_index: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn test_db() -> crate::db::Database {
        let dir = tempdir().unwrap();
        crate::db::Database::open_path(&dir.path().join("t.db")).unwrap()
    }

    #[test]
    fn route_no_op_when_same() {
        let mut db = test_db();
        set_route_state(&mut db, "app-1", "main", Some("home"), &json!({}), &[], 0).unwrap();
        let result = navigate_route(&mut db, "app-1", "main", "home", &json!({}), true).unwrap();
        assert!(!result.changed);
        let result2 =
            navigate_route(&mut db, "app-1", "main", "settings", &json!({}), true).unwrap();
        assert!(result2.changed);
        assert_eq!(result2.state.current_route_id.as_deref(), Some("settings"));
    }
}
