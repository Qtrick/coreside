//! SQLite repository for user-saved theme presets.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};

use super::snapshot::{AppearanceSnapshot, APPEARANCE_SNAPSHOT_SCHEMA_VERSION};

const MAX_NAME_LEN: usize = 80;
const MAX_DESCRIPTION_LEN: usize = 280;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemePresetRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub schema_version: u32,
    pub snapshot: AppearanceSnapshot,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePresetCapabilities {
    pub used: u32,
    pub limit: u32,
    pub can_create: bool,
    pub plan_id: String,
    pub plan_source: String,
    pub free_limit: u32,
}

pub fn normalize_preset_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("Theme name is required".into());
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(format!(
            "Theme name must be at most {MAX_NAME_LEN} characters"
        ));
    }
    // Reject control characters / newlines that break UI labels.
    if name.chars().any(|c| c.is_control()) {
        return Err("Theme name contains invalid characters".into());
    }
    Ok(name.to_string())
}

pub fn normalize_description(raw: &str) -> Result<String, String> {
    let description = raw.trim();
    if description.chars().count() > MAX_DESCRIPTION_LEN {
        return Err(format!(
            "Description must be at most {MAX_DESCRIPTION_LEN} characters"
        ));
    }
    if description.chars().any(|c| c.is_control() && c != '\n') {
        return Err("Description contains invalid characters".into());
    }
    Ok(description.to_string())
}

fn name_normalized(name: &str) -> String {
    name.trim().to_lowercase()
}

fn row_to_record(
    id: String,
    name: String,
    description: String,
    schema_version: i64,
    snapshot_json: String,
    revision: i64,
    created_at: String,
    updated_at: String,
) -> DbResult<ThemePresetRecord> {
    let snapshot = AppearanceSnapshot::parse_and_validate(&snapshot_json)
        .map_err(|e| DbError::Corrupted(format!("theme preset {id}: {e}")))?;
    if schema_version != i64::from(snapshot.schema_version) {
        return Err(DbError::Corrupted(format!(
            "theme preset {id}: stored schema version {schema_version} does not match snapshot schema version {}",
            snapshot.schema_version
        )));
    }
    Ok(ThemePresetRecord {
        id,
        name,
        description,
        schema_version: schema_version as u32,
        snapshot,
        revision,
        created_at,
        updated_at,
    })
}

pub fn count_theme_presets(db: &Database) -> DbResult<u32> {
    let n: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM theme_presets", [], |r| r.get(0))?;
    Ok(n.max(0) as u32)
}

pub fn list_theme_presets(db: &Database) -> DbResult<Vec<ThemePresetRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, description, schema_version, snapshot_json, revision, created_at, updated_at
         FROM theme_presets
         ORDER BY updated_at DESC, name COLLATE NOCASE ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (
            id,
            name,
            description,
            schema_version,
            snapshot_json,
            revision,
            created_at,
            updated_at,
        ) = row?;
        match row_to_record(
            id,
            name,
            description,
            schema_version,
            snapshot_json,
            revision,
            created_at,
            updated_at,
        ) {
            Ok(record) => out.push(record),
            Err(error) => {
                tracing::warn!(error = %error, "skipping invalid theme preset row");
            }
        }
    }
    Ok(out)
}

pub fn get_theme_preset(db: &Database, id: &str) -> DbResult<ThemePresetRecord> {
    let row = db
        .conn()
        .query_row(
            "SELECT id, name, description, schema_version, snapshot_json, revision, created_at, updated_at
             FROM theme_presets WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("theme preset {id}")))?;
    row_to_record(row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7)
}

/// Insert a new preset. Quota is enforced inside the transaction.
pub fn insert_theme_preset(
    db: &mut Database,
    name: &str,
    description: &str,
    snapshot: &AppearanceSnapshot,
    max_allowed: u32,
) -> DbResult<ThemePresetRecord> {
    let name = normalize_preset_name(name).map_err(DbError::Invalid)?;
    let description = normalize_description(description).map_err(DbError::Invalid)?;
    snapshot.validate().map_err(DbError::Invalid)?;
    let snapshot_json = snapshot.to_json_string().map_err(DbError::Invalid)?;
    let id = format!("theme-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let name_norm = name_normalized(&name);

    db.with_transaction(|conn| {
        snapshot.assert_media_assets_exist_on_conn(conn)?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM theme_presets", [], |r| r.get(0))?;
        if (count as u32) >= max_allowed {
            return Err(DbError::Invalid(format!(
                "Theme preset limit reached ({max_allowed}). Delete or update an existing theme."
            )));
        }
        let result = conn.execute(
            "INSERT INTO theme_presets
             (id, name, name_normalized, description, schema_version, snapshot_json, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)",
            rusqlite::params![
                id,
                name,
                name_norm,
                description,
                APPEARANCE_SNAPSHOT_SCHEMA_VERSION as i64,
                snapshot_json,
                now,
                now,
            ],
        );
        match result {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(DbError::Conflict(
                    "A theme with this name already exists".into(),
                ))
            }
            Err(e) => Err(DbError::from(e)),
        }
    })?;

    get_theme_preset(db, &id)
}

