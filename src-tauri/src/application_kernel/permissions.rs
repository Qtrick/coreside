//! Application permissions — declared, granted by trusted code only.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

pub const ALLOWED_PERMISSIONS: &[&str] = &[
    "local_data.read",
    "local_data.write",
    "media.read",
    "media.create",
    "media.export",
    "project_context.read",
    "tool_reference.read",
    "automation.propose",
    "export.prepare",
    "microphone.request",
    "clipboard.read",
    "clipboard.write",
    "file.open_user_selected",
    "file.save_user_selected",
    "external_link.open",
    "web_search.request",
];

pub const FORBIDDEN_PERMISSIONS: &[&str] = &[
    "unrestricted.filesystem",
    "unrestricted.network",
    "unrestricted.shell",
    "unrestricted.tauri",
    "credential.read",
    "credential.write",
    "protected_settings.write",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionGrant {
    pub id: String,
    pub application_id: String,
    pub permission: String,
    pub scope: serde_json::Value,
    pub status: String,
    pub grant_source: String,
    pub granted_at: Option<String>,
    pub revoked_at: Option<String>,
}

pub fn validate_declared_permissions(perms: &[String]) -> Result<(), String> {
    for p in perms {
        if FORBIDDEN_PERMISSIONS.contains(&p.as_str()) || p.starts_with("unrestricted.") {
            return Err(format!("forbidden permission: {p}"));
        }
        if !ALLOWED_PERMISSIONS.contains(&p.as_str()) {
            return Err(format!("unknown permission: {p}"));
        }
    }
    Ok(())
}

/// Trusted grant — never callable as an agent operation.
pub fn grant_permission(
    db: &mut Database,
    application_id: &str,
    permission: &str,
    scope: serde_json::Value,
    grant_source: &str,
) -> DbResult<PermissionGrant> {
    validate_declared_permissions(&[permission.to_string()]).map_err(DbError::Invalid)?;
    let id = format!("perm-{}", Uuid::new_v4());
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO application_permissions (
            id, application_id, permission, scope_json, status, grant_source, granted_at
         ) VALUES (?1,?2,?3,?4,'granted',?5,?6)
         ON CONFLICT(application_id, permission) DO UPDATE SET
            status = 'granted', scope_json = excluded.scope_json,
            grant_source = excluded.grant_source, granted_at = excluded.granted_at, revoked_at = NULL",
        params![
            id,
            application_id,
            permission,
            scope.to_string(),
            grant_source,
            now
        ],
    )?;
    get_permission(db, application_id, permission)
}

pub fn revoke_permission(
    db: &mut Database,
    application_id: &str,
    permission: &str,
) -> DbResult<()> {
    db.conn().execute(
        "UPDATE application_permissions SET status = 'revoked', revoked_at = ?3
         WHERE application_id = ?1 AND permission = ?2",
        params![application_id, permission, now_rfc3339()],
    )?;
    Ok(())
}

pub fn has_permission(db: &Database, application_id: &str, permission: &str) -> DbResult<bool> {
    let status: Option<String> = db
        .conn()
        .query_row(
            "SELECT status FROM application_permissions
             WHERE application_id = ?1 AND permission = ?2",
            params![application_id, permission],
            |row| row.get(0),
        )
        .optional()
        .map_err(DbError::Sqlite)?;
    Ok(matches!(status.as_deref(), Some("granted")))
}

use rusqlite::OptionalExtension;

fn get_permission(
    db: &Database,
    application_id: &str,
    permission: &str,
) -> DbResult<PermissionGrant> {
    db.conn()
        .query_row(
            "SELECT id, application_id, permission, scope_json, status, grant_source, granted_at, revoked_at
             FROM application_permissions WHERE application_id = ?1 AND permission = ?2",
            params![application_id, permission],
            |row| {
                let scope_s: String = row.get(3)?;
                Ok(PermissionGrant {
                    id: row.get(0)?,
                    application_id: row.get(1)?,
                    permission: row.get(2)?,
                    scope: serde_json::from_str(&scope_s).unwrap_or(serde_json::json!({})),
                    status: row.get(4)?,
                    grant_source: row.get(5)?,
                    granted_at: row.get(6)?,
                    revoked_at: row.get(7)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound("permission".into()),
            other => DbError::Sqlite(other),
        })
}

pub fn list_permissions(db: &Database, application_id: &str) -> DbResult<Vec<PermissionGrant>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, application_id, permission, scope_json, status, grant_source, granted_at, revoked_at
         FROM application_permissions WHERE application_id = ?1",
    )?;
    let rows = stmt.query_map([application_id], |row| {
        let scope_s: String = row.get(3)?;
        Ok(PermissionGrant {
            id: row.get(0)?,
            application_id: row.get(1)?,
            permission: row.get(2)?,
            scope: serde_json::from_str(&scope_s).unwrap_or(serde_json::json!({})),
            status: row.get(4)?,
            grant_source: row.get(5)?,
            granted_at: row.get(6)?,
            revoked_at: row.get(7)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn assert_can_write_data(db: &Database, application_id: &str) -> DbResult<()> {
    if has_permission(db, application_id, "local_data.write")? {
        Ok(())
    } else {
        Err(DbError::Invalid(
            "permission_denied: local_data.write required".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_local_data() {
        assert!(validate_declared_permissions(&["local_data.read".into()]).is_ok());
    }

    #[test]
    fn rejects_credential() {
        assert!(validate_declared_permissions(&["credential.read".into()]).is_err());
    }

    #[test]
    fn rejects_unrestricted() {
        assert!(validate_declared_permissions(&["unrestricted.shell".into()]).is_err());
    }
}
