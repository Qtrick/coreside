//! Cross-surface application transactions with atomic apply and undo.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::operations::{validate_operations, AppOperation};
use super::packs::validate_definition_components;
use super::patch::apply_component_op;
use super::surfaces::{
    create_inline_surface, get_surface, update_surface_definition, SurfaceRecord,
};
use crate::ai::{ToolComponent, ToolDefinition};
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::params;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppTransactionRecord {
    pub id: String,
    pub turn_id: Option<String>,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub summary: String,
    pub status: String,
    pub operations: Vec<AppOperation>,
    pub silent: bool,
    pub created_at: String,
    pub applied_at: Option<String>,
    pub reverted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub transaction: AppTransactionRecord,
    pub surfaces: Vec<SurfaceRecord>,
    pub conflicts: Vec<String>,
}

pub fn create_transaction(
    db: &mut Database,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    turn_id: Option<&str>,
    summary: &str,
    operations: &[AppOperation],
    silent: bool,
) -> DbResult<AppTransactionRecord> {
    validate_operations(operations).map_err(DbError::Invalid)?;
    let id = format!("txn-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let ops_json = serde_json::to_string(operations)?;
    db.conn().execute(
        "INSERT INTO app_transactions (
            id, turn_id, conversation_id, project_id, summary, status,
            operations_json, silent, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7, ?8)",
        params![
            id,
            turn_id,
            conversation_id,
            project_id,
            summary,
            ops_json,
            if silent { 1 } else { 0 },
            now
        ],
    )?;
    for (i, op) in operations.iter().enumerate() {
        db.conn().execute(
            "INSERT INTO app_operations (
                id, transaction_id, sequence, op_type, target_json, base_revision,
                payload_json, validation_status, apply_status, idempotency_key, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', 'pending', ?8, ?9)",
            params![
                op.id,
                id,
                i as i64,
                op.op_type,
                serde_json::to_string(&op.target)?,
                op.base_revision,
                serde_json::to_string(&op.payload)?,
                op.idempotency_key,
                now
            ],
        )?;
    }
    get_transaction(db, &id)
}

