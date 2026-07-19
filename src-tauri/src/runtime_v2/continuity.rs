//! Focus, scroll, and media continuity snapshots with suspension.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuspensionState {
    Active,
    Suspended,
    Background,
}

impl SuspensionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Background => "background",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "suspended" => Self::Suspended,
            "background" => Self::Background,
            _ => Self::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContinuitySnapshot {
    pub id: String,
    pub surface_id: String,
    pub window_id: String,
    pub focus: Value,
    pub scroll: Value,
    pub media: Value,
    pub suspension_state: SuspensionState,
    pub updated_at: String,
}

pub fn get_continuity(
    db: &Database,
    surface_id: &str,
    window_id: &str,
) -> DbResult<ContinuitySnapshot> {
    db.conn()
        .query_row(
            "SELECT id, surface_id, window_id, focus_json, scroll_json, media_json,
                    suspension_state, updated_at
             FROM surface_continuity WHERE surface_id = ?1 AND window_id = ?2",
            params![surface_id, window_id],
            |row| parse_continuity_row(row),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("continuity {surface_id}/{window_id}"))
            }
            other => DbError::Sqlite(other),
        })
}

pub fn save_continuity(
    db: &mut Database,
    surface_id: &str,
    window_id: &str,
    focus: &Value,
    scroll: &Value,
    media: &Value,
    suspension_state: SuspensionState,
) -> DbResult<ContinuitySnapshot> {
    let now = now_rfc3339();
    let id = format!("cont-{}", Uuid::new_v4());
    db.conn().execute(
        "INSERT INTO surface_continuity (
            id, surface_id, window_id, focus_json, scroll_json, media_json,
            suspension_state, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(surface_id, window_id) DO UPDATE SET
            focus_json = excluded.focus_json,
            scroll_json = excluded.scroll_json,
            media_json = excluded.media_json,
            suspension_state = excluded.suspension_state,
            updated_at = excluded.updated_at",
        params![
            id,
            surface_id,
            window_id,
            focus.to_string(),
            scroll.to_string(),
            media.to_string(),
            suspension_state.as_str(),
            now
        ],
    )?;
    get_continuity(db, surface_id, window_id)
}

pub fn suspend_surface(
    db: &mut Database,
    surface_id: &str,
    window_id: &str,
) -> DbResult<ContinuitySnapshot> {
    let existing = get_continuity(db, surface_id, window_id).unwrap_or(ContinuitySnapshot {
        id: format!("cont-{}", Uuid::new_v4()),
        surface_id: surface_id.into(),
        window_id: window_id.into(),
        focus: json!({}),
        scroll: json!({}),
        media: json!({}),
        suspension_state: SuspensionState::Active,
        updated_at: now_rfc3339(),
    });
    save_continuity(
        db,
        surface_id,
        window_id,
        &existing.focus,
        &existing.scroll,
        &existing.media,
        SuspensionState::Suspended,
    )
}

fn parse_continuity_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContinuitySnapshot> {
    let focus_json: String = row.get(3)?;
    let scroll_json: String = row.get(4)?;
    let media_json: String = row.get(5)?;
    let suspension: String = row.get(6)?;
    Ok(ContinuitySnapshot {
        id: row.get(0)?,
        surface_id: row.get(1)?,
        window_id: row.get(2)?,
        focus: serde_json::from_str(&focus_json).unwrap_or(json!({})),
        scroll: serde_json::from_str(&scroll_json).unwrap_or(json!({})),
        media: serde_json::from_str(&media_json).unwrap_or(json!({})),
        suspension_state: SuspensionState::parse(&suspension),
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
    fn continuity_suspend() {
        let mut db = test_db();
        save_continuity(
            &mut db,
            "surf-1",
            "main",
            &json!({"componentId": "input-1"}),
            &json!({"y": 120}),
            &json!({"playing": true}),
            SuspensionState::Active,
        )
        .unwrap();
        let suspended = suspend_surface(&mut db, "surf-1", "main").unwrap();
        assert_eq!(suspended.suspension_state, SuspensionState::Suspended);
        assert_eq!(
            suspended.focus.get("componentId").and_then(|v| v.as_str()),
            Some("input-1")
        );
    }
}
