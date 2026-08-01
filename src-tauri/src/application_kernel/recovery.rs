//! Recovery Mode + safe startup + crash-loop protection.
//! Protected core only — never depends on generated components.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::{now_rfc3339, Database, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryState {
    pub recovery_mode: bool,
    pub disable_user_surfaces: bool,
    pub disable_custom_layouts: bool,
    pub disable_capability_packs: bool,
    pub unclean_shutdown: bool,
    pub last_failure: Option<serde_json::Value>,
    pub updated_at: String,
}

pub fn get_recovery_state(db: &Database) -> DbResult<RecoveryState> {
    db.conn()
        .query_row(
            "SELECT recovery_mode, disable_user_surfaces, disable_custom_layouts,
                    disable_capability_packs, unclean_shutdown, last_failure_json, updated_at
             FROM recovery_state WHERE id = 'local'",
            [],
            |row| {
                let fail: Option<String> = row.get(5)?;
                Ok(RecoveryState {
                    recovery_mode: row.get::<_, i64>(0)? != 0,
                    disable_user_surfaces: row.get::<_, i64>(1)? != 0,
                    disable_custom_layouts: row.get::<_, i64>(2)? != 0,
                    disable_capability_packs: row.get::<_, i64>(3)? != 0,
                    unclean_shutdown: row.get::<_, i64>(4)? != 0,
                    last_failure: fail.and_then(|s| serde_json::from_str(&s).ok()),
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(crate::db::DbError::Sqlite)
}

pub fn set_recovery_mode(db: &mut Database, enabled: bool) -> DbResult<RecoveryState> {
    db.conn().execute(
        "UPDATE recovery_state SET recovery_mode = ?1, updated_at = ?2 WHERE id = 'local'",
        params![if enabled { 1 } else { 0 }, now_rfc3339()],
    )?;
    get_recovery_state(db)
}

pub fn enter_safe_startup(db: &mut Database, reason: &str) -> DbResult<RecoveryState> {
    let fail = json!({ "reason": reason, "at": now_rfc3339() }).to_string();
    db.conn().execute(
        "UPDATE recovery_state SET
            recovery_mode = 1,
            disable_user_surfaces = 1,
            unclean_shutdown = 1,
            last_failure_json = ?1,
            updated_at = ?2
         WHERE id = 'local'",
        params![fail, now_rfc3339()],
    )?;
    // Suspend repeatedly crashing apps
    db.conn().execute(
        "UPDATE application_manifests SET lifecycle_state = 'suspended', health_state = 'suspended'
         WHERE crash_count >= 3 AND disabled = 0",
        [],
    )?;
    get_recovery_state(db)
}

pub fn clear_recovery(db: &mut Database) -> DbResult<RecoveryState> {
    db.conn().execute(
        "UPDATE recovery_state SET
            recovery_mode = 0,
            disable_user_surfaces = 0,
            disable_custom_layouts = 0,
            disable_capability_packs = 0,
            unclean_shutdown = 0,
            last_failure_json = NULL,
            updated_at = ?1
         WHERE id = 'local'",
        [now_rfc3339()],
    )?;
    get_recovery_state(db)
}

pub fn set_flags(
    db: &mut Database,
    disable_user_surfaces: Option<bool>,
    disable_custom_layouts: Option<bool>,
    disable_capability_packs: Option<bool>,
) -> DbResult<RecoveryState> {
    let cur = get_recovery_state(db)?;
    db.conn().execute(
        "UPDATE recovery_state SET
            disable_user_surfaces = ?1,
            disable_custom_layouts = ?2,
            disable_capability_packs = ?3,
            updated_at = ?4
         WHERE id = 'local'",
        params![
            if disable_user_surfaces.unwrap_or(cur.disable_user_surfaces) {
                1
            } else {
                0
            },
            if disable_custom_layouts.unwrap_or(cur.disable_custom_layouts) {
                1
            } else {
                0
            },
            if disable_capability_packs.unwrap_or(cur.disable_capability_packs) {
                1
            } else {
                0
            },
            now_rfc3339()
        ],
    )?;
    get_recovery_state(db)
}

/// Agent must not disable Recovery Mode via operations.
pub fn assert_agent_cannot_disable_recovery(
    op_type: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    if op_type.contains("recovery") {
        return Err("Recovery Mode is protected".into());
    }
    if payload.get("recoveryMode") == Some(&serde_json::json!(false))
        || payload.get("disableRecovery") == Some(&serde_json::json!(true))
    {
        return Err("cannot disable Recovery Mode".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_agent_disable() {
        assert!(assert_agent_cannot_disable_recovery(
            "settings.update",
            &json!({ "recoveryMode": false })
        )
        .is_err());
    }
}