pub fn get_transaction(db: &Database, id: &str) -> DbResult<AppTransactionRecord> {
    db.conn()
        .query_row(
            "SELECT id, turn_id, conversation_id, project_id, summary, status,
                    operations_json, silent, created_at, applied_at, reverted_at
             FROM app_transactions WHERE id = ?1",
            [id],
            |row| {
                let ops_json: String = row.get(6)?;
                let operations: Vec<AppOperation> =
                    serde_json::from_str(&ops_json).unwrap_or_default();
                Ok(AppTransactionRecord {
                    id: row.get(0)?,
                    turn_id: row.get(1)?,
                    conversation_id: row.get(2)?,
                    project_id: row.get(3)?,
                    summary: row.get(4)?,
                    status: row.get(5)?,
                    operations,
                    silent: row.get::<_, i64>(7)? != 0,
                    created_at: row.get(8)?,
                    applied_at: row.get(9)?,
                    reverted_at: row.get(10)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("transaction {id}")),
            other => DbError::Sqlite(other),
        })
}

pub fn apply_transaction(db: &mut Database, transaction_id: &str) -> DbResult<ApplyResult> {
    apply_transaction_with_bus(db, transaction_id, None)
}

pub fn apply_transaction_with_bus(
    db: &mut Database,
    transaction_id: &str,
    mut bus: Option<&mut super::events::EventBus>,
) -> DbResult<ApplyResult> {
    let txn = get_transaction(db, transaction_id)?;
    if txn.status == "applied" {
        return Ok(ApplyResult {
            transaction: txn,
            surfaces: vec![],
            conflicts: vec![],
        });
    }
    if txn.status == "reverted" || txn.status == "failed" {
        return Err(DbError::Invalid(format!(
            "cannot apply a {} transaction",
            txn.status
        )));
    }

    let mut previous = json!({});
    let mut surfaces = Vec::new();
    let mut conflicts = Vec::new();

    // Snapshot affected surfaces first
    for op in &txn.operations {
        if let Some(sid) = op.target.surface_id.as_ref() {
            if let Ok(s) = get_surface(db, sid) {
                previous
                    .as_object_mut()
                    .unwrap()
                    .insert(sid.clone(), json!({ "revision": s.current_revision, "definition": s.definition }));
            }
        }
    }

    db.conn()
        .execute_batch("SAVEPOINT runtime_v2_apply")
        .map_err(DbError::Sqlite)?;

    let mut failed = false;
    for op in &txn.operations {
        match apply_one(db, op, &txn, bus.as_deref_mut()) {
            Ok(Some(s)) => surfaces.push(s),
            Ok(None) => {}
            Err(e) => {
                // Stale revision → structured conflict for multiwindow UX
                let msg = if e.contains("revision") || e.contains("stale") {
                    format!("revision_conflict:{}:{e}", op.id)
                } else {
                    format!("{}: {e}", op.id)
                };
                conflicts.push(msg);
                failed = true;
                break;
            }
        }
    }

    if failed {
        db.conn()
            .execute_batch("ROLLBACK TO SAVEPOINT runtime_v2_apply; RELEASE SAVEPOINT runtime_v2_apply")
            .map_err(DbError::Sqlite)?;
        surfaces.clear();
        db.conn().execute(
            "UPDATE app_transactions SET status = 'failed', previous_snapshot_json = ?1 WHERE id = ?2",
            params![previous.to_string(), transaction_id],
        )?;
        return Ok(ApplyResult {
            transaction: get_transaction(db, transaction_id)?,
            surfaces,
            conflicts,
        });
    }

    let now = now_rfc3339();
    let result = json!({ "surfaces": surfaces.iter().map(|s| &s.id).collect::<Vec<_>>() });
    db.conn().execute(
        "UPDATE app_transactions SET status = 'applied', applied_at = ?1,
         previous_snapshot_json = ?2, result_snapshot_json = ?3 WHERE id = ?4",
        params![now, previous.to_string(), result.to_string(), transaction_id],
    )?;
    db.conn()
        .execute_batch("RELEASE SAVEPOINT runtime_v2_apply")
        .map_err(DbError::Sqlite)?;

    Ok(ApplyResult {
        transaction: get_transaction(db, transaction_id)?,
        surfaces,
        conflicts,
    })
}

fn apply_one(
    db: &mut Database,
    op: &AppOperation,
    txn: &AppTransactionRecord,
    bus: Option<&mut super::events::EventBus>,
) -> Result<Option<SurfaceRecord>, String> {
    match op.op_type.as_str() {
        "chat.inline_surface_create" => {
            let conversation_id = op
                .target
                .conversation_id
                .as_deref()
                .or(txn.conversation_id.as_deref())
                .ok_or_else(|| "conversationId required".to_string())?;
            let name = op
                .payload
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Inline surface");
            let definition = op
                .payload
                .get("definition")
                .cloned()
                .unwrap_or_else(|| op.payload.clone());
            validate_definition_components(&definition)?;
            let packs: Vec<String> = op
                .payload
                .get("capabilityPacks")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let s = create_inline_surface(
                db,
                conversation_id,
                op.target.message_id.as_deref(),
                op.target.project_id.as_deref().or(txn.project_id.as_deref()),
                name,
                &definition,
                &packs,
            )
            .map_err(|e| e.to_string())?;
            Ok(Some(s))
        }
        "chat.inline_surface_update" | "component.update_props" | "component.insert"
        | "component.remove" | "component.replace" | "component.move"
        | "component.update_children" | "component.update_visibility"
        | "component.update_actions" => {
            let sid = op
                .target
                .surface_id
                .as_deref()
                .ok_or_else(|| "surfaceId required".to_string())?;
            let surface = get_surface(db, sid).map_err(|e| e.to_string())?;
            let mut def_value = surface.definition.clone();
            // Definition may be a ToolDefinition-shaped object
            let mut components: Vec<ToolComponent> = def_value
                .get("components")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            if op.op_type.starts_with("component.") {
                apply_component_op(
                    &mut components,
                    &op.op_type,
                    op.target.component_id.as_deref(),
                    op.target.parent_id.as_deref(),
                    op.base_revision,
                    surface.current_revision,
                    &op.payload,
                )
                .map_err(|e| e.to_string())?;
                if let Some(obj) = def_value.as_object_mut() {
                    obj.insert("components".into(), serde_json::to_value(&components).unwrap());
                }
            } else if let Some(definition) = op.payload.get("definition") {
                def_value = definition.clone();
            }
            let s = update_surface_definition(
                db,
                sid,
                &def_value,
                op.payload
                    .get("changeSummary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("patch"),
                op.base_revision,
            )
            .map_err(|e| e.to_string())?;
            Ok(Some(s))
        }
        "surface.create" | "tool.full_replace" => {
            let tool: ToolDefinition = serde_json::from_value(
                op.payload
                    .get("tool")
                    .cloned()
                    .unwrap_or_else(|| op.payload.clone()),
            )
            .map_err(|e| e.to_string())?;
            crate::security::assert_not_protected(&tool.id)?;
            super::packs::validate_tool_components(&tool.components)?;
            let workspace = "ws-personal-default";
            let action = op
                .payload
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or(if op.op_type == "surface.create" {
                    "create"
                } else {
                    "replace"
                });
            let target = op
                .payload
                .get("targetToolId")
                .and_then(|v| v.as_str())
                .or(op.target.tool_id.as_deref());
            let applied = crate::db::apply_tool_change(
                db,
                workspace,
                &tool,
                action,
                target,
                op.payload
                    .get("changeSummary")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )
            .map_err(|e| e.to_string())?;
            let s = super::surfaces::upsert_surface_from_tool(
                db,
                &applied.definition,
                workspace,
                applied.current_version,
            )
            .map_err(|e| e.to_string())?;
            Ok(Some(s))
        }
        "surface.promote" => {
            let sid = op
                .target
                .surface_id
                .as_deref()
                .ok_or_else(|| "surfaceId required".to_string())?;
            let s = super::surfaces::promote_inline_to_tool(db, sid, "ws-personal-default")
                .map_err(|e| e.to_string())?;
            Ok(Some(s))
        }
        "state.set" | "state.patch" => {
            let sid = op
                .target
                .surface_id
                .as_deref()
                .ok_or_else(|| "surfaceId required".to_string())?;
            let state = op.payload.get("state").cloned().unwrap_or(op.payload.clone());
            super::surfaces::save_surface_state(db, sid, &state).map_err(|e| e.to_string())?;
            Ok(Some(get_surface(db, sid).map_err(|e| e.to_string())?))
        }
        "chat.status" | "chat.notification" | "setting.create" | "setting.update"
        | "setting.delete" | "layout.update" | "layout.add_panel" | "layout.move_panel"
        | "layout.resize_panel" | "layout.remove_panel" | "layout.set_visibility"
        | "wallpaper.apply" | "wallpaper.create" | "wallpaper.delete" | "automation.create"
        | "automation.update" | "automation.pause" | "automation.resume" | "export.prepare"
        | "project.panel_create" | "project.panel_update" | "chat.branch_create"
        | "surface.update_metadata" | "surface.move" | "surface.duplicate"
        | "surface.archive" | "surface.restore" | "surface.delete"
        | "chat.inline_surface_remove" | "state.reset" | "state.delete_key"
        | "route.navigate" | "manifest.upsert" | "manifest.disable"
        | "manifest.restore_last_known_good" | "data.model_upsert" | "data.record_create"
        | "data.record_update" | "data.record_delete" | "data.migrate" | "permission.request"
        | "test.upsert" | "test.run" | "package.export" | "package.import" => {
            // Handled by Application Kernel or recorded for replay.
            Ok(None)
        }
        "subscription.create" => {
            let owner = op
                .target
                .surface_id
                .as_deref()
                .ok_or_else(|| "surfaceId required for subscription".to_string())?;
            let sub_id = op
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("sub-{}", Uuid::new_v4()));
            let event_types: Vec<String> = op
                .payload
                .get("eventTypes")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let source_filter: super::events::EventRef = op
                .payload
                .get("sourceFilter")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let target: super::events::EventRef = op
                .payload
                .get("target")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let sub = super::events::Subscription {
                id: sub_id.clone(),
                owner_surface_id: owner.into(),
                event_types: event_types.clone(),
                source_filter: source_filter.clone(),
                target: target.clone(),
                enabled: true,
            };
            if let Some(bus) = bus {
                bus.add_subscription(sub.clone()).map_err(|e| e.to_string())?;
            }
            db.conn()
                .execute(
                    "INSERT OR REPLACE INTO surface_subscriptions (
                        id, owner_surface_id, source_filter_json, target_json,
                        event_types_json, enabled, created_at, updated_at
                     ) VALUES (?1,?2,?3,?4,?5,1,datetime('now'),datetime('now'))",
                    params![
                        sub_id,
                        owner,
                        serde_json::to_string(&source_filter).unwrap_or_else(|_| "{}".into()),
                        serde_json::to_string(&target).unwrap_or_else(|_| "{}".into()),
                        serde_json::to_string(&event_types).unwrap_or_else(|_| "[]".into())
                    ],
                )
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        "subscription.update" | "subscription.delete" => {
            let sid = op
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "subscription id required".to_string())?;
            if op.op_type == "subscription.delete" {
                db.conn()
                    .execute("DELETE FROM surface_subscriptions WHERE id = ?1", [sid])
                    .map_err(|e| e.to_string())?;
                if let Some(bus) = bus {
                    bus.remove_subscription(sid);
                }
            } else {
                let enabled = op
                    .payload
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                db.conn()
                    .execute(
                        "UPDATE surface_subscriptions SET enabled = ?2, updated_at = datetime('now') WHERE id = ?1",
                        params![sid, if enabled { 1 } else { 0 }],
                    )
                    .map_err(|e| e.to_string())?;
                if let Some(bus) = bus {
                    bus.set_subscription_enabled(sid, enabled);
                }
            }
            Ok(None)
        }
        "event.dispatch" => {
            let event_type = op
                .payload
                .get("eventType")
                .and_then(|v| v.as_str())
                .unwrap_or("custom")
                .to_string();
            let ev = super::events::SurfaceEvent {
                id: format!("evt-{}", Uuid::new_v4()),
                event_type: event_type.clone(),
                scope: op
                    .payload
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or("surface")
                    .into(),
                source: super::events::EventRef {
                    surface_id: op.target.surface_id.clone(),
                    tool_id: op.target.tool_id.clone(),
                    conversation_id: op.target.conversation_id.clone(),
                    project_id: op.target.project_id.clone(),
                    component_id: op.target.component_id.clone(),
                },
                target: Default::default(),
                payload: op.payload.get("payload").cloned().unwrap_or(json!({})),
                idempotency_key: op.idempotency_key.clone(),
            };
            if let Some(bus) = bus {
                let _matched = bus.dispatch(&ev, 0).map_err(|e| e.to_string())?;
            }
            db.conn()
                .execute(
                    "INSERT INTO surface_events (
                        id, source_json, target_json, scope, event_type, payload_json,
                        idempotency_key, status, created_at, processed_at
                     ) VALUES (?1,?2,'{}',?3,?4,?5,?6,'processed',datetime('now'),datetime('now'))",
                    params![
                        ev.id,
                        serde_json::to_string(&ev.source).unwrap_or_else(|_| "{}".into()),
                        ev.scope,
                        ev.event_type,
                        ev.payload.to_string(),
                        ev.idempotency_key
                    ],
                )
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        other => Err(format!("unsupported operation in apply: {other}")),
    }
}

