//! Tutorial / onboarding progress persistence and sample seeding.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{now_rfc3339, Database, DbError, DbResult, DEFAULT_WORKSPACE_ID};
use crate::ai::ToolDefinition;
use crate::application_kernel::recovery;

pub const ESSENTIALS_TUTORIAL_ID: &str = "coreside-essentials";
pub const TUTORIAL_SAMPLE_CONVERSATION_ID: &str = "conv-tutorial-sample";
pub const TUTORIAL_SAMPLE_TOOL_ID: &str = "tool-tutorial-sample-planner";

const VALID_STATUSES: &[&str] = &[
    "not_started",
    "in_progress",
    "completed",
    "skipped",
    "superseded",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TutorialProgress {
    pub tutorial_id: String,
    pub tutorial_version: i64,
    pub status: String,
    pub current_step_id: Option<String>,
    pub completed_step_ids: Vec<String>,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub skipped_at: Option<String>,
    pub last_opened_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingState {
    pub welcome_eligible: bool,
    pub onboarding_disabled: bool,
    pub is_secondary_window: bool,
    pub recovery_mode: bool,
    pub has_meaningful_activity: bool,
    pub progress: Vec<TutorialProgress>,
}

fn parse_completed_ids(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn map_progress_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TutorialProgress> {
    let completed_raw: String = row.get(4)?;
    Ok(TutorialProgress {
        tutorial_id: row.get(0)?,
        tutorial_version: row.get(1)?,
        status: row.get(2)?,
        current_step_id: row.get(3)?,
        completed_step_ids: parse_completed_ids(&completed_raw),
        started_at: row.get(5)?,
        updated_at: row.get(6)?,
        completed_at: row.get(7)?,
        skipped_at: row.get(8)?,
        last_opened_at: row.get(9)?,
    })
}

const PROGRESS_SELECT: &str = "SELECT tutorial_id, tutorial_version, status, current_step_id,
        completed_step_ids, started_at, updated_at, completed_at, skipped_at, last_opened_at
     FROM tutorial_progress";

pub fn list_tutorial_progress(db: &Database) -> DbResult<Vec<TutorialProgress>> {
    let mut stmt = db.conn().prepare(&format!("{PROGRESS_SELECT} ORDER BY tutorial_id"))?;
    let rows = stmt.query_map([], map_progress_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_tutorial_progress(db: &Database, tutorial_id: &str) -> DbResult<Option<TutorialProgress>> {
    db.conn()
        .query_row(
            &format!("{PROGRESS_SELECT} WHERE tutorial_id = ?1"),
            [tutorial_id],
            map_progress_row,
        )
        .optional()
        .map_err(Into::into)
}

fn validate_status(status: &str) -> DbResult<()> {
    if VALID_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(DbError::Invalid(format!("invalid tutorial status: {status}")))
    }
}

#[derive(Debug, Clone)]
pub struct UpsertTutorialProgressInput {
    pub tutorial_id: String,
    pub tutorial_version: i64,
    pub status: String,
    pub current_step_id: Option<String>,
    pub completed_step_ids: Option<Vec<String>>,
}

pub fn upsert_tutorial_progress(
    db: &mut Database,
    input: UpsertTutorialProgressInput,
) -> DbResult<TutorialProgress> {
    validate_status(&input.status)?;
    let now = now_rfc3339();
    let existing = get_tutorial_progress(db, &input.tutorial_id)?;
    let completed_ids = input
        .completed_step_ids
        .unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|e| e.completed_step_ids.clone())
                .unwrap_or_default()
        });
    let completed_json = serde_json::to_string(&completed_ids)?;

    let started_at = match existing.as_ref().and_then(|e| e.started_at.clone()) {
        Some(s) => Some(s),
        None if input.status == "in_progress" || input.status == "completed" => Some(now.clone()),
        None => None,
    };
    let completed_at = if input.status == "completed" {
        Some(
            existing
                .as_ref()
                .and_then(|e| e.completed_at.clone())
                .unwrap_or_else(|| now.clone()),
        )
    } else {
        existing.as_ref().and_then(|e| e.completed_at.clone())
    };
    let skipped_at = if input.status == "skipped" {
        Some(
            existing
                .as_ref()
                .and_then(|e| e.skipped_at.clone())
                .unwrap_or_else(|| now.clone()),
        )
    } else {
        existing.as_ref().and_then(|e| e.skipped_at.clone())
    };
    let last_opened_at = Some(now.clone());

    db.conn().execute(
        "INSERT INTO tutorial_progress (
            tutorial_id, tutorial_version, status, current_step_id, completed_step_ids,
            started_at, updated_at, completed_at, skipped_at, last_opened_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(tutorial_id) DO UPDATE SET
            tutorial_version = excluded.tutorial_version,
            status = excluded.status,
            current_step_id = excluded.current_step_id,
            completed_step_ids = excluded.completed_step_ids,
            started_at = COALESCE(tutorial_progress.started_at, excluded.started_at),
            updated_at = excluded.updated_at,
            completed_at = excluded.completed_at,
            skipped_at = excluded.skipped_at,
            last_opened_at = excluded.last_opened_at",
        params![
            input.tutorial_id,
            input.tutorial_version,
            input.status,
            input.current_step_id,
            completed_json,
            started_at,
            now,
            completed_at,
            skipped_at,
            last_opened_at,
        ],
    )?;

    get_tutorial_progress(db, &input.tutorial_id)?
        .ok_or_else(|| DbError::Invalid("tutorial progress missing after upsert".into()))
}

pub fn reset_tutorial_progress(db: &mut Database, tutorial_id: Option<&str>) -> DbResult<u64> {
    let n = if let Some(id) = tutorial_id {
        db.conn()
            .execute("DELETE FROM tutorial_progress WHERE tutorial_id = ?1", [id])?
    } else {
        db.conn().execute("DELETE FROM tutorial_progress", [])?
    };
    Ok(n as u64)
}

/// Pure eligibility for the first-run welcome dialog.
pub fn welcome_eligible(
    is_secondary_window: bool,
    onboarding_disabled: bool,
    recovery_mode: bool,
    essentials_status: Option<&str>,
    has_meaningful_activity: bool,
) -> bool {
    if is_secondary_window || onboarding_disabled || recovery_mode || has_meaningful_activity {
        return false;
    }
    // in_progress must not re-open Welcome — Continue lives in Help & learning.
    // Re-showing Welcome would let "Explore on my own" mark the tour skipped.
    match essentials_status {
        Some("completed") | Some("skipped") | Some("superseded") | Some("in_progress") => {
            false
        }
        _ => true,
    }
}

pub fn onboarding_disabled_from_env() -> bool {
    matches!(
        std::env::var("CORESIDE_DISABLE_ONBOARDING").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    ) || matches!(
        std::env::var("CORESIDE_E2E").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

pub fn has_meaningful_prior_activity(db: &Database) -> DbResult<bool> {
    let conv_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM conversations WHERE id != ?1",
        [TUTORIAL_SAMPLE_CONVERSATION_ID],
        |r| r.get(0),
    )?;
    if conv_count > 0 {
        return Ok(true);
    }
    let tool_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM tools WHERE id != ?1",
        [TUTORIAL_SAMPLE_TOOL_ID],
        |r| r.get(0),
    )?;
    Ok(tool_count > 0)
}

pub fn get_onboarding_state(
    db: &Database,
    is_secondary_window: bool,
) -> DbResult<OnboardingState> {
    let onboarding_disabled = onboarding_disabled_from_env();
    let recovery_mode = recovery::get_recovery_state(db)
        .map(|s| s.recovery_mode)
        .unwrap_or(false);
    let progress = list_tutorial_progress(db)?;
    let essentials_status = progress
        .iter()
        .find(|p| p.tutorial_id == ESSENTIALS_TUTORIAL_ID)
        .map(|p| p.status.as_str());
    let has_meaningful_activity = has_meaningful_prior_activity(db)?;
    let welcome_eligible = welcome_eligible(
        is_secondary_window,
        onboarding_disabled,
        recovery_mode,
        essentials_status,
        has_meaningful_activity,
    );
    Ok(OnboardingState {
        welcome_eligible,
        onboarding_disabled,
        is_secondary_window,
        recovery_mode,
        has_meaningful_activity,
        progress,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialSampleSeed {
    pub conversation_id: String,
    pub tool_id: String,
    pub created: bool,
}

pub fn seed_tutorial_sample(db: &mut Database) -> DbResult<TutorialSampleSeed> {
    let _ = super::ensure_default_workspace(db)?;
    let existing_conv = super::get_conversation(db, TUTORIAL_SAMPLE_CONVERSATION_ID).ok();
    let existing_tool = super::get_tool(db, TUTORIAL_SAMPLE_TOOL_ID).ok();
    if existing_conv.is_some() && existing_tool.is_some() {
        return Ok(TutorialSampleSeed {
            conversation_id: TUTORIAL_SAMPLE_CONVERSATION_ID.to_string(),
            tool_id: TUTORIAL_SAMPLE_TOOL_ID.to_string(),
            created: false,
        });
    }

    let now = now_rfc3339();
    if existing_conv.is_none() {
        db.conn().execute(
            "INSERT INTO conversations (id, workspace_id, title, project_id, pinned, archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, 0, 0, ?4, ?5)",
            params![
                TUTORIAL_SAMPLE_CONVERSATION_ID,
                DEFAULT_WORKSPACE_ID,
                "Tutorial: sample planner chat",
                now,
                now
            ],
        )?;
        let meta = serde_json::json!({ "tutorialSample": true });
        let meta_str = serde_json::to_string(&meta)?;
        let msg_id = format!("msg-tutorial-{}", Uuid::new_v4());
        db.conn().execute(
            "INSERT INTO messages (id, conversation_id, role, content, metadata, created_at)
             VALUES (?1, ?2, 'assistant', ?3, ?4, ?5)",
            params![
                msg_id,
                TUTORIAL_SAMPLE_CONVERSATION_ID,
                "This is a sample chat used by the Coreside tour. Safe to delete — it is not required after the tour.",
                meta_str,
                now
            ],
        )?;
    }

    if existing_tool.is_none() {
        let def = ToolDefinition {
            id: TUTORIAL_SAMPLE_TOOL_ID.to_string(),
            name: "Sample Planner".into(),
            description: "Tutorial sample app — safe to delete.".into(),
            layout: serde_json::json!({ "type": "single-column" }),
            components: vec![],
        };
        super::apply_tool_change(
            db,
            DEFAULT_WORKSPACE_ID,
            &def,
            "create",
            None,
            "tutorial sample",
        )?;
    }

    Ok(TutorialSampleSeed {
        conversation_id: TUTORIAL_SAMPLE_CONVERSATION_ID.to_string(),
        tool_id: TUTORIAL_SAMPLE_TOOL_ID.to_string(),
        created: true,
    })
}

pub fn cleanup_tutorial_sample(db: &mut Database) -> DbResult<u64> {
    // Hard-scoped to tutorial sample constants only — never clear_tools / broad deletes.
    let mut removed = 0u64;
    if super::get_conversation(db, TUTORIAL_SAMPLE_CONVERSATION_ID).is_ok() {
        super::delete_conversation(db, TUTORIAL_SAMPLE_CONVERSATION_ID)?;
        removed += 1;
    }
    if super::get_tool(db, TUTORIAL_SAMPLE_TOOL_ID).is_ok() {
        // Cascade deletes versions/state via FK when tools row is removed.
        let n = db
            .conn()
            .execute(
                "DELETE FROM tools WHERE id = ?1",
                [TUTORIAL_SAMPLE_TOOL_ID],
            )?;
        removed += n as u64;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_test_db() -> (Database, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tutorial.db");
        let db = Database::open_path(&path).unwrap();
        (db, dir)
    }

    #[test]
    fn upsert_and_list_progress() {
        let (mut db, _dir) = open_test_db();
        let row = upsert_tutorial_progress(
            &mut db,
            UpsertTutorialProgressInput {
                tutorial_id: ESSENTIALS_TUTORIAL_ID.into(),
                tutorial_version: 1,
                status: "in_progress".into(),
                current_step_id: Some("welcome".into()),
                completed_step_ids: Some(vec!["welcome".into()]),
            },
        )
        .unwrap();
        assert_eq!(row.status, "in_progress");
        assert_eq!(row.completed_step_ids, vec!["welcome".to_string()]);
        assert!(row.started_at.is_some());

        let listed = list_tutorial_progress(&db).unwrap();
        assert_eq!(listed.len(), 1);

        let done = upsert_tutorial_progress(
            &mut db,
            UpsertTutorialProgressInput {
                tutorial_id: ESSENTIALS_TUTORIAL_ID.into(),
                tutorial_version: 1,
                status: "completed".into(),
                current_step_id: None,
                completed_step_ids: Some(vec![
                    "welcome".into(),
                    "chat-composer".into(),
                ]),
            },
        )
        .unwrap();
        assert_eq!(done.status, "completed");
        assert!(done.completed_at.is_some());
    }

    #[test]
    fn reset_clears_rows() {
        let (mut db, _dir) = open_test_db();
        upsert_tutorial_progress(
            &mut db,
            UpsertTutorialProgressInput {
                tutorial_id: "a".into(),
                tutorial_version: 1,
                status: "skipped".into(),
                current_step_id: None,
                completed_step_ids: None,
            },
        )
        .unwrap();
        upsert_tutorial_progress(
            &mut db,
            UpsertTutorialProgressInput {
                tutorial_id: "b".into(),
                tutorial_version: 1,
                status: "not_started".into(),
                current_step_id: None,
                completed_step_ids: None,
            },
        )
        .unwrap();
        assert_eq!(reset_tutorial_progress(&mut db, Some("a")).unwrap(), 1);
        assert_eq!(list_tutorial_progress(&db).unwrap().len(), 1);
        assert_eq!(reset_tutorial_progress(&mut db, None).unwrap(), 1);
        assert!(list_tutorial_progress(&db).unwrap().is_empty());
    }

    #[test]
    fn welcome_eligibility_rules() {
        assert!(welcome_eligible(false, false, false, None, false));
        assert!(welcome_eligible(
            false,
            false,
            false,
            Some("not_started"),
            false
        ));
        assert!(!welcome_eligible(
            false,
            false,
            false,
            Some("in_progress"),
            false
        ));
        assert!(!welcome_eligible(true, false, false, None, false));
        assert!(!welcome_eligible(false, true, false, None, false));
        assert!(!welcome_eligible(false, false, true, None, false));
        assert!(!welcome_eligible(false, false, false, Some("completed"), false));
        assert!(!welcome_eligible(false, false, false, Some("skipped"), false));
        assert!(!welcome_eligible(false, false, false, None, true));
    }

    #[test]
    fn sample_seed_is_idempotent_and_cleanup_scoped() {
        let (mut db, _dir) = open_test_db();
        let first = seed_tutorial_sample(&mut db).unwrap();
        assert!(first.created);
        let second = seed_tutorial_sample(&mut db).unwrap();
        assert!(!second.created);
        assert!(!has_meaningful_prior_activity(&db).unwrap());

        // Real user conversation makes activity meaningful.
        crate::db::create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Real chat", None).unwrap();
        assert!(has_meaningful_prior_activity(&db).unwrap());

        let removed = cleanup_tutorial_sample(&mut db).unwrap();
        assert!(removed >= 1);
        assert!(crate::db::get_conversation(&db, TUTORIAL_SAMPLE_CONVERSATION_ID).is_err());
        // Real chat remains.
        let convs = crate::db::list_conversations(&db, None).unwrap();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].title, "Real chat");
    }

    #[test]
    fn rejects_invalid_status() {
        let (mut db, _dir) = open_test_db();
        let err = upsert_tutorial_progress(
            &mut db,
            UpsertTutorialProgressInput {
                tutorial_id: "x".into(),
                tutorial_version: 1,
                status: "done".into(),
                current_step_id: None,
                completed_step_ids: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
    }
}
