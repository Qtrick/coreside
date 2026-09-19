//! Cross-surface application transactions with atomic apply and undo.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::operations::{validate_operations, AppOperation, Audience};
use super::packs::{validate_definition_components, validate_tool_components_for_packs};
use super::patch::{apply_component_op, find_component, find_component_mut};
use super::preservation::{
    apply_preservation_on_replace, invalidate_component_live_state, resolve_policy_for_apply,
    upsert_preservation,
};
use super::surfaces::{
    archive_surface, create_inline_surface, delete_surface, get_surface, restore_surface,
    update_surface_definition, DeleteSurfaceOptions, SurfaceRecord,
};
use crate::ai::{ToolComponent, ToolDefinition};
use crate::db::{now_rfc3339, Database, DbError, DbResult};
use rusqlite::{params, OptionalExtension};

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
    let mut deferred: Vec<super::outbox::DeferredBusEffect> = Vec::new();
    let result = apply_transaction_deferred(db, transaction_id, &mut deferred)?;
    // Only mutate the live EventBus after the authoritative SQLite work succeeded.
    if result.transaction.status == "applied" && result.conflicts.is_empty() {
        for effect in &deferred {
            if let Some(bus) = bus.as_deref_mut() {
                if let Err(err) = super::outbox::apply_deferred_effect(bus, effect) {
                    tracing::warn!(error = %err, "deferred EventBus effect failed after commit");
                }
            }
        }
    }
    Ok(result)
}

