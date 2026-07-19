//! Operation provenance — durable audit without chain-of-thought or secrets.

use crate::db::{now_rfc3339, Database, DbResult};
use rusqlite::params;
use uuid::Uuid;

use super::{ChangeRequest, COMPILER_VERSION};

pub fn record_provenance(
    db: &mut Database,
    transaction_id: &str,
    req: &ChangeRequest,
    validation_status: &str,
    test_status: Option<&str>,
    recovery_snapshot_id: Option<&str>,
) -> DbResult<()> {
    let id = format!("prov-{}", Uuid::new_v4());
    let now = now_rfc3339();
    db.conn().execute(
        "INSERT INTO operation_provenance (
            id, transaction_id, turn_id, conversation_id, project_id, source_type,
            provider, model, prompt_version, protocol_version, compiler_version,
            approval_status, approval_at, apply_at, validation_status, test_status,
            recovery_snapshot_id, citations_json, created_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,'[]',?18)
         ON CONFLICT(transaction_id) DO UPDATE SET
            validation_status = excluded.validation_status,
            test_status = excluded.test_status,
            apply_at = excluded.apply_at",
        params![
            id,
            transaction_id,
            req.turn_id,
            req.conversation_id,
            req.project_id,
            req.source_type,
            req.provider,
            req.model,
            "system-prompts-v1",
            "2",
            COMPILER_VERSION,
            if req.approval_granted {
                "approved"
            } else {
                "auto"
            },
            now,
            now,
            validation_status,
            test_status,
            recovery_snapshot_id,
            now
        ],
    )?;
    Ok(())
}
