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
    // 1. Bound route params
    let params_str = route_params.to_string();
    if params_str.len() > 16384 {
        return Err(DbError::Invalid(
            "Route parameters exceed maximum allowed size of 16KB".to_string(),
        ));
    }
    if route_id.trim().is_empty() || route_id.len() > 128 {
        return Err(DbError::Invalid("Invalid route id".to_string()));
    }

    // 2. Validate route membership against manifest if manifest exists
    if let Ok(manifest_rec) = crate::application_kernel::manifest::get_manifest(db, application_id) {
        if !manifest_rec.manifest.routes.is_empty() {
            let matching_route = manifest_rec
                .manifest
                .routes
                .iter()
                .find(|r| r.route_id == route_id);
            match matching_route {
                None => {
                    return Err(DbError::Invalid(format!(
                        "Route '{route_id}' is not declared in application '{application_id}' manifest"
                    )));
                }
                Some(r) => {
                    if let Some(ref surface_id) = r.surface_id {
                        // Check surface ownership
                        if let Ok(surf) = crate::runtime_v2::surfaces::get_surface(db, surface_id) {
                            if surf.tool_id.as_deref() != Some(application_id) || surf.archived {
                                return Err(DbError::Invalid(format!(
                                    "Surface '{surface_id}' for route '{route_id}' does not belong to application or is archived"
                                )));
                            }
                        }
                    }
                }
            }
        }
    }

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
        history.truncate((index + 1).max(0) as usize);
        history.push(json!({
            "routeId": route_id,
            "params": route_params,
        }));
        // Bound history to max 50 entries
        if history.len() > 50 {
            let start = history.len() - 50;
            history = history.split_off(start);
        }
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

pub fn route_back(
    db: &mut Database,
    application_id: &str,
    window_id: &str,
) -> DbResult<NavigateResult> {
    let state = get_route_state(db, application_id, window_id)?;
    if state.history_index <= 0 || state.history.is_empty() {
        return Ok(NavigateResult {
            state,
            changed: false,
        });
    }
    let new_index = (state.history_index - 1) as usize;
    if let Some(target) = state.history.get(new_index) {
        let route_id = target
            .get("routeId")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let params = target.get("params").cloned().unwrap_or(json!({}));
        let updated = set_route_state(
            db,
            application_id,
            window_id,
            Some(route_id),
            &params,
            &state.history,
            new_index as i64,
        )?;
        Ok(NavigateResult {
            state: updated,
            changed: true,
        })
    } else {
        Ok(NavigateResult {
            state,
            changed: false,
        })
    }
}

pub fn route_forward(
    db: &mut Database,
    application_id: &str,
    window_id: &str,
) -> DbResult<NavigateResult> {
    let state = get_route_state(db, application_id, window_id)?;
    let max_index = (state.history.len() as i64) - 1;
    if state.history_index >= max_index || state.history.is_empty() {
        return Ok(NavigateResult {
            state,
            changed: false,
        });
    }
    let new_index = (state.history_index + 1) as usize;
    if let Some(target) = state.history.get(new_index) {
        let route_id = target
            .get("routeId")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let params = target.get("params").cloned().unwrap_or(json!({}));
        let updated = set_route_state(
            db,
            application_id,
            window_id,
            Some(route_id),
            &params,
            &state.history,
            new_index as i64,
        )?;
        Ok(NavigateResult {
            state: updated,
            changed: true,
        })
    } else {
        Ok(NavigateResult {
            state,
            changed: false,
        })
    }
}

pub fn ensure_initial_route(
    db: &mut Database,
    application_id: &str,
    window_id: &str,
) -> DbResult<RouteState> {
    if let Ok(state) = get_route_state(db, application_id, window_id) {
        if state.current_route_id.is_some() {
            return Ok(state);
        }
    }
    let initial_route_id =
        if let Ok(rec) = crate::application_kernel::manifest::get_manifest(db, application_id) {
            rec.manifest.routes.first().map(|r| r.route_id.clone())
        } else {
            None
        };
    let route_id = initial_route_id.unwrap_or_else(|| "main".to_string());
    navigate_route(db, application_id, window_id, &route_id, &json!({}), false).map(|r| r.state)
}

fn parse_route_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RouteState> {
    let params_json: String = row.get(4)?;
    let history_json: String = row.get(5)?;
    let route_params: Value = serde_json::from_str(&params_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let history: Vec<Value> = serde_json::from_str(&history_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(RouteState {
        id: row.get(0)?,
        application_id: row.get(1)?,
        window_id: row.get(2)?,
        current_route_id: row.get(3)?,
        route_params,
        history,
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

    #[test]
    fn route_back_and_forward() {
        let mut db = test_db();
        navigate_route(&mut db, "app-1", "main", "step1", &json!({}), true).unwrap();
        navigate_route(&mut db, "app-1", "main", "step2", &json!({}), true).unwrap();
        navigate_route(&mut db, "app-1", "main", "step3", &json!({}), true).unwrap();

        let back1 = route_back(&mut db, "app-1", "main").unwrap();
        assert!(back1.changed);
        assert_eq!(back1.state.current_route_id.as_deref(), Some("step2"));

        let back2 = route_back(&mut db, "app-1", "main").unwrap();
        assert!(back2.changed);
        assert_eq!(back2.state.current_route_id.as_deref(), Some("step1"));

        let back3 = route_back(&mut db, "app-1", "main").unwrap();
        assert!(!back3.changed);

        let fwd1 = route_forward(&mut db, "app-1", "main").unwrap();
        assert!(fwd1.changed);
        assert_eq!(fwd1.state.current_route_id.as_deref(), Some("step2"));
    }
}