/// Apply surface operations collecting EventBus effects for post-commit dispatch.
/// On conflict/failure, returns ApplyResult with status `failed` and populated conflicts —
/// callers MUST treat that as non-success and roll back any outer transaction.
pub fn apply_transaction_deferred(
    db: &mut Database,
    transaction_id: &str,
    deferred: &mut Vec<super::outbox::DeferredBusEffect>,
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
    let mut initial_revisions: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();

    for op in &txn.operations {
        if let Some(sid) = op.target.surface_id.as_ref() {
            if let Ok(s) = get_surface(db, sid) {
                initial_revisions
                    .entry(sid.clone())
                    .or_insert(s.current_revision);
                let current_state = super::surfaces::get_surface_state(db, sid).unwrap_or_else(|_| json!({}));
                previous.as_object_mut().unwrap().insert(
                    sid.clone(),
                    json!({
                        "revision": s.current_revision,
                        "definition": s.definition,
                        "state": current_state,
                    }),
                );
            }
        }
    }

    db.conn()
        .execute_batch("SAVEPOINT runtime_v2_apply")
        .map_err(DbError::Sqlite)?;

    let mut failed = false;
    for op in &txn.operations {
        match apply_one(db, op, &txn, &initial_revisions, deferred) {
            Ok(Some(s)) => surfaces.push(s),
            Ok(None) => {}
            Err(e) => {
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
        // Surface mutations roll back; mark failed only after savepoint release so
        // the status write is not undone by ROLLBACK TO. Outer kernel BEGIN still
        // rolls this back on Conflicted (caller must not treat ApplyResult alone as success).
        db.conn()
            .execute_batch(
                "ROLLBACK TO SAVEPOINT runtime_v2_apply; RELEASE SAVEPOINT runtime_v2_apply",
            )
            .map_err(DbError::Sqlite)?;
        surfaces.clear();
        deferred.clear();
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
        params![
            now,
            previous.to_string(),
            result.to_string(),
            transaction_id
        ],
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

pub(crate) fn resolve_effective_surface_id(op: &AppOperation) -> Option<String> {
    op.target
        .surface_id
        .clone()
        .or_else(|| {
            op.target
                .tool_id
                .as_deref()
                .map(super::surfaces::surface_id_for_tool)
        })
        .or_else(|| {
            op.payload
                .get("targetToolId")
                .and_then(|v| v.as_str())
                .map(super::surfaces::surface_id_for_tool)
        })
        .or_else(|| {
            op.payload
                .get("surfaceId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

fn apply_one(
    db: &mut Database,
    op: &AppOperation,
    txn: &AppTransactionRecord,
    initial_revisions: &std::collections::HashMap<String, i64>,
    deferred: &mut Vec<super::outbox::DeferredBusEffect>,
) -> Result<Option<SurfaceRecord>, String> {
    // Audience routing enforcement
    if let Some(aud) = &op.audience {
        match aud {
            Audience::FutureParticipants => {
                // Operations intended for future participants or template instantiation
                // are preserved in transaction history without mutating live session surfaces.
                return Ok(None);
            }
            Audience::CurrentSurface => {
                if op.target.surface_id.is_none() && op.target.tool_id.is_none() {
                    return Err(format!(
                        "operation '{}' specifies CurrentSurface audience but lacks surfaceId or toolId target",
                        op.id
                    ));
                }
            }
            Audience::CurrentChat => {
                if let (Some(txn_chat), Some(op_chat)) = (
                    txn.conversation_id.as_deref(),
                    op.target.conversation_id.as_deref(),
                ) {
                    if !txn_chat.is_empty() && !op_chat.is_empty() && txn_chat != op_chat {
                        return Err(format!(
                            "cross-chat violation: operation '{}' targeting conversation '{}' does not match transaction conversation '{}'",
                            op.id, op_chat, txn_chat
                        ));
                    }
                }
            }
            Audience::CurrentProject => {
                if let (Some(txn_proj), Some(op_proj)) =
                    (txn.project_id.as_deref(), op.target.project_id.as_deref())
                {
                    if !txn_proj.is_empty() && !op_proj.is_empty() && txn_proj != op_proj {
                        return Err(format!(
                            "cross-project violation: operation '{}' targeting project '{}' does not match transaction project '{}'",
                            op.id, op_proj, txn_proj
                        ));
                    }
                }
            }
            Audience::CurrentUser => {}
        }
    }

    // Cross-project and cross-chat isolation enforcement on surface/tool targets
    let effective_surface_id = resolve_effective_surface_id(op);
    if let Some(sid) = effective_surface_id.as_deref() {
        if let Ok(target_surface) = get_surface(db, sid) {
            if let (Some(txn_proj), Some(surf_proj)) = (
                txn.project_id.as_deref(),
                target_surface.project_id.as_deref(),
            ) {
                if !txn_proj.is_empty() && !surf_proj.is_empty() && txn_proj != surf_proj {
                    return Err(format!(
                        "cross-project violation: transaction in project '{}' cannot mutate surface '{}' belonging to project '{}'",
                        txn_proj, sid, surf_proj
                    ));
                }
            }
            if let (Some(op_proj), Some(surf_proj)) = (
                op.target.project_id.as_deref(),
                target_surface.project_id.as_deref(),
            ) {
                if !op_proj.is_empty() && !surf_proj.is_empty() && op_proj != surf_proj {
                    return Err(format!(
                        "cross-project violation: operation '{}' targeting project '{}' cannot mutate surface '{}' belonging to project '{}'",
                        op.id, op_proj, sid, surf_proj
                    ));
                }
            }
            if let (Some(txn_chat), Some(surf_chat)) = (
                txn.conversation_id.as_deref(),
                target_surface.conversation_id.as_deref(),
            ) {
                if !txn_chat.is_empty() && !surf_chat.is_empty() && txn_chat != surf_chat {
                    return Err(format!(
                        "cross-chat violation: operation '{}' in conversation '{}' cannot mutate surface '{}' belonging to conversation '{}'",
                        op.id, txn_chat, sid, surf_chat
                    ));
                }
            }
            if let (Some(op_chat), Some(surf_chat)) = (
                op.target.conversation_id.as_deref(),
                target_surface.conversation_id.as_deref(),
            ) {
                if !op_chat.is_empty() && !surf_chat.is_empty() && op_chat != surf_chat {
                    return Err(format!(
                        "cross-chat violation: operation '{}' targeting conversation '{}' cannot mutate surface '{}' belonging to conversation '{}'",
                        op.id, op_chat, sid, surf_chat
                    ));
                }
            }
        }
    }

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
                op.target
                    .project_id
                    .as_deref()
                    .or(txn.project_id.as_deref()),
                name,
                &definition,
                &packs,
            )
            .map_err(|e| e.to_string())?;
            Ok(Some(s))
        }
        "chat.inline_surface_update"
        | "component.update_props"
        | "component.insert"
        | "component.remove"
        | "component.replace"
        | "component.move"
        | "component.update_children"
        | "component.update_visibility"
        | "component.update_actions" => {
            let effective_sid = resolve_effective_surface_id(op)
                .ok_or_else(|| "surfaceId or toolId required".to_string())?;
            let sid = &effective_sid;
            let surface = get_surface(db, sid).map_err(|e| e.to_string())?;
            let effective_base =
                effective_base_revision(op, surface.current_revision, initial_revisions, sid);

            // Canonical SoftwareDocument path: if the definition contains sections,
            // mutate the SoftwareDocument directly to preserve regions, layout, contracts, and packs.
            let is_structured = surface.definition.get("sections").is_some();
            let def_value = if is_structured {
                let original_doc = super::software_document::SoftwareDocument::from_value(&surface.definition)
                    .map_err(|e| format!("failed to load SoftwareDocument for surface '{sid}': {e}"))?;
                let mut doc = original_doc.clone();
                doc.apply_operation(op)?;
                let _notes = doc.validate_and_repair();
                super::software_document::admit_software_document(
                    Some(&original_doc),
                    &doc,
                    &surface.capability_packs,
                )?;
                serde_json::to_value(&doc).map_err(|e| e.to_string())?
            } else {
                let mut def_value = surface.definition.clone();
                // Definition is a flat ToolDefinition-shaped object
                let mut components: Vec<ToolComponent> = def_value
                    .get("components")
                    .cloned()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
                if op.op_type.starts_with("component.") {
                    let cid = op.target.component_id.as_deref();
                    let policy = resolve_policy_for_apply(db, sid, cid, &op.payload);
                    let old_comp = cid.and_then(|id| find_component(&components, id).cloned());
                    apply_component_op(
                        &mut components,
                        &op.op_type,
                        cid,
                        op.target.parent_id.as_deref(),
                        effective_base,
                        surface.current_revision,
                        &op.payload,
                    )
                    .map_err(|e| e.to_string())?;
                    if op.op_type == "component.replace" {
                        if let (Some(old), Some(id)) = (old_comp.as_ref(), cid) {
                            if let Some(new) = find_component_mut(&mut components, id) {
                                let preserved = apply_preservation_on_replace(old, new, policy);
                                if !preserved {
                                    let _ = invalidate_component_live_state(db, sid, id);
                                } else {
                                    let _ = upsert_preservation(
                                        db,
                                        sid,
                                        id,
                                        policy,
                                        prop_preservation_key(new),
                                        &new.component_type,
                                        None,
                                    );
                                }
                            }
                        }
                    } else if op.op_type == "component.remove" {
                        if let Some(id) = cid {
                            // Removed components must not keep live state/drafts.
                            let _ = invalidate_component_live_state(db, sid, id);
                        }
                    } else if let Some(id) = cid {
                        // Record policy for future replaces when payload declares one.
                        if op.payload.get("preservationPolicy").is_some()
                            || op.payload.get("policy").is_some()
                        {
                            let ctype = find_component(&components, id)
                                .map(|c| c.component_type.clone())
                                .unwrap_or_else(|| "unknown".into());
                            let _ = upsert_preservation(
                                db,
                                sid,
                                id,
                                policy,
                                find_component(&components, id).and_then(prop_preservation_key),
                                &ctype,
                                None,
                            );
                        }
                    }
                    if let Some(obj) = def_value.as_object_mut() {
                        obj.insert(
                            "components".into(),
                            serde_json::to_value(&components).unwrap(),
                        );
                    }
                } else if let Some(definition) = op.payload.get("definition") {
                    def_value = definition.clone();
                }
                def_value
            };

            let s = update_surface_definition(
                db,
                sid,
                &def_value,
                op.payload
                    .get("changeSummary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("patch"),
                effective_base,
            )
            .map_err(|e| e.to_string())?;
            Ok(Some(s))
        }
        "surface.add_section"
        | "surface.remove_section"
        | "surface.update_section"
        | "component.bind_state"
        | "component.bind_action"
        | "component.set_style_token" => {
            let effective_sid = resolve_effective_surface_id(op)
                .ok_or_else(|| "surfaceId or toolId required".to_string())?;
            let sid = &effective_sid;
            let surface = get_surface(db, sid).map_err(|e| e.to_string())?;
            let effective_base =
                effective_base_revision(op, surface.current_revision, initial_revisions, sid);

            // LOSSLESS LOAD: Use from_value() which detects persisted SoftwareDocument format
            // (has `sections` key) and deserializes directly, preserving state_contracts,
            // action_contracts, design_tokens, capability_packs, and section hierarchy.
            // Falls back to from_tool_definition() only for legacy ToolDefinition-format definitions.
            let mut doc = super::software_document::SoftwareDocument::from_value(
                &surface.definition,
            )
            .map_err(|e| {
                format!(
                    "failed to load SoftwareDocument for surface '{}': {e}",
                    sid
                )
            })?;

            match op.op_type.as_str() {
                "surface.add_section" => {
                    let section: super::software_document::DocumentSection =
                        serde_json::from_value(
                            op.payload
                                .get("section")
                                .cloned()
                                .unwrap_or_else(|| op.payload.clone()),
                        )
                        .map_err(|e| format!("invalid section payload: {e}"))?;
                    // Validate section components against surface capability packs
                    if !section.components.is_empty() {
                        let allowed = if surface.capability_packs.is_empty() {
                            super::packs::required_packs_for_definition(&surface.definition)?
                        } else {
                            super::packs::normalize_capability_packs(&surface.capability_packs)?
                        };
                        super::packs::validate_tool_components_for_packs(
                            &section.components,
                            &allowed,
                        )?;
                    }
                    let index = op
                        .payload
                        .get("index")
                        .and_then(|v| v.as_u64())
                        .map(|i| i as usize);
                    doc.add_section(section, index)?;
                }
                "surface.remove_section" => {
                    let sec_id = op
                        .payload
                        .get("sectionId")
                        .and_then(|v| v.as_str())
                        .or_else(|| op.target.component_id.as_deref())
                        .ok_or_else(|| "sectionId required".to_string())?;
                    doc.remove_section(sec_id)?;
                }
                "surface.update_section" => {
                    let sec_id = op
                        .payload
                        .get("sectionId")
                        .and_then(|v| v.as_str())
                        .or_else(|| op.target.component_id.as_deref())
                        .ok_or_else(|| "sectionId required".to_string())?;
                    let comps: Option<Vec<ToolComponent>> = op
                        .payload
                        .get("components")
                        .cloned()
                        .and_then(|v| serde_json::from_value(v).ok());
                    // Pre-mutation validation: validate components against capability packs before mutating
                    if let Some(ref new_comps) = comps {
                        let allowed = if surface.capability_packs.is_empty() {
                            super::packs::required_packs_for_definition(&surface.definition)?
                        } else {
                            super::packs::normalize_capability_packs(&surface.capability_packs)?
                        };
                        super::packs::validate_tool_components_for_packs(new_comps, &allowed)?;
                    }
                    let title = op
                        .payload
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let layout = op
                        .payload
                        .get("layout")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    doc.update_section(sec_id, comps, title, layout)?;
                }
                "component.bind_state" => {
                    let cid = op
                        .target
                        .component_id
                        .as_deref()
                        .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                        .ok_or_else(|| "componentId required".to_string())?;
                    let key = op
                        .payload
                        .get("key")
                        .and_then(|v| v.as_str())
                        .or_else(|| op.payload.get("valueKey").and_then(|v| v.as_str()))
                        .ok_or_else(|| "key or valueKey required".to_string())?;
                    let initial_val = op.payload.get("initialValue").cloned();
                    doc.bind_state(cid, key, initial_val)?;
                }
                "component.bind_action" => {
                    let cid = op
                        .target
                        .component_id
                        .as_deref()
                        .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                        .ok_or_else(|| "componentId required".to_string())?;
                    let action: crate::ai::response_schema::ActionDefinition =
                        serde_json::from_value(
                            op.payload
                                .get("action")
                                .cloned()
                                .unwrap_or_else(|| op.payload.clone()),
                        )
                        .map_err(|e| format!("invalid action payload: {e}"))?;
                    doc.bind_action(cid, action)?;
                }
                "component.set_style_token" => {
                    let target_id = op
                        .target
                        .component_id
                        .as_deref()
                        .or_else(|| op.payload.get("targetId").and_then(|v| v.as_str()))
                        .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                        .ok_or_else(|| "targetId or componentId required".to_string())?;
                    let token = op
                        .payload
                        .get("token")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| "token required".to_string())?;
                    let val = op
                        .payload
                        .get("value")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::Null);
                    doc.set_style_token(target_id, token, val)?;
                }
                _ => {}
            }

            let original_doc = super::software_document::SoftwareDocument::from_value(&surface.definition).ok();
            let _notes = doc.validate_and_repair();
            super::software_document::admit_software_document(
                original_doc.as_ref(),
                &doc,
                &surface.capability_packs,
            )?;
            // LOSSLESS PERSIST: Serialize as SoftwareDocument JSON (not back to ToolDefinition).
            // This preserves state_contracts, action_contracts, design_tokens, capability_packs,
            // and region hierarchy across all semantic edit operations.
            let updated_def = serde_json::to_value(&doc).map_err(|e| e.to_string())?;

            let summary = op
                .payload
                .get("changeSummary")
                .and_then(|v| v.as_str())
                .unwrap_or(op.op_type.as_str());

            let s = update_surface_definition(db, sid, &updated_def, summary, effective_base)
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
            let existing_surface_id = super::surfaces::surface_id_for_tool(
                op.payload
                    .get("targetToolId")
                    .and_then(|v| v.as_str())
                    .or(op.target.tool_id.as_deref())
                    .unwrap_or(&tool.id),
            );
            if let Ok(existing) = get_surface(db, &existing_surface_id) {
                if let (Some(txn_proj), Some(surf_proj)) =
                    (txn.project_id.as_deref(), existing.project_id.as_deref())
                {
                    if !txn_proj.is_empty() && !surf_proj.is_empty() && txn_proj != surf_proj {
                        return Err(format!(
                            "cross-project violation: transaction in project '{}' cannot replace tool surface '{}' belonging to project '{}'",
                            txn_proj, existing.id, surf_proj
                        ));
                    }
                }
                let allowed = if existing.capability_packs.is_empty() {
                    super::packs::required_packs_for_definition(&existing.definition)?
                } else {
                    super::packs::normalize_capability_packs(&existing.capability_packs)?
                };
                validate_tool_components_for_packs(&tool.components, &allowed)?;
            }
            let workspace = "ws-personal-default";
            let action = op.payload.get("action").and_then(|v| v.as_str()).unwrap_or(
                if op.op_type == "surface.create" {
                    "create"
                } else {
                    "replace"
                },
            );
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
        "surface.delete" | "chat.inline_surface_remove" => {
            let sid = op
                .target
                .surface_id
                .as_deref()
                .ok_or_else(|| "surfaceId required".to_string())?;
            let delete_linked_tool = op
                .payload
                .get("deleteLinkedTool")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let deleted = delete_surface(db, sid, DeleteSurfaceOptions { delete_linked_tool })
                .map_err(|e| e.to_string())?;
            deferred.push(
                super::outbox::DeferredBusEffect::RemoveSubscriptionsForSurface {
                    surface_id: sid.into(),
                },
            );
            let ev = super::events::SurfaceEvent {
                id: format!("evt-{}", Uuid::new_v4()),
                event_type: "surface.deleted".into(),
                scope: "surface".into(),
                source: super::events::EventRef {
                    surface_id: Some(sid.into()),
                    tool_id: deleted.tool_id.clone(),
                    conversation_id: deleted
                        .conversation_id
                        .clone()
                        .or_else(|| op.target.conversation_id.clone()),
                    project_id: deleted
                        .project_id
                        .clone()
                        .or_else(|| op.target.project_id.clone()),
                    component_id: None,
                },
                target: Default::default(),
                payload: json!({ "surfaceId": sid }),
                idempotency_key: op.idempotency_key.clone(),
            };
            deferred.push(super::outbox::DeferredBusEffect::Dispatch(ev));
            Ok(Some(deleted))
        }
        "surface.archive" => {
            let sid = op
                .target
                .surface_id
                .as_deref()
                .ok_or_else(|| "surfaceId required".to_string())?;
            let s = archive_surface(db, sid).map_err(|e| e.to_string())?;
            Ok(Some(s))
        }
        "surface.restore" => {
            let effective_sid = resolve_effective_surface_id(op)
                .ok_or_else(|| "surfaceId or toolId required".to_string())?;
            let sid = &effective_sid;
            let s = restore_surface(db, sid).map_err(|e| e.to_string())?;
            Ok(Some(s))
        }
        "state.set" | "state.patch" => {
            let effective_sid = resolve_effective_surface_id(op)
                .ok_or_else(|| "surfaceId or toolId required".to_string())?;
            let sid = &effective_sid;
            let surface = get_surface(db, sid).map_err(|e| e.to_string())?;

            // Load contracts if surface has SoftwareDocument definition
            let doc = super::software_document::SoftwareDocument::from_value(&surface.definition).ok();

            let incoming = op
                .payload
                .get("state")
                .cloned()
                .unwrap_or_else(|| op.payload.clone());

            // Extract touched keys
            let touched_keys: Vec<String> = if let Some(key) = op.payload.get("key").and_then(|v| v.as_str()) {
                vec![key.to_string()]
            } else if let Some(obj) = incoming.as_object() {
                obj.keys().cloned().collect()
            } else {
                vec![]
            };

            // Enforce state contracts on every touched key
            if let Some(ref d) = doc {
                for k in &touched_keys {
                    let sc = d.state_contracts.iter().find(|s| &s.key == k).ok_or_else(|| {
                        format!("state key '{k}' has no declared state contract on surface '{sid}'")
                    })?;
                    if sc.write_policy == "readonly" {
                        return Err(format!("cannot write to readonly state key '{k}' on surface '{sid}'"));
                    }
                    if sc.read_policy == "restricted" || sc.sensitivity.as_deref() == Some("sensitive") {
                        return Err(format!("cannot write to restricted/sensitive state key '{k}' on surface '{sid}'"));
                    }
                }
            }

            // Load current state fail-closed (or default to empty object if no state yet)
            let mut current = super::surfaces::get_surface_state(db, sid).map_err(|e| e.to_string())?;

            // Apply updates while preserving unrelated state
            if let Some(key) = op.payload.get("key").and_then(|v| v.as_str()) {
                let val = op.payload.get("value").cloned().unwrap_or(Value::Null);
                if let Some(map) = current.as_object_mut() {
                    map.insert(key.to_string(), val);
                }
            } else if op.op_type == "state.patch" {
                super::surfaces::merge_json_objects(&mut current, &incoming);
            } else if let Some(obj) = incoming.as_object() {
                // In state.set with an object, update the specified keys only — never wipe undeclared/unrelated state!
                if let Some(cur_map) = current.as_object_mut() {
                    for (k, v) in obj {
                        cur_map.insert(k.clone(), v.clone());
                    }
                } else {
                    current = incoming;
                }
            } else {
                current = incoming;
            }

            super::surfaces::save_surface_state(db, sid, &current).map_err(|e| e.to_string())?;
            // Personal tools read `tool_state` in the canvas — mirror only on durable apply.
            if let Some(tool_id) = surface.tool_id.as_deref().filter(|t| !t.is_empty()) {
                crate::db::save_tool_state(db, tool_id, &current).map_err(|e| e.to_string())?;
            }
            Ok(Some(get_surface(db, sid).map_err(|e| e.to_string())?))
        }
        "layout.update" => {
            let sid = op
                .target
                .surface_id
                .as_deref()
                .or_else(|| op.payload.get("surfaceId").and_then(|v| v.as_str()))
                .ok_or_else(|| "surfaceId required for layout.update".to_string())?;
            let surface = get_surface(db, sid).map_err(|e| e.to_string())?;
            let mut def_value = surface.definition.clone();
            let new_layout = op
                .payload
                .get("layout")
                .cloned()
                .unwrap_or_else(|| op.payload.clone());
            let norm_layout = crate::ai::normalize_layout(&new_layout);
            crate::ai::validate_layout(&norm_layout)?;
            if let Some(obj) = def_value.as_object_mut() {
                obj.insert("layout".into(), norm_layout.clone());
            }
            let effective_base =
                effective_base_revision(op, surface.current_revision, initial_revisions, sid);
            let s = update_surface_definition(
                db,
                sid,
                &def_value,
                op.payload
                    .get("changeSummary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("layout.update"),
                effective_base,
            )
            .map_err(|e| e.to_string())?;
            let layout_str = crate::ai::layout_type_string(&norm_layout);
            if let Some(tool_id) = s.tool_id.as_deref().filter(|t| !t.is_empty()) {
                let _ = db.conn().execute(
                    "UPDATE tools SET layout = ?1, updated_at = datetime('now') WHERE id = ?2",
                    rusqlite::params![layout_str, tool_id],
                );
            }
            Ok(Some(s))
        }
        "wallpaper.apply" => {
            let wallpaper_json = op
                .payload
                .get("wallpaperJson")
                .or_else(|| op.payload.get("wallpaper_json"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let default_wallpaper = r#"{"kind":"none"}"#;
            if wallpaper_json.trim().is_empty() {
                crate::db::set_setting(db, "wallpaperJson", "").map_err(|e| e.to_string())?;
                crate::db::set_setting(db, "wallpaper", default_wallpaper)
                    .map_err(|e| e.to_string())?;
            } else {
                let parsed = crate::wallpapers::validate_wallpaper_config(wallpaper_json)
                    .map_err(|e| format!("invalid wallpaper: {e}"))?;
                let canonical_json = serde_json::to_string(&parsed).map_err(|e| e.to_string())?;
                crate::db::set_setting(db, "wallpaperJson", &canonical_json)
                    .map_err(|e| e.to_string())?;
                crate::db::set_setting(db, "wallpaper", default_wallpaper)
                    .map_err(|e| e.to_string())?;
            }
            Ok(None)
        }
        "data.model_upsert" | "data.record_create" | "data.record_update"
        | "data.record_delete" | "data.migrate" => {
            crate::application_kernel::data::apply_kernel_operations(db, std::slice::from_ref(op))
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        "chat.status" | "chat.notification" => Ok(None),
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
            deferred.push(super::outbox::DeferredBusEffect::AddSubscription(sub));
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
                deferred
                    .push(super::outbox::DeferredBusEffect::RemoveSubscription { id: sid.into() });
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
                deferred.push(super::outbox::DeferredBusEffect::SetSubscriptionEnabled {
                    id: sid.into(),
                    enabled,
                });
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
            deferred.push(super::outbox::DeferredBusEffect::Dispatch(ev.clone()));
            db.conn()
                .execute(
                    "INSERT INTO surface_events (
                        id, source_json, target_json, scope, event_type, payload_json,
                        idempotency_key, status, created_at, processed_at
                     ) VALUES (?1,?2,'{}',?3,?4,?5,?6,'pending',datetime('now'),NULL)",
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
        return Err(DbError::Invalid(
            "only applied transactions can be undone".into(),
        ));
    }
    let prev_json: Option<String> = db
        .conn()
        .query_row(
            "SELECT previous_snapshot_json FROM app_transactions WHERE id = ?1",
            [transaction_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?.flatten();
    let prev = prev_json.ok_or_else(|| {
        DbError::Invalid(format!("transaction '{transaction_id}' has no previous snapshot to undo to"))
    })?;

    let map = match serde_json::from_str::<Value>(&prev) {
        Ok(Value::Object(m)) => m,
        Ok(_) => return Err(DbError::Invalid("invalid snapshot JSON structure".into())),
        Err(e) => return Err(DbError::Invalid(format!("corrupt snapshot JSON: {e}"))),
    };

    db.conn().execute_batch("SAVEPOINT undo_sp").map_err(DbError::Sqlite)?;

    let res = (|| -> DbResult<()> {
        for (sid, snap) in &map {
            if let Some(def) = snap.get("definition") {
                update_surface_definition(db, sid, def, "undo transaction", None)?;
            }
            if let Some(st) = snap.get("state") {
                super::surfaces::save_surface_state(db, sid, st)?;
            }
        }
        let now = now_rfc3339();
        db.conn().execute(
            "UPDATE app_transactions SET status = 'reverted', reverted_at = ?1 WHERE id = ?2",
            params![now, transaction_id],
        )?;
        Ok(())
    })();

    match res {
        Ok(()) => {
            let _ = db.conn().execute_batch("RELEASE SAVEPOINT undo_sp");
            get_transaction(db, transaction_id)
        }
        Err(e) => {
            let _ = db.conn().execute_batch("ROLLBACK TO SAVEPOINT undo_sp");
            let _ = db.conn().execute_batch("RELEASE SAVEPOINT undo_sp");
            Err(e)
        }
    }
}

/// List recent transactions for a conversation (replay / undo UI).
pub fn list_transactions(
    db: &Database,
    conversation_id: &str,
    limit: usize,
) -> DbResult<Vec<AppTransactionRecord>> {
    let capped = limit.min(super::limits::MAX_REPLAY_OPS_LOADED);
    let mut stmt = db.conn().prepare(
        "SELECT id FROM app_transactions WHERE conversation_id = ?1
         ORDER BY created_at DESC LIMIT ?2",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![conversation_id, capped as i64], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter().map(|id| get_transaction(db, &id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{ToolAction, ToolChangePayload, ToolComponent, ToolDefinition};
    use crate::db::{
        apply_tool_change, create_conversation, get_tool, Database, DEFAULT_WORKSPACE_ID,
    };
    use crate::runtime_v2::operations::{tool_change_to_operations, OperationTarget};
    use crate::runtime_v2::surfaces::upsert_surface_from_tool;
    use tempfile::tempdir;

    fn test_db() -> Database {
        let dir = tempdir().unwrap();
        Database::open_path(&dir.path().join("t.db")).unwrap()
    }

    fn op(op_type: &str, surface_id: Option<&str>, payload: Value) -> AppOperation {
        AppOperation {
            id: format!("op-{}", Uuid::new_v4()),
            op_type: op_type.into(),
            target: OperationTarget {
                surface_id: surface_id.map(|s| s.into()),
                ..Default::default()
            },
            base_revision: None,
            transaction_group: None,
            idempotency_key: None,
            depends_on: None,
            payload,
            requires_approval: None,
            destructive: None,
            audience: None,
        }
    }

    #[test]
    fn apply_delete_and_archive_ops() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Txn", None).unwrap();
        let def = json!({
            "id": "d",
            "name": "Inline",
            "layout": "stack",
            "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Inline", &def, &[]).unwrap();
        let surface2 =
            create_inline_surface(&mut db, &conv.id, None, None, "Keep", &def, &[]).unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "delete+archive",
            &[
                op("chat.inline_surface_remove", Some(&surface.id), json!({})),
                op("surface.archive", Some(&surface2.id), json!({})),
            ],
            false,
        )
        .unwrap();
        let result = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(result.transaction.status, "applied");
        assert!(get_surface(&db, &surface.id).is_err());

        let archived = get_surface(&db, &surface2.id).unwrap();
        assert!(archived.archived);

        let txn2 = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "restore",
            &[op("surface.restore", Some(&surface2.id), json!({}))],
            false,
        )
        .unwrap();
        apply_transaction(&mut db, &txn2.id).unwrap();
        let restored = get_surface(&db, &surface2.id).unwrap();
        assert!(!restored.archived);
    }

    #[test]
    fn legacy_adapter_replacement_cannot_expand_existing_surface_packs() {
        let mut db = test_db();
        let original = ToolDefinition {
            id: "core-only-tool".into(),
            name: "Core only".into(),
            description: String::new(),
            layout: json!({"type": "single-column"}),
            components: vec![ToolComponent {
                id: "title".into(),
                component_type: "heading".into(),
                value_key: None,
                props: Some(json!({"text": "Original"})),
                children: None,
                ..Default::default()
            }],
        };
        let saved = apply_tool_change(
            &mut db,
            DEFAULT_WORKSPACE_ID,
            &original,
            "create",
            None,
            "seed",
        )
        .unwrap();
        let surface = upsert_surface_from_tool(
            &mut db,
            &saved.definition,
            DEFAULT_WORKSPACE_ID,
            saved.current_version,
        )
        .unwrap();
        let before_tool = get_tool(&db, &original.id).unwrap();
        let before_surface = get_surface(&db, &surface.id).unwrap();

        let forbidden = ToolChangePayload {
            action: ToolAction::Replace,
            target_tool_id: Some(original.id.clone()),
            tool: Some(ToolDefinition {
                components: vec![ToolComponent {
                    id: "scene".into(),
                    component_type: "svgScene".into(),
                    value_key: None,
                    props: Some(json!({})),
                    children: None,
                    ..Default::default()
                }],
                ..original.clone()
            }),
            change_summary: "attempt to add SVG".into(),
        };
        let ops = tool_change_to_operations(&forbidden);
        let txn = create_transaction(&mut db, None, None, None, "legacy replacement", &ops, false)
            .unwrap();
        let result = apply_transaction(&mut db, &txn.id).unwrap();

        assert_eq!(result.transaction.status, "failed");
        assert!(result.conflicts.iter().any(|conflict| {
            conflict.contains("coreside.svg") && conflict.contains("has not been granted")
        }));
        let after_tool = get_tool(&db, &original.id).unwrap();
        let after_surface = get_surface(&db, &surface.id).unwrap();
        assert_eq!(after_tool.definition, before_tool.definition);
        assert_eq!(after_tool.current_version, before_tool.current_version);
        assert_eq!(after_surface.definition, before_surface.definition);
        assert_eq!(
            after_surface.current_revision,
            before_surface.current_revision
        );
        assert_eq!(
            after_surface.capability_packs,
            before_surface.capability_packs
        );
    }

    #[test]
    fn apply_sequential_operations_same_surface_shared_base_revision() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "SeqTxn", None).unwrap();
        let def = json!({
            "id": "seq-surface",
            "name": "Sequential Surface",
            "layout": "stack",
            "components": [
                {"id": "c0", "type": "text", "props": {"text": "Initial"}}
            ]
        });
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Sequential Surface",
            &def,
            &[],
        )
        .unwrap();
        let initial_rev = surface.current_revision;

        let mut op1 = op(
            "component.insert",
            Some(&surface.id),
            json!({
                "component": {"id": "c1", "type": "text", "props": {"text": "Child 1"}}
            }),
        );
        op1.base_revision = Some(initial_rev);

        let mut op2 = op(
            "component.insert",
            Some(&surface.id),
            json!({
                "component": {"id": "c2", "type": "text", "props": {"text": "Child 2"}}
            }),
        );
        // Sibling op in the same model turn shares initial_rev
        op2.base_revision = Some(initial_rev);

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "sequential-inserts",
            &[op1, op2],
            false,
        )
        .unwrap();

        let result = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(result.transaction.status, "applied");
        assert!(result.conflicts.is_empty());

        let updated_surface = get_surface(&db, &surface.id).unwrap();
        assert_eq!(updated_surface.current_revision, initial_rev + 2);
        let comps = updated_surface
            .definition
            .get("components")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(comps.len(), 3);
        assert_eq!(comps[1]["id"], "c1");
        assert_eq!(comps[2]["id"], "c2");

        // Adversarial test: operation with stale base revision before transaction start must fail
        let mut stale_op = op(
            "component.insert",
            Some(&surface.id),
            json!({
                "component": {"id": "c3", "type": "text", "props": {"text": "Child 3"}}
            }),
        );
        stale_op.base_revision = Some(initial_rev - 1);
        let stale_txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "stale-insert",
            &[stale_op],
            false,
        )
        .unwrap();
        let stale_result = apply_transaction(&mut db, &stale_txn.id).unwrap();
        assert_eq!(stale_result.transaction.status, "failed");
        assert!(!stale_result.conflicts.is_empty());
    }

    #[test]
    fn test_audience_routing_and_future_participants() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Test", None).unwrap();
        let def = json!({
            "id": "aud-tool",
            "name": "Audience Tool",
            "layout": "stack",
            "components": [{"id": "c0", "type": "text", "props": {"text": "init"}}]
        });
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            Some("proj-1"),
            "Audience Test",
            &def,
            &[],
        )
        .unwrap();

        let initial_rev = surface.current_revision;

        // 1. FutureParticipants operation should be preserved in txn log but not mutate live surface
        let mut fp_op = op(
            "component.insert",
            Some(&surface.id),
            json!({
                "component": {"id": "c-fp", "type": "text", "props": {"text": "Future only"}}
            }),
        );
        fp_op.audience = Some(Audience::FutureParticipants);

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            Some("proj-1"),
            None,
            "future-participants-txn",
            &[fp_op],
            false,
        )
        .unwrap();

        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(res.transaction.status, "applied");
        // Live surface revision unchanged!
        let current_surf = get_surface(&db, &surface.id).unwrap();
        assert_eq!(current_surf.current_revision, initial_rev);
        let comps = current_surf
            .definition
            .get("components")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(comps.len(), 1); // "c-fp" not inserted into live surface

        // 2. CurrentSurface audience requires surface target
        let mut bad_cs_op = op(
            "component.insert",
            None,
            json!({
                "component": {"id": "c-bad", "type": "text", "props": {"text": "No surface"}}
            }),
        );
        bad_cs_op.target.surface_id = None;
        bad_cs_op.target.tool_id = None;
        bad_cs_op.audience = Some(Audience::CurrentSurface);

        let bad_txn_res = create_transaction(
            &mut db,
            Some(&conv.id),
            Some("proj-1"),
            None,
            "bad-cs-txn",
            &[bad_cs_op],
            false,
        );
        assert!(bad_txn_res.is_err());
    }

    #[test]
    fn test_cross_project_isolation_boundary() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Test", None).unwrap();
        let def = json!({
            "id": "proj-tool",
            "name": "Project Tool",
            "layout": "stack",
            "components": [{"id": "c0", "type": "text", "props": {"text": "init"}}]
        });
        let surface_a = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            Some("project-alpha"),
            "Project A Surface",
            &def,
            &[],
        )
        .unwrap();

        // Transaction in project-beta attempts to mutate surface_a in project-alpha
        let mut attack_op = op(
            "component.insert",
            Some(&surface_a.id),
            json!({
                "component": {"id": "c-infiltrate", "type": "text", "props": {"text": "Infiltrate"}}
            }),
        );
        attack_op.audience = Some(Audience::CurrentProject);

        let txn_b = create_transaction(
            &mut db,
            Some(&conv.id),
            Some("project-beta"),
            None,
            "cross-project-attack",
            &[attack_op],
            false,
        )
        .unwrap();

        let apply_res = apply_transaction(&mut db, &txn_b.id).unwrap();
        assert_eq!(apply_res.transaction.status, "failed");
        assert!(
            apply_res
                .conflicts
                .iter()
                .any(|c| c.contains("cross-project violation")),
            "Expected cross-project violation conflict, got: {:?}",
            apply_res.conflicts
        );

        // Verify surface_a is completely unmodified
        let surf_after = get_surface(&db, &surface_a.id).unwrap();
        assert_eq!(surf_after.current_revision, surface_a.current_revision);
    }

    #[test]
    fn test_cross_chat_isolation_boundary() {
        let mut db = test_db();
        let conv_a = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat A", None).unwrap();
        let conv_b = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat B", None).unwrap();

        let def = json!({
            "id": "chat-tool",
            "name": "Chat Tool",
            "layout": "stack",
            "components": [{"id": "c0", "type": "text", "props": {"text": "init"}}]
        });
        let surface_a = create_inline_surface(
            &mut db,
            &conv_a.id,
            None,
            Some("proj-shared"),
            "Surface in Chat A",
            &def,
            &[],
        )
        .unwrap();

        // Transaction in conv_b attempts to mutate surface_a in conv_a
        let mut attack_op = op(
            "component.insert",
            Some(&surface_a.id),
            json!({
                "component": {"id": "c-infiltrate", "type": "text", "props": {"text": "Infiltrate"}}
            }),
        );
        attack_op.audience = Some(Audience::CurrentChat);

        let txn_b = create_transaction(
            &mut db,
            Some(&conv_b.id),
            Some("proj-shared"),
            None,
            "cross-chat-attack",
            &[attack_op],
            false,
        )
        .unwrap();

        let apply_res = apply_transaction(&mut db, &txn_b.id).unwrap();
        assert_eq!(apply_res.transaction.status, "failed");
        assert!(
            apply_res
                .conflicts
                .iter()
                .any(|c| c.contains("cross-chat violation")),
            "Expected cross-chat violation conflict, got: {:?}",
            apply_res.conflicts
        );

        // Verify surface_a is completely unmodified
        let surf_after = get_surface(&db, &surface_a.id).unwrap();
        assert_eq!(surf_after.current_revision, surface_a.current_revision);
        let comps = surf_after
            .definition
            .get("components")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0]["id"], "c0");
    }

    /// P0 regression test: semantic edits must not destroy SoftwareDocument metadata.
    ///
    /// Previously, `surface.add_section` and `component.bind_state` parsed the persisted definition
    /// as a flat ToolDefinition, then reconstructed a fresh SoftwareDocument from components only —
    /// silently discarding state_contracts, action_contracts, design_tokens, and capability_packs.
    ///
    /// This test proves that those document-level fields survive a `component.bind_state` edit
    /// and a `surface.add_section` edit.  It would fail against the old implementation.
    #[test]
    fn software_document_metadata_survives_semantic_edit() {
        use crate::runtime_v2::software_document::{
            ActionContract, DocumentSection, SoftwareDocument, StateContract, StateScope,
        };

        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "DocIntegrity", None)
            .unwrap();

        // Build a rich SoftwareDocument with state_contracts, action_contracts,
        // design_tokens, capability_packs.
        let mut doc = SoftwareDocument::new("doc-integrity-test", "Document Integrity Test");
        doc.description = Some("P0 regression surface".into());
        doc.capability_packs = vec!["coreside.core".into(), "coreside.charts".into()];
        doc.design_tokens = Some(json!({ "density": "compact", "accent": "blue" }));
        doc.state_contracts = vec![
            StateContract {
                key: "user_notes".into(),
                type_name: "string".into(),
                initial_value: json!(""),
                scope: StateScope::Persistent,
                description: Some("User notes field".into()),
                preservation_policy: Some("preserve".into()),
                ..Default::default()
            },
            StateContract {
                key: "selected_tab".into(),
                type_name: "string".into(),
                initial_value: json!("tab-1"),
                scope: StateScope::Session,
                description: None,
                preservation_policy: None,
                ..Default::default()
            },
            StateContract {
                key: "saveResult".into(),
                type_name: "object".into(),
                initial_value: json!(null),
                scope: StateScope::Session,
                description: Some("Action result".into()),
                preservation_policy: None,
                ..Default::default()
            },
        ];
        doc.action_contracts = vec![ActionContract {
            action_id: "btn-save-notes-action".into(),
            action_name: "tool_state.set".into(),
            description: Some("Saves notes to persistent state".into()),
            result_key: Some("saveResult".into()),
            input_from_state: Some(std::collections::HashMap::from([(
                "value".into(),
                "user_notes".into(),
            )])),
        }];
        let section = DocumentSection {
            id: "main".into(),
            role: Some("content".into()),
            layout: Some("stack".into()),
            components: vec![ToolComponent {
                id: "notes-input".into(),
                component_type: "textArea".into(),
                value_key: Some("user_notes".into()),
                props: Some(json!({ "label": "User Notes", "placeholder": "Write notes here..." })),
                ..Default::default()
            }],
            ..Default::default()
        };
        doc.sections.push(section);

        // Persist as a SoftwareDocument (not ToolDefinition).
        let doc_value = serde_json::to_value(&doc).unwrap();
        let packs: Vec<String> = vec!["coreside.core".to_string(), "coreside.charts".to_string()];
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Document Integrity Test",
            &doc_value,
            &packs,
        )
        .unwrap();

        // --- Apply a component.bind_state operation ---
        // This previously destroyed the SoftwareDocument metadata.
        let bind_op = AppOperation {
            id: format!("op-{}", Uuid::new_v4()),
            op_type: "component.bind_state".into(),
            target: OperationTarget {
                surface_id: Some(surface.id.clone()),
                component_id: Some("notes-input".into()),
                ..Default::default()
            },
            base_revision: None,
            transaction_group: None,
            idempotency_key: None,
            depends_on: None,
            payload: json!({
                "key": "user_notes",
                "initialValue": ""
            }),
            requires_approval: None,
            destructive: None,
            audience: None,
        };
        let txn1 = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "bind state",
            &[bind_op],
            false,
        )
        .unwrap();
        let result1 = apply_transaction(&mut db, &txn1.id).unwrap();
        assert_eq!(
            result1.transaction.status, "applied",
            "bind_state transaction must apply, conflicts: {:?}",
            result1.conflicts
        );

        // Reload and verify ALL document-level metadata is intact.
        let updated = get_surface(&db, &surface.id).unwrap();
        let loaded_doc = SoftwareDocument::from_value(&updated.definition)
            .expect("reloaded definition must deserialize as SoftwareDocument");

        assert_eq!(
            loaded_doc.id, "doc-integrity-test",
            "document id must survive semantic edit"
        );
        assert_eq!(
            loaded_doc.description.as_deref(),
            Some("P0 regression surface"),
            "document description must survive semantic edit"
        );
        assert!(
            loaded_doc.capability_packs.contains(&"coreside.charts".to_string()),
            "capability_packs must survive: got {:?}",
            loaded_doc.capability_packs
        );
        assert!(
            loaded_doc.design_tokens.is_some(),
            "design_tokens must survive semantic edit"
        );
        assert_eq!(
            loaded_doc.design_tokens.as_ref().unwrap()["density"],
            json!("compact"),
            "design_tokens values must be intact"
        );
        let has_notes_contract = loaded_doc
            .state_contracts
            .iter()
            .any(|sc| sc.key == "user_notes");
        assert!(
            has_notes_contract,
            "state_contract 'user_notes' must survive: got {:?}",
            loaded_doc
                .state_contracts
                .iter()
                .map(|sc| &sc.key)
                .collect::<Vec<_>>()
        );
        let notes_scope = loaded_doc
            .state_contracts
            .iter()
            .find(|sc| sc.key == "user_notes")
            .map(|sc| sc.scope);
        assert_eq!(
            notes_scope,
            Some(StateScope::Persistent),
            "state scope must be preserved"
        );
        let has_selected_tab = loaded_doc
            .state_contracts
            .iter()
            .any(|sc| sc.key == "selected_tab");
        assert!(
            has_selected_tab,
            "state_contract 'selected_tab' must survive: got {:?}",
            loaded_doc
                .state_contracts
                .iter()
                .map(|sc| &sc.key)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            loaded_doc.action_contracts.len(),
            1,
            "action_contracts must survive: got {:?}",
            loaded_doc.action_contracts
        );
        assert_eq!(
            loaded_doc.action_contracts[0].action_id,
            "btn-save-notes-action"
        );

        // --- Apply a surface.add_section operation ---
        let add_section_op = AppOperation {
            id: format!("op-{}", Uuid::new_v4()),
            op_type: "surface.add_section".into(),
            target: OperationTarget {
                surface_id: Some(surface.id.clone()),
                ..Default::default()
            },
            base_revision: None,
            transaction_group: None,
            idempotency_key: None,
            depends_on: None,
            payload: json!({
                "section": {
                    "id": "sidebar",
                    "role": "sidebar",
                    "layout": "stack",
                    "components": []
                }
            }),
            requires_approval: None,
            destructive: None,
            audience: None,
        };
        let txn2 = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "add sidebar section",
            &[add_section_op],
            false,
        )
        .unwrap();
        let result2 = apply_transaction(&mut db, &txn2.id).unwrap();
        assert_eq!(result2.transaction.status, "applied");

        // Verify AGAIN that all metadata survives the second edit too.
        let updated2 = get_surface(&db, &surface.id).unwrap();
        let loaded_doc2 = SoftwareDocument::from_value(&updated2.definition)
            .expect("definition must still be a SoftwareDocument after add_section");
        assert!(
            loaded_doc2.sections.iter().any(|s| s.id == "sidebar"),
            "sidebar section must be present"
        );
        assert!(
            loaded_doc2.sections.iter().any(|s| s.id == "main"),
            "main section must still be present"
        );
        assert!(
            loaded_doc2
                .capability_packs
                .contains(&"coreside.charts".to_string()),
            "capability_packs must still be intact after add_section"
        );
        assert!(
            loaded_doc2
                .state_contracts
                .iter()
                .any(|sc| sc.key == "user_notes"),
            "state_contracts must still be intact after add_section"
        );
        assert_eq!(
            loaded_doc2.action_contracts.len(),
            1,
            "action_contracts must still be intact after add_section"
        );
    }
}

/// Compute the effective base revision for sequential operations within a transaction.
/// When multiple ops target the same surface with the same base_revision, the first op
/// advances the revision and subsequent ops see the updated revision.
fn effective_base_revision(
    op: &AppOperation,
    surface_revision: i64,
    initial_revisions: &std::collections::HashMap<String, i64>,
    surface_id: &str,
) -> Option<i64> {
    let init_rev = initial_revisions.get(surface_id).copied();
    match (op.base_revision, init_rev) {
        (Some(base), Some(init)) if base == init => Some(surface_revision),
        (Some(base), _) if base == surface_revision => Some(surface_revision),
        (Some(base), _) => Some(base),
        (None, _) => None,
    }
}

fn prop_preservation_key(comp: &ToolComponent) -> Option<&str> {
    comp.props
        .as_ref()
        .and_then(|p| {
            p.get("preservationKey")
                .or_else(|| p.get("preservation_key"))
        })
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}
