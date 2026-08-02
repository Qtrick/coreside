//! Lifecycle states + dependency edges + bounded garbage collection.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

pub const LIFECYCLE_STATES: &[&str] = &[
    "draft",
    "preview",
    "active",
    "suspended",
    "archived",
    "deleted_pending_cleanup",
    "failed",
    "disabled",
    "restored",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyEdge {
    pub source_type: String,
    pub source_id: String,
    pub target_type: String,
    pub target_id: String,
    pub relationship_type: String,
}

pub fn add_dependency(db: &mut Database, edge: DependencyEdge) -> DbResult<()> {
    db.conn().execute(
        "INSERT INTO application_dependencies (
            id, source_type, source_id, target_type, target_id, relationship_type, created_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            format!("dep-{}", Uuid::new_v4()),
            edge.source_type,
            edge.source_id,
            edge.target_type,
            edge.target_id,
            edge.relationship_type,
            now_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn deletion_warnings(db: &Database, target_type: &str, target_id: &str) -> Vec<String> {
    super::impact::dependency_impact(db, target_type, target_id)
}

/// Bounded GC — never deletes user data, LKG, credentials, or active versions.
pub fn garbage_collect(db: &mut Database) -> DbResult<u64> {
    let mut removed = 0u64;
    // Orphaned preview diagnostics older cleanup is soft: surface lifecycle draft previews
    removed += db.conn().execute(
        "DELETE FROM application_health_events WHERE created_at < datetime('now', '-30 day')",
        [],
    )? as u64;
    // Superseded package rows marked failed
    removed += db.conn().execute(
        "DELETE FROM application_packages WHERE status = 'failed' AND created_at < datetime('now', '-7 day')",
        [],
    )? as u64;
    Ok(removed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRecord {
    pub id: String,
    pub application_id: Option<String>,
    pub turn_id: Option<String>,
    pub job_type: String,
    pub status: String,
    pub progress: f64,
    pub current_stage: Option<String>,
    pub error_category: Option<String>,
}

pub fn create_job(
    db: &mut Database,
    application_id: Option<&str>,
    turn_id: Option<&str>,
    job_type: &str,
) -> DbResult<JobRecord> {
    let id = format!("job-{}", Uuid::new_v4());
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO application_jobs (
            id, application_id, turn_id, job_type, status, progress, current_stage, created_at, updated_at
         ) VALUES (?1,?2,?3,?4,'pending',0,'queued',?5,?5)",
        params![id, application_id, turn_id, job_type, now],
    )?;
    get_job(db, &id)
}

pub fn get_job(db: &Database, id: &str) -> DbResult<JobRecord> {
    db.conn()
        .query_row(
            "SELECT id, application_id, turn_id, job_type, status, progress, current_stage, error_category
             FROM application_jobs WHERE id = ?1",
            [id],
            |row| {
                Ok(JobRecord {
                    id: row.get(0)?,
                    application_id: row.get(1)?,
                    turn_id: row.get(2)?,
                    job_type: row.get(3)?,
                    status: row.get(4)?,
                    progress: row.get(5)?,
                    current_stage: row.get(6)?,
                    error_category: row.get(7)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                crate::db::DbError::NotFound(format!("job {id}"))
            }
            other => crate::db::DbError::Sqlite(other),
        })
}

pub fn interrupt_active_jobs(db: &mut Database) -> DbResult<u64> {
    // On restart: mark in-flight jobs interrupted; do not auto-resume provider calls
    let n = db.conn().execute(
        "UPDATE application_jobs SET status = 'interrupted', error_category = 'cancelled', updated_at = ?1
         WHERE status IN ('pending','running')",
        [now_rfc3339()],
    )?;
    Ok(n as u64)
}

/// Turn an application off or back on. Disabling also revokes remembered
/// runtime action grants so a re-enabled application starts by asking again.
pub fn set_application_enabled(
    db: &mut Database,
    application_id: &str,
    enabled: bool,
) -> DbResult<()> {
    crate::security::assert_not_protected(application_id).map_err(crate::db::DbError::Invalid)?;
    let n = db.conn().execute(
        "UPDATE application_manifests SET
            disabled = ?2,
            lifecycle_state = CASE WHEN ?2 = 1 THEN 'disabled' ELSE 'active' END,
            updated_at = ?3
         WHERE application_id = ?1",
        params![application_id, if enabled { 0 } else { 1 }, now_rfc3339()],
    )?;
    if n == 0 {
        return Err(crate::db::DbError::NotFound(format!(
            "application {application_id}"
        )));
    }
    if !enabled {
        let _ =
            super::registered_actions::grants::revoke_grants_for_application(db, application_id)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildFailure {
    pub id: String,
    pub application_id: String,
    pub safe_message: String,
    pub retryable: bool,
    pub request_ref: Option<String>,
    pub created_at: String,
}

/// Record why a build failed, using a user-safe message only.
pub fn record_build_failure(
    db: &mut Database,
    application_id: &str,
    message: &str,
    retryable: bool,
    request_ref: Option<&str>,
) -> DbResult<BuildFailure> {
    let id = format!("buildfail-{}", Uuid::new_v4());
    let safe: String = crate::security::sanitize_error(message, None)
        .chars()
        .take(300)
        .collect();
    db.conn().execute(
        "INSERT INTO application_build_failures (
            id, application_id, safe_message, retryable, request_ref, created_at
         ) VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            id,
            application_id,
            safe,
            if retryable { 1 } else { 0 },
            request_ref,
            now_rfc3339()
        ],
    )?;
    // A non-retryable failure is the crash signal that drives the documented
    // "three strikes and the application is suspended" recovery path. Without
    // this, `crash_count` never leaves zero and safe startup can never suspend
    // a repeatedly failing application.
    //
    // Tool renderers may report with a tool id that is not yet a kernel
    // application; keep the failure row and skip the strike rather than
    // failing the whole IPC call.
    if !retryable {
        match crate::application_kernel::manifest::record_crash(db, application_id) {
            Ok(_) => {}
            Err(DbError::NotFound(_)) => {}
            Err(err) => return Err(err),
        }
    }
    Ok(BuildFailure {
        id,
        application_id: application_id.into(),
        safe_message: safe,
        retryable,
        request_ref: request_ref.map(|s| s.to_string()),
        created_at: now_rfc3339(),
    })
}

pub fn clear_build_failures(db: &mut Database, application_id: &str) -> DbResult<u64> {
    let n = db.conn().execute(
        "UPDATE application_build_failures SET cleared_at = ?2
         WHERE application_id = ?1 AND cleared_at IS NULL",
        params![application_id, now_rfc3339()],
    )?;
    Ok(n as u64)
}

pub fn list_build_failures(db: &Database, application_id: &str) -> DbResult<Vec<BuildFailure>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, application_id, safe_message, retryable, request_ref, created_at
         FROM application_build_failures
         WHERE application_id = ?1 AND cleared_at IS NULL
         ORDER BY created_at DESC LIMIT 20",
    )?;
    let rows = stmt.query_map([application_id], |row| {
        Ok(BuildFailure {
            id: row.get(0)?,
            application_id: row.get(1)?,
            safe_message: row.get(2)?,
            retryable: row.get::<_, i64>(3)? != 0,
            request_ref: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationVersion {
    pub version: i64,
    pub validation_status: String,
    pub test_status: String,
    pub is_known_good: bool,
    pub created_at: String,
}

pub fn list_versions(db: &Database, application_id: &str) -> DbResult<Vec<ApplicationVersion>> {
    let mut stmt = db.conn().prepare(
        "SELECT version, validation_status, test_status, is_known_good, created_at
         FROM application_manifest_versions
         WHERE application_id = ?1 ORDER BY version DESC LIMIT 100",
    )?;
    let rows = stmt.query_map([application_id], |row| {
        Ok(ApplicationVersion {
            version: row.get(0)?,
            validation_status: row.get(1)?,
            test_status: row.get(2)?,
            is_known_good: row.get::<_, i64>(3)? != 0,
            created_at: row.get(4)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn update_job_progress(
    db: &mut Database,
    id: &str,
    status: &str,
    progress: f64,
    stage: Option<&str>,
) -> DbResult<JobRecord> {
    db.conn().execute(
        "UPDATE application_jobs SET status = ?2, progress = ?3, current_stage = ?4, updated_at = ?5
         WHERE id = ?1",
        params![id, status, progress, stage, now_rfc3339()],
    )?;
    get_job(db, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use tempfile::tempdir;

    #[test]
    fn non_retryable_failure_without_manifest_still_records() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("orphan.db")).unwrap();
        let failure = record_build_failure(
            &mut db,
            "tool-without-manifest",
            "Error: render blew up",
            false,
            Some("tool-without-manifest"),
        )
        .expect("failure row must succeed even without a kernel app");
        assert!(!failure.retryable);
        assert_eq!(
            list_build_failures(&db, "tool-without-manifest")
                .unwrap()
                .len(),
            1
        );
    }
}
