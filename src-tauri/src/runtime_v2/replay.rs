//! Read-only historical application state reconstruction for Replay.
//!
//! HARD SAFETY INVARIANTS:
//! - Replay operates on immutable historical checkpoints and transaction logs.
//! - Takes `&Database` only (never `&mut Database`), mathematically guaranteeing NO SQLite writes.
//! - Never calls AI provider APIs.
//! - Never invokes registered actions or Kernel capabilities.
//! - Never triggers event handlers or dispatches surface events.
//! - Reconstructed surfaces are strictly marked `is_read_only: true`.
//! - Strictly isolated to the requested `conversation_id`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::operations::AppOperation;
use super::preview_transaction::{PreviewSurfaceModel, PreviewTransaction};
use crate::db::{Database, DbError, DbResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayedSurface {
    pub surface_id: String,
    pub name: String,
    pub definition: Value,
    pub state: Value,
    pub revision: i64,
    pub is_read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayStateSnapshot {
    pub conversation_id: String,
    pub target_turn_id: Option<String>,
    pub target_transaction_id: Option<String>,
    pub surfaces: Vec<ReplayedSurface>,
    pub transaction_count: usize,
    pub operation_count: usize,
    pub read_only_banner: String,
}

/// In-memory state reconstruction at a target historical turn or transaction.
///
/// Guaranteed read-only: zero database mutation, zero side effects.
/// Fail-closed on missing conversation, corrupt definitions/packs/operations,
/// or a target that does not exist.
pub fn reconstruct_replay_state(
    db: &Database,
    conversation_id: &str,
    target_turn_id: Option<&str>,
    target_transaction_id: Option<&str>,
) -> DbResult<ReplayStateSnapshot> {
    use rusqlite::OptionalExtension;

    // 1. Verify conversation exists — never treat query errors as "missing".
    let conv_exists: bool = db
        .conn()
        .query_row(
            "SELECT 1 FROM conversations WHERE id = ?1",
            [conversation_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !conv_exists {
        return Err(DbError::NotFound(format!("conversation {conversation_id}")));
    }

    // 2. Fetch surfaces belonging to this conversation
    let mut stmt = db.conn().prepare(
        "SELECT id, name, definition_json, current_revision, capability_packs_json
         FROM surfaces WHERE conversation_id = ?1",
    )?;
    let surfaces_rows = stmt.query_map([conversation_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut preview_txn = PreviewTransaction::new(target_turn_id.unwrap_or("replay-turn"), None);

    // Initial base state per surface from historical version record
    for r in surfaces_rows {
        let (sid, _name, def_str, _cur_rev, packs_str) = r?;
        let earliest_def_res: Result<String, _> = db.conn().query_row(
            "SELECT definition_json FROM surface_versions WHERE surface_id = ?1 ORDER BY revision ASC LIMIT 1",
            [&sid],
            |row| row.get(0),
        );
        let historical_def_str = match earliest_def_res {
            Ok(s) => s,
            Err(rusqlite::Error::QueryReturnedNoRows) => def_str,
            Err(e) => {
                return Err(DbError::Invalid(format!(
                    "corrupted surface version for {sid}: {e}"
                )));
            }
        };
        let def: Value = serde_json::from_str(&historical_def_str).map_err(|e| {
            DbError::Invalid(format!("corrupted historical definition for {sid}: {e}"))
        })?;
        let packs: Vec<String> = serde_json::from_str(&packs_str)
            .map_err(|e| DbError::Invalid(format!("corrupted capability packs for {sid}: {e}")))?;

        let initial_model = PreviewSurfaceModel {
            surface_id: sid.clone(),
            tool_id: None,
            application_id: None,
            capability_packs: packs,
            definition: def,
            state: json!({}),
            base_revision: 1,
            preview_revision: 1,
        };
        preview_txn.seed_surface(initial_model);
    }

    // 3. Query applied transactions up to target point
    let mut tx_stmt = db.conn().prepare(
        "SELECT id, turn_id, operations_json, created_at
         FROM app_transactions
         WHERE conversation_id = ?1 AND status = 'applied'
         ORDER BY created_at ASC, id ASC",
    )?;
    let tx_rows = tx_stmt.query_map([conversation_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut applied_tx_count = 0usize;
    let mut applied_op_count = 0usize;
    let mut reached_target = false;
    let wants_target = target_transaction_id.is_some() || target_turn_id.is_some();

    for tx in tx_rows {
        let (tx_id, turn_id_opt, ops_json, _created_at) = tx?;
        if reached_target {
            break;
        }

        let ops: Vec<AppOperation> = serde_json::from_str(&ops_json).map_err(|e| {
            DbError::Invalid(format!("corrupted operations for transaction {tx_id}: {e}"))
        })?;
        for op in &ops {
            preview_txn
                .paint_op(op, |_| None)
                .map_err(|e| DbError::Invalid(format!("replay op failed in {tx_id}: {e}")))?;
            applied_op_count += 1;
        }
        applied_tx_count += 1;

        if let Some(target_tx) = target_transaction_id {
            if tx_id == target_tx {
                reached_target = true;
            }
        }
        if let Some(target_turn) = target_turn_id {
            if turn_id_opt.as_deref() == Some(target_turn) {
                reached_target = true;
            }
        }
    }

    if wants_target && !reached_target {
        return Err(DbError::NotFound(format!(
            "replay target not found in conversation {conversation_id}"
        )));
    }

    // 4. Construct read-only replayed surfaces (public projections only).
    let mut result_surfaces = Vec::new();
    for (sid, model) in &preview_txn.surfaces {
        let public_state = super::visibility::project_surface_state(
            &model.definition,
            &model.state,
            super::visibility::Audience::Renderer,
        );
        let public_def = super::visibility::project_definition_value(
            &model.definition,
            super::visibility::Audience::Renderer,
        );
        result_surfaces.push(ReplayedSurface {
            surface_id: sid.clone(),
            name: model
                .definition
                .get("title")
                .or_else(|| model.definition.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Replayed Surface")
                .to_string(),
            definition: public_def,
            state: public_state,
            revision: model.preview_revision,
            is_read_only: true,
        });
    }

    Ok(ReplayStateSnapshot {
        conversation_id: conversation_id.to_string(),
        target_turn_id: target_turn_id.map(|s| s.to_string()),
        target_transaction_id: target_transaction_id.map(|s| s.to_string()),
        surfaces: result_surfaces,
        transaction_count: applied_tx_count,
        operation_count: applied_op_count,
        read_only_banner: "Viewing historical turn state. Interactions are disabled.".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let dir = tempfile::tempdir().unwrap();
        Database::open_path(&dir.path().join("replay_test.db")).unwrap()
    }

    #[test]
    fn replay_reconstructs_state_without_database_mutation() {
        let mut db = test_db();
        let conv = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Replay Test",
            None,
        )
        .unwrap();

        let surf = crate::runtime_v2::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Task Board",
            &json!({
                "type": "container",
                "id": "root",
                "title": "Task Board",
                "stateContracts": [
                    {
                        "key": "count",
                        "type": "integer",
                        "initialValue": 0,
                        "writePolicy": "model"
                    }
                ]
            }),
            &[],
        )
        .unwrap();

        // Transaction 1: Add task count = 1
        let op1 = AppOperation {
            id: "op-1".into(),
            op_type: "state.set".into(),
            target: crate::runtime_v2::operations::OperationTarget {
                surface_id: Some(surf.id.clone()),
                conversation_id: Some(conv.id.clone()),
                ..Default::default()
            },
            payload: json!({"count": 1}),
            ..Default::default()
        };
        db.conn()
            .execute(
                "INSERT INTO app_transactions (id, conversation_id, turn_id, status, operations_json, created_at)
                 VALUES ('tx-1', ?1, 'turn-1', 'applied', ?2, '2026-09-01T10:00:00Z')",
                rusqlite::params![conv.id, serde_json::to_string(&vec![op1]).unwrap()],
            )
            .unwrap();

        // Transaction 2: Add task count = 2
        let op2 = AppOperation {
            id: "op-2".into(),
            op_type: "state.set".into(),
            target: crate::runtime_v2::operations::OperationTarget {
                surface_id: Some(surf.id.clone()),
                conversation_id: Some(conv.id.clone()),
                ..Default::default()
            },
            payload: json!({"count": 2}),
            ..Default::default()
        };
        db.conn()
            .execute(
                "INSERT INTO app_transactions (id, conversation_id, turn_id, status, operations_json, created_at)
                 VALUES ('tx-2', ?1, 'turn-2', 'applied', ?2, '2026-09-01T10:01:00Z')",
                rusqlite::params![conv.id, serde_json::to_string(&vec![op2]).unwrap()],
            )
            .unwrap();

        // Capture DB row counts before replay
        let tx_count_before: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM app_transactions", [], |r| r.get(0))
            .unwrap();

        // 1. Reconstruct at turn-1
        let snap_turn1 = reconstruct_replay_state(&db, &conv.id, Some("turn-1"), None).unwrap();
        assert_eq!(snap_turn1.transaction_count, 1);
        assert_eq!(snap_turn1.operation_count, 1);
        let s1 = snap_turn1
            .surfaces
            .iter()
            .find(|s| s.surface_id == surf.id)
            .unwrap();
        assert_eq!(s1.state.get("count").and_then(|v| v.as_i64()), Some(1));
        assert!(s1.is_read_only);

        // 2. Reconstruct at turn-2
        let snap_turn2 = reconstruct_replay_state(&db, &conv.id, Some("turn-2"), None).unwrap();
        assert_eq!(snap_turn2.transaction_count, 2);
        assert_eq!(snap_turn2.operation_count, 2);
        let s2 = snap_turn2
            .surfaces
            .iter()
            .find(|s| s.surface_id == surf.id)
            .unwrap();
        assert_eq!(s2.state.get("count").and_then(|v| v.as_i64()), Some(2));
        assert!(s2.is_read_only);

        // 3. Invariant: Database must NOT have been mutated
        let tx_count_after: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM app_transactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tx_count_before, tx_count_after);
    }

    #[test]
    fn replay_isolates_conversations() {
        let mut db = test_db();
        let conv_a = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Conv A",
            None,
        )
        .unwrap();
        let conv_b = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Conv B",
            None,
        )
        .unwrap();

        let surf_a = crate::runtime_v2::create_inline_surface(
            &mut db,
            &conv_a.id,
            None,
            None,
            "Surface A",
            &json!({"type":"container","id":"root"}),
            &[],
        )
        .unwrap();

        // Transaction in Conv A
        let op_a = AppOperation {
            id: "op-a".into(),
            op_type: "state.set".into(),
            target: crate::runtime_v2::operations::OperationTarget {
                surface_id: Some(surf_a.id.clone()),
                conversation_id: Some(conv_a.id.clone()),
                ..Default::default()
            },
            payload: json!({"secret_data": "hidden_a"}),
            ..Default::default()
        };
        db.conn()
            .execute(
                "INSERT INTO app_transactions (id, conversation_id, turn_id, status, operations_json, created_at)
                 VALUES ('tx-a', ?1, 'turn-a', 'applied', ?2, '2026-09-01T10:00:00Z')",
                rusqlite::params![conv_a.id, serde_json::to_string(&vec![op_a]).unwrap()],
            )
            .unwrap();

        // Replay Conv B must NEVER see Conv A surfaces or data
        let snap_b = reconstruct_replay_state(&db, &conv_b.id, None, None).unwrap();
        assert_eq!(snap_b.surfaces.len(), 0);
        assert_eq!(snap_b.transaction_count, 0);
        assert_eq!(snap_b.operation_count, 0);
    }

    #[test]
    fn replay_missing_target_returns_not_found() {
        let mut db = test_db();
        let conv = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Missing Target",
            None,
        )
        .unwrap();
        let _surf = crate::runtime_v2::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Board",
            &json!({"type":"container","id":"root"}),
            &[],
        )
        .unwrap();

        let err_turn =
            reconstruct_replay_state(&db, &conv.id, Some("turn-does-not-exist"), None).unwrap_err();
        assert!(
            matches!(err_turn, DbError::NotFound(ref m) if m.contains("replay target")),
            "{err_turn:?}"
        );

        let err_tx =
            reconstruct_replay_state(&db, &conv.id, None, Some("tx-does-not-exist")).unwrap_err();
        assert!(
            matches!(err_tx, DbError::NotFound(ref m) if m.contains("replay target")),
            "{err_tx:?}"
        );
    }

    #[test]
    fn replay_corrupt_operations_json_returns_invalid() {
        let mut db = test_db();
        let conv = crate::db::create_conversation(
            &mut db,
            crate::db::DEFAULT_WORKSPACE_ID,
            "Corrupt Ops",
            None,
        )
        .unwrap();
        let _surf = crate::runtime_v2::create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Board",
            &json!({"type":"container","id":"root"}),
            &[],
        )
        .unwrap();

        db.conn()
            .execute(
                "INSERT INTO app_transactions (id, conversation_id, turn_id, status, operations_json, created_at)
                 VALUES ('tx-bad', ?1, 'turn-1', 'applied', 'NOT-JSON{{{', '2026-09-01T10:00:00Z')",
                rusqlite::params![conv.id],
            )
            .unwrap();

        let err = reconstruct_replay_state(&db, &conv.id, Some("turn-1"), None).unwrap_err();
        assert!(
            matches!(err, DbError::Invalid(ref m) if m.contains("corrupted operations")),
            "{err:?}"
        );
    }
}