pub fn undo_transaction(db: &mut Database, transaction_id: &str) -> DbResult<AppTransactionRecord> {
    let txn = get_transaction(db, transaction_id)?;
    if txn.status != "applied" {
        return Err(DbError::Invalid("only applied transactions can be undone".into()));
    }
    let prev_json: Option<String> = db
        .conn()
        .query_row(
            "SELECT previous_snapshot_json FROM app_transactions WHERE id = ?1",
            [transaction_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .unwrap_or(None);
    if let Some(prev) = prev_json {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&prev) {
            for (sid, snap) in map {
                if let Some(def) = snap.get("definition") {
                    let _ = update_surface_definition(
                        db,
                        &sid,
                        def,
                        "undo transaction",
                        None,
                    );
                }
            }
        }
    }
    let now = now_rfc3339();
    db.conn().execute(
        "UPDATE app_transactions SET status = 'reverted', reverted_at = ?1 WHERE id = ?2",
        params![now, transaction_id],
    )?;
    get_transaction(db, transaction_id)
}

/// List recent transactions for a conversation (replay / undo UI).
pub fn list_transactions(
    db: &Database,
    conversation_id: &str,
    limit: usize,
) -> DbResult<Vec<AppTransactionRecord>> {
    let mut stmt = db.conn().prepare(
        "SELECT id FROM app_transactions WHERE conversation_id = ?1
         ORDER BY created_at DESC LIMIT ?2",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![conversation_id, limit as i64], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter().map(|id| get_transaction(db, &id)).collect()
}
