//! Component preservation policies — trusted Partial Update adaptation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreservationPolicy {
    Replace,
    PreserveInstance,
    PreserveState,
    PreserveUserInput,
    PreserveMediaState,
    PreserveScroll,
    PreserveFocus,
    PreserveSelection,
    PreserveIfCompatible,
    ResetExplicitly,
}

impl PreservationPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::PreserveInstance => "preserve_instance",
            Self::PreserveState => "preserve_state",
            Self::PreserveUserInput => "preserve_user_input",
            Self::PreserveMediaState => "preserve_media_state",
            Self::PreserveScroll => "preserve_scroll",
            Self::PreserveFocus => "preserve_focus",
            Self::PreserveSelection => "preserve_selection",
            Self::PreserveIfCompatible => "preserve_if_compatible",
            Self::ResetExplicitly => "reset_explicitly",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "replace" => Some(Self::Replace),
            "preserve_instance" => Some(Self::PreserveInstance),
            "preserve_state" => Some(Self::PreserveState),
            "preserve_user_input" => Some(Self::PreserveUserInput),
            "preserve_media_state" => Some(Self::PreserveMediaState),
            "preserve_scroll" => Some(Self::PreserveScroll),
            "preserve_focus" => Some(Self::PreserveFocus),
            "preserve_selection" => Some(Self::PreserveSelection),
            "preserve_if_compatible" => Some(Self::PreserveIfCompatible),
            "reset_explicitly" => Some(Self::ResetExplicitly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreservationRecord {
    pub id: String,
    pub surface_id: String,
    pub component_id: String,
    pub preservation_key: Option<String>,
    pub policy: PreservationPolicy,
    pub state_schema_version: String,
    pub capability_pack: Option<String>,
    pub component_type: String,
    pub last_preserved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Whether an incoming component should preserve live state given stored policy.
pub fn should_preserve(
    policy: PreservationPolicy,
    incoming_key: Option<&str>,
    stored_key: Option<&str>,
    compatible: bool,
) -> bool {
    match policy {
        PreservationPolicy::Replace | PreservationPolicy::ResetExplicitly => false,
        PreservationPolicy::PreserveInstance
        | PreservationPolicy::PreserveState
        | PreservationPolicy::PreserveUserInput
        | PreservationPolicy::PreserveMediaState
        | PreservationPolicy::PreserveScroll
        | PreservationPolicy::PreserveFocus
        | PreservationPolicy::PreserveSelection => true,
        PreservationPolicy::PreserveIfCompatible => {
            if let (Some(a), Some(b)) = (incoming_key, stored_key) {
                a == b && compatible
            } else {
                compatible
            }
        }
    }
}

pub fn upsert_preservation(
    db: &mut Database,
    surface_id: &str,
    component_id: &str,
    policy: PreservationPolicy,
    preservation_key: Option<&str>,
    component_type: &str,
    capability_pack: Option<&str>,
) -> DbResult<PreservationRecord> {
    let now = now_rfc3339();
    let existing: Option<String> = db
        .conn()
        .query_row(
            "SELECT id FROM component_preservation WHERE surface_id = ?1 AND component_id = ?2",
            params![surface_id, component_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(DbError::Sqlite)?;

    let id = existing.unwrap_or_else(|| format!("pres-{}", Uuid::new_v4()));
    db.conn().execute(
        "INSERT INTO component_preservation (
            id, surface_id, component_id, preservation_key, policy, component_type,
            capability_pack, last_preserved_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(surface_id, component_id) DO UPDATE SET
            preservation_key = excluded.preservation_key,
            policy = excluded.policy,
            component_type = excluded.component_type,
            capability_pack = excluded.capability_pack,
            last_preserved_at = excluded.last_preserved_at,
            updated_at = excluded.updated_at",
        params![
            id,
            surface_id,
            component_id,
            preservation_key,
            policy.as_str(),
            component_type,
            capability_pack,
            now,
            now,
            now
        ],
    )?;
    get_preservation(db, surface_id, component_id)
}

pub fn get_preservation(
    db: &Database,
    surface_id: &str,
    component_id: &str,
) -> DbResult<PreservationRecord> {
    db.conn()
        .query_row(
            "SELECT id, surface_id, component_id, preservation_key, policy,
                    state_schema_version, capability_pack, component_type,
                    last_preserved_at, created_at, updated_at
             FROM component_preservation WHERE surface_id = ?1 AND component_id = ?2",
            params![surface_id, component_id],
            |row| {
                let policy_str: String = row.get(4)?;
                Ok(PreservationRecord {
                    id: row.get(0)?,
                    surface_id: row.get(1)?,
                    component_id: row.get(2)?,
                    preservation_key: row.get(3)?,
                    policy: PreservationPolicy::parse(&policy_str)
                        .unwrap_or(PreservationPolicy::PreserveIfCompatible),
                    state_schema_version: row.get(5)?,
                    capability_pack: row.get(6)?,
                    component_type: row.get(7)?,
                    last_preserved_at: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("preservation {surface_id}/{component_id}"))
            }
            other => DbError::Sqlite(other),
        })
}

pub fn list_preservation_for_surface(
    db: &Database,
    surface_id: &str,
) -> DbResult<Vec<PreservationRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, surface_id, component_id, preservation_key, policy,
                state_schema_version, capability_pack, component_type,
                last_preserved_at, created_at, updated_at
         FROM component_preservation WHERE surface_id = ?1",
    )?;
    let rows = stmt.query_map([surface_id], |row| {
        let policy_str: String = row.get(4)?;
        Ok(PreservationRecord {
            id: row.get(0)?,
            surface_id: row.get(1)?,
            component_id: row.get(2)?,
            preservation_key: row.get(3)?,
            policy: PreservationPolicy::parse(&policy_str)
                .unwrap_or(PreservationPolicy::PreserveIfCompatible),
            state_schema_version: row.get(5)?,
            capability_pack: row.get(6)?,
            component_type: row.get(7)?,
            last_preserved_at: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::Sqlite)
}

trait OptionalRow<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalRow<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_db() -> crate::db::Database {
        let dir = tempdir().unwrap();
        crate::db::Database::open_path(&dir.path().join("t.db")).unwrap()
    }

    #[test]
    fn preservation_match_by_key() {
        assert!(should_preserve(
            PreservationPolicy::PreserveIfCompatible,
            Some("timer-1"),
            Some("timer-1"),
            true
        ));
        assert!(!should_preserve(
            PreservationPolicy::PreserveIfCompatible,
            Some("timer-2"),
            Some("timer-1"),
            true
        ));
        assert!(!should_preserve(
            PreservationPolicy::Replace,
            Some("x"),
            Some("x"),
            true
        ));
        assert!(should_preserve(
            PreservationPolicy::PreserveFocus,
            None,
            None,
            false
        ));
    }

    #[test]
    fn preservation_roundtrip() {
        let mut db = test_db();
        let rec = upsert_preservation(
            &mut db,
            "surf-1",
            "cmp-1",
            PreservationPolicy::PreserveMediaState,
            Some("media-key"),
            "video",
            Some("media"),
        )
        .unwrap();
        assert_eq!(rec.policy, PreservationPolicy::PreserveMediaState);
        let loaded = get_preservation(&db, "surf-1", "cmp-1").unwrap();
        assert_eq!(loaded.preservation_key.as_deref(), Some("media-key"));
    }
}