pub fn rename_theme_preset(
    db: &mut Database,
    id: &str,
    name: &str,
    expected_revision: i64,
) -> DbResult<ThemePresetRecord> {
    let name = normalize_preset_name(name).map_err(DbError::Invalid)?;
    let name_norm = name_normalized(&name);
    let now = now_rfc3339();

    db.with_transaction(|conn| {
        let current: Option<(i64, String)> = conn
            .query_row(
                "SELECT revision, name FROM theme_presets WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((revision, _)) = current else {
            return Err(DbError::NotFound(format!("theme preset {id}")));
        };
        if revision != expected_revision {
            return Err(DbError::Conflict(
                "Theme preset was updated elsewhere. Refresh and try again.".into(),
            ));
        }
        let result = conn.execute(
            "UPDATE theme_presets
             SET name = ?1, name_normalized = ?2, revision = revision + 1, updated_at = ?3
             WHERE id = ?4 AND revision = ?5",
            rusqlite::params![name, name_norm, now, id, expected_revision],
        );
        match result {
            Ok(0) => Err(DbError::Conflict(
                "Theme preset was updated elsewhere. Refresh and try again.".into(),
            )),
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(DbError::Conflict(
                    "A theme with this name already exists".into(),
                ))
            }
            Err(e) => Err(DbError::from(e)),
        }
    })?;
    get_theme_preset(db, id)
}

pub fn update_theme_preset_snapshot(
    db: &mut Database,
    id: &str,
    snapshot: &AppearanceSnapshot,
    expected_revision: i64,
) -> DbResult<ThemePresetRecord> {
    snapshot.validate().map_err(DbError::Invalid)?;
    let snapshot_json = snapshot.to_json_string().map_err(DbError::Invalid)?;
    let now = now_rfc3339();

    db.with_transaction(|conn| {
        snapshot.assert_media_assets_exist_on_conn(conn)?;
        let current: Option<i64> = conn
            .query_row(
                "SELECT revision FROM theme_presets WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(revision) = current else {
            return Err(DbError::NotFound(format!("theme preset {id}")));
        };
        if revision != expected_revision {
            return Err(DbError::Conflict(
                "Theme preset was updated elsewhere. Refresh and try again.".into(),
            ));
        }
        let n = conn.execute(
            "UPDATE theme_presets
             SET snapshot_json = ?1, schema_version = ?2, revision = revision + 1, updated_at = ?3
             WHERE id = ?4 AND revision = ?5",
            rusqlite::params![
                snapshot_json,
                APPEARANCE_SNAPSHOT_SCHEMA_VERSION as i64,
                now,
                id,
                expected_revision,
            ],
        )?;
        if n == 0 {
            return Err(DbError::Conflict(
                "Theme preset was updated elsewhere. Refresh and try again.".into(),
            ));
        }
        Ok(())
    })?;
    get_theme_preset(db, id)
}

pub fn delete_theme_preset(db: &mut Database, id: &str, expected_revision: i64) -> DbResult<()> {
    db.with_transaction(|conn| {
        let current: Option<i64> = conn
            .query_row(
                "SELECT revision FROM theme_presets WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(revision) = current else {
            return Err(DbError::NotFound(format!("theme preset {id}")));
        };
        if revision != expected_revision {
            return Err(DbError::Conflict(
                "Theme preset was updated elsewhere. Refresh and try again.".into(),
            ));
        }
        let n = conn.execute(
            "DELETE FROM theme_presets WHERE id = ?1 AND revision = ?2",
            rusqlite::params![id, expected_revision],
        )?;
        if n == 0 {
            return Err(DbError::Conflict(
                "Theme preset was updated elsewhere. Refresh and try again.".into(),
            ));
        }
        Ok(())
    })
}

/// Scan preset snapshots for a media asset reference (for deletion warnings).
pub fn find_theme_presets_referencing_asset(
    db: &Database,
    asset_id: &str,
) -> DbResult<Vec<(String, String)>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, name, snapshot_json FROM theme_presets")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, snapshot_json) = row?;
        if let Ok(snap) = AppearanceSnapshot::parse_and_validate(&snapshot_json) {
            if snap.referenced_asset_ids().iter().any(|a| a == asset_id) {
                out.push((id, name));
            }
        }
    }
    Ok(out)
}
