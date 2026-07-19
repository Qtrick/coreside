//! Lifecycle states + dependency edges + bounded garbage collection.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbResult};
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
