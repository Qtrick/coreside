//! Cross-surface application transactions with atomic apply and undo.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::operations::{validate_operations, AppOperation, Audience};
use super::packs::{validate_definition_components, validate_tool_components_for_packs};
use super::preservation::{
    apply_preservation_on_replace, clear_surface_state_keys, component_value_key,
    invalidate_component_live_state, resolve_policy_for_apply, should_preserve,
    upsert_preservation, PreservationPolicy,
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
    if let Some(cid) = conversation_id {
        if !crate::db::conversation_exists(db, cid) {
            return Err(DbError::NotFound(format!(
                "conversation {cid} not found or deleted"
            )));
        }
    }
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
                // SECURITY: fail closed — malformed operations_json must not silently
                // become an empty operation list, which would appear as a valid no-op.
                let operations: Vec<AppOperation> =
                    serde_json::from_str(&ops_json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("transaction operations_json malformed: {e}"),
                            )),
                        )
                    })?;
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
    let mut created_surfaces_list: Vec<String> = Vec::new();

    for op in &txn.operations {
        let sid_opt = resolve_effective_surface_id(op);
        if let Some(ref sid) = sid_opt {
            if let Ok(s) = get_surface(db, sid) {
                initial_revisions
                    .entry(sid.clone())
                    .or_insert(s.current_revision);
                let (current_state, state_rev) =
                    super::surfaces::get_surface_state_with_revision(db, sid)
                        .map_err(DbError::from)?;
                previous.as_object_mut().unwrap().insert(
                    sid.clone(),
                    json!({
                        "revision": s.current_revision,
                        "definition": s.definition,
                        "state": current_state,
                        "stateRevision": state_rev,
                        "expectedPostRevision": s.current_revision + 1,
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
            Ok(Some(s)) => {
                if op.op_type == "chat.inline_surface_create" || op.op_type == "surface.create" {
                    created_surfaces_list.push(s.id.clone());
                }
                surfaces.push(s);
            }
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

    if !created_surfaces_list.is_empty() {
        previous
            .as_object_mut()
            .unwrap()
            .insert("created_surfaces".into(), json!(created_surfaces_list));
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
        db.conn().execute(
            "UPDATE app_operations SET apply_status = 'failed' WHERE transaction_id = ?1",
            [transaction_id],
        )?;
        return Ok(ApplyResult {
            transaction: get_transaction(db, transaction_id)?,
            surfaces,
            conflicts,
        });
    }

    // Multi-op transactions advance revision more than once. Stamp the actual
    // post-apply revision so LIFO undo OCC matches the tip (not pre+1).
    if let Some(obj) = previous.as_object_mut() {
        for (sid, snap) in obj.iter_mut() {
            if sid == "created_surfaces" {
                continue;
            }
            if let Ok(s) = get_surface(db, sid) {
                if let Some(snap_obj) = snap.as_object_mut() {
                    snap_obj.insert("expectedPostRevision".into(), json!(s.current_revision));
                }
            }
        }
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
    // Authoritatively mark all operations in this committed transaction as applied
    db.conn().execute(
        "UPDATE app_operations SET apply_status = 'applied', validation_status = 'valid' WHERE transaction_id = ?1",
        [transaction_id],
    )?;

    // Append durable conversation event log INSIDE the same savepoint so a
    // failure rolls back the state mutation instead of silently losing catch-up.
    if let Some(ref conv_id) = txn.conversation_id {
        if let Err(e) = super::turn_journal::append_conversation_event_on_conn(
            db.conn(),
            conv_id,
            txn.turn_id.as_deref(),
            None,
            "surface.transaction_applied",
            &json!({
                "transactionId": transaction_id,
                "status": "applied",
                "operationsCount": txn.operations.len(),
                "surfaces": surfaces.iter().map(|s| &s.id).collect::<Vec<_>>(),
            }),
        ) {
            // Defensive: clear the open savepoint even when an outer BEGIN will
            // also ROLLBACK — callers that invoke this without an outer txn
            // must not leave runtime_v2_apply dangling.
            let _ = db.conn().execute_batch(
                "ROLLBACK TO SAVEPOINT runtime_v2_apply; RELEASE SAVEPOINT runtime_v2_apply",
            );
            return Err(e);
        }
    }

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

    // Choke point: suspended/disabled manifest-backed apps cannot mutate via any arm.
    if let Some(sid) = effective_surface_id.as_deref() {
        if let Ok(target_surface) = get_surface(db, sid) {
            crate::application_kernel::manifest::check_tool_application_accepts_mutations(
                db,
                target_surface.tool_id.as_deref(),
            )?;
        }
    }
    if let Some(app) = op
        .target
        .application_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if crate::application_kernel::manifest::authoritative_application_id(db, Some(app))
            .is_some()
            && !crate::application_kernel::manifest::application_accepts_mutations(db, app)
        {
            return Err(format!(
                "application '{app}' is disabled or suspended and cannot receive mutations"
            ));
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
            let packs: Vec<String> = match op.payload.get("capabilityPacks") {
                Some(v) => serde_json::from_value(v.clone()).map_err(|e| {
                    format!("invalid capabilityPacks in inline_surface_create: {e}")
                })?,
                None => Vec::new(),
            };
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
            // Surface row now exists — promote patches that were waiting on this target.
            let _ = super::patch_scheduler::promote_deferred_patches(db, Some(&s.id), None);
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

            let (mut doc, original_doc) = if surface.definition.get("sections").is_some() {
                let orig =
                    super::software_document::SoftwareDocument::from_value(&surface.definition)
                        .map_err(|e| {
                            format!("failed to load SoftwareDocument for surface '{sid}': {e}")
                        })?;
                (orig.clone(), Some(orig))
            } else {
                let tool_def: ToolDefinition = serde_json::from_value(surface.definition.clone())
                    .map_err(|e| {
                    format!("failed to parse flat ToolDefinition for surface '{sid}': {e}")
                })?;
                let orig =
                    super::software_document::SoftwareDocument::from_tool_definition(&tool_def);
                (orig.clone(), Some(orig))
            };

            let (current_state, _) = super::surfaces::get_surface_state_with_revision(db, sid)
                .map_err(|e| format!("failed to load surface state for surface '{sid}': {e}"))?;

            if op.op_type == "chat.inline_surface_update" && op.payload.get("definition").is_some()
            {
                let incoming_raw = op.payload.get("definition").unwrap();
                let candidate_doc = if incoming_raw.get("sections").is_some() {
                    super::software_document::SoftwareDocument::from_value(incoming_raw).map_err(
                        |e| {
                            format!(
                                "invalid candidate software document on inline_surface_update: {e}"
                            )
                        },
                    )?
                } else {
                    let tool_def: ToolDefinition = serde_json::from_value(incoming_raw.clone())
                        .map_err(|e| {
                            format!(
                                "invalid candidate tool definition on inline_surface_update: {e}"
                            )
                        })?;
                    super::software_document::SoftwareDocument::from_tool_definition(&tool_def)
                };
                super::software_document::admit_software_document_with_state(
                    original_doc.as_ref(),
                    &candidate_doc,
                    &surface.capability_packs,
                    Some(&current_state),
                    false,
                )?;
                // Preserve compatible component live props across full definition swaps.
                let mut candidate_doc = candidate_doc;
                if let Some(ref prior) = original_doc {
                    apply_definition_preservation(db, sid, prior, &mut candidate_doc, &op.payload)?;
                }
                doc = candidate_doc;
            } else if op.op_type == "component.replace" {
                apply_component_replace_with_preservation(db, sid, &mut doc, op)?;
                let _notes = doc.validate_and_repair();
                super::software_document::admit_software_document_with_state(
                    original_doc.as_ref(),
                    &doc,
                    &surface.capability_packs,
                    Some(&current_state),
                    false,
                )?;
            } else if op.op_type == "component.remove" {
                let cid = op
                    .target
                    .component_id
                    .as_deref()
                    .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
                    .ok_or_else(|| "componentId required".to_string())?;
                let old = doc.find_component(cid).cloned();
                // Component is gone — live state for its identity/valueKey must drop.
                let vk = old.as_ref().and_then(component_value_key);
                invalidate_component_live_state(db, sid, cid, vk).map_err(|e| e.to_string())?;
                doc.apply_operation(op)?;
                let _notes = doc.validate_and_repair();
                super::software_document::admit_software_document_with_state(
                    original_doc.as_ref(),
                    &doc,
                    &surface.capability_packs,
                    Some(&current_state),
                    false,
                )?;
            } else {
                // update_props / insert / move / visibility / actions / children
                if op.op_type == "component.update_props" {
                    maybe_preserve_or_reset_on_update_props(db, sid, &mut doc, op)?;
                } else {
                    doc.apply_operation(op)?;
                }
                doc.merge_inferred_state_contracts_from_bindings();
                let _notes = doc.validate_and_repair();
                super::software_document::admit_software_document_with_state(
                    original_doc.as_ref(),
                    &doc,
                    &surface.capability_packs,
                    Some(&current_state),
                    false,
                )?;
            }
            let def_value = serde_json::to_value(&doc).map_err(|e| e.to_string())?;

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
            let mut doc =
                super::software_document::SoftwareDocument::from_value(&surface.definition)
                    .map_err(|e| {
                        format!("failed to load SoftwareDocument for surface '{}': {e}", sid)
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
                    let comps: Option<Vec<ToolComponent>> = match op.payload.get("components") {
                        Some(v) => Some(
                            serde_json::from_value(v.clone())
                                .map_err(|e| format!("invalid components in section: {e}"))?,
                        ),
                        None => None,
                    };
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

            // SECURITY: Fail-closed — if the existing surface definition cannot be parsed
            // as a SoftwareDocument, this is a hard failure for canonical surfaces.
            // For legacy surfaces, the admission check will handle the None case explicitly.
            let original_doc =
                super::software_document::SoftwareDocument::from_value(&surface.definition)
                    .map_err(|e| format!("existing surface definition is malformed: {e}"))?;
            let _notes = doc.validate_and_repair();
            let (current_state, _) = super::surfaces::get_surface_state_with_revision(db, sid)
                .map_err(|e| format!("failed to load surface state for surface '{sid}': {e}"))?;
            super::software_document::admit_software_document_with_state(
                Some(&original_doc),
                &doc,
                &surface.capability_packs,
                Some(&current_state),
                false,
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
            let mut normalized_op = op.clone();
            super::operations::normalize_operation_payload(&mut normalized_op);
            let tool: ToolDefinition = serde_json::from_value(
                normalized_op
                    .payload
                    .get("tool")
                    .cloned()
                    .unwrap_or_else(|| normalized_op.payload.clone()),
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
                    application_id: None,
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
            let mut doc =
                super::software_document::SoftwareDocument::from_value(&surface.definition)
                    .map_err(|e| format!("invalid software document on surface '{sid}': {e}"))?;

            let incoming = op
                .payload
                .get("state")
                .or_else(|| op.payload.get("values"))
                .cloned()
                .unwrap_or_else(|| op.payload.clone());

            let key_mode = op.payload.get("key").and_then(|v| v.as_str());
            if key_mode.is_none() && !incoming.is_object() {
                return Err(format!(
                    "operation '{}' rejected: state mutations must be an object or {{key, value}}",
                    op.id
                ));
            }

            // Extract touched keys
            let touched_keys: Vec<String> = if let Some(key) = key_mode {
                vec![key.to_string()]
            } else if let Some(obj) = incoming.as_object() {
                obj.keys().cloned().collect()
            } else {
                vec![]
            };

            if let Some(rules) = super::interactive::interactive_definition_of(&surface.definition)?
            {
                let owned = rules.owned_keys();
                if let Some(k) = touched_keys.iter().find(|k| owned.contains(*k)) {
                    return Err(format!(
                        "operation '{}' rejected: state key '{k}' on surface '{sid}' is owned by the interactive rules engine",
                        op.id
                    ));
                }
            }

            // Enforce state contracts on every touched key. If no contracts are declared,
            // infer safe model contracts from component bindings so that newly generated surfaces
            // or legacy surfaces always enforce contract discipline without an empty-contract bypass.
            if doc.state_contracts.is_empty() {
                doc.infer_missing_contracts_if_empty();
            }

            let has_explicit_contracts = surface
                .definition
                .as_object()
                .map(|o| {
                    o.contains_key("stateContracts")
                        && !o
                            .get("stateContracts")
                            .and_then(|v| v.as_array())
                            .map(|a| a.is_empty())
                            .unwrap_or(false)
                })
                .unwrap_or(false);

            // Load current state before authorization so opaque (undeclared-but-present)
            // keys cannot be claimed via legacy synthesis.
            let (mut current, current_rev) =
                super::surfaces::get_surface_state_with_revision(db, sid)
                    .map_err(|e| e.to_string())?;

            if !current.is_object() {
                current = json!({});
            }

            for k in &touched_keys {
                let target_val = if key_mode.is_some() {
                    op.payload.get("value").unwrap_or(&Value::Null)
                } else if let Some(obj) = incoming.as_object() {
                    obj.get(k).unwrap_or(&Value::Null)
                } else {
                    &Value::Null
                };

                let synthesized;
                let sc = if let Some(found) = doc.state_contracts.iter().find(|s| &s.key == k) {
                    found
                } else if !has_explicit_contracts {
                    let already_present = current
                        .as_object()
                        .map(|o| o.contains_key(k))
                        .unwrap_or(false);
                    if already_present {
                        return Err(format!(
                            "cannot write to opaque state key '{k}' on surface '{sid}' without a declared contract"
                        ));
                    }
                    // Legacy compatibility path: infer safe default contract so legacy tools don't break,
                    // but newly generated Runtime V2 surfaces with explicit contracts are strictly enforced.
                    synthesized = super::software_document::StateContract::new_with_origin(
                        k.clone(),
                        Value::Null,
                        super::software_document::StateScope::Persistent,
                        "legacy",
                    );
                    &synthesized
                } else {
                    return Err(format!(
                        "state key '{k}' has no declared state contract on surface '{sid}'"
                    ));
                };

                if sc.write_policy == "readonly"
                    || sc.write_policy == "user"
                    || sc.write_policy == "system"
                {
                    return Err(format!(
                        "cannot write to {} state key '{k}' on surface '{sid}'",
                        sc.write_policy
                    ));
                }
                if sc.read_policy == "restricted" || sc.sensitivity.as_deref() == Some("sensitive")
                {
                    return Err(format!(
                        "cannot write to restricted/sensitive state key '{k}' on surface '{sid}'"
                    ));
                }
                super::software_document::validate_state_value_type(
                    k,
                    target_val,
                    &sc.type_name,
                    sc.is_effective_nullable(),
                )?;
            }

            // Apply updates while strictly preserving unrelated state (including opaque keys)
            if let Some(key) = key_mode {
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
                }
            }

            super::surfaces::save_surface_state_occ(db, sid, &current, current_rev)
                .map_err(|e| e.to_string())?;

            // COMPATIBILITY ADAPTER: Legacy (pre-V2) personal tools still read `tool_state`
            // from the canvas. Mirror canonical surface_state to tool_state ONLY for legacy
            // surfaces that are not yet migrated to the canonical surface_state authority.
            //
            // SECURITY INVARIANT: For Runtime V2 surfaces, surface_state is the sole authority.
            // tool_state is a read-only projection for legacy rendering only. The frontend must
            // never use tool_state as a write authority for Runtime V2 surfaces.
            //
            // This adapter exists solely for backward compatibility with pre-V2 tools.
            // It will be removed once all surfaces are migrated to canonical surface_state.
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
                .or_else(|| op.payload.get("ownerSurfaceId").and_then(|v| v.as_str()))
                .unwrap_or("");
            if owner.is_empty() {
                return Err("surfaceId required for subscription".to_string());
            }
            // Authorize subscription ownership against conversation/project
            let surf = super::surfaces::get_surface(db, owner).map_err(|e| e.to_string())?;
            if let Some(txn_conv) = txn.conversation_id.as_deref() {
                if surf.conversation_id.as_deref() != Some(txn_conv) {
                    return Err(format!(
                        "subscription surface '{owner}' does not belong to conversation '{txn_conv}'"
                    ));
                }
            }
            if let Some(txn_proj) = txn.project_id.as_deref() {
                if surf.project_id.as_deref() != Some(txn_proj) {
                    return Err(format!(
                        "subscription surface '{owner}' does not belong to project '{txn_proj}'"
                    ));
                }
            }
            let sub_id = op
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("sub-{}", Uuid::new_v4()));
            let event_types: Vec<String> = match op.payload.get("eventTypes") {
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid eventTypes in subscription.create: {e}"))?,
                None => Vec::new(),
            };
            let source_filter: super::events::EventRef = match op.payload.get("sourceFilter") {
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid sourceFilter in subscription.create: {e}"))?,
                None => Default::default(),
            };
            let target: super::events::EventRef = match op.payload.get("target") {
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid target in subscription.create: {e}"))?,
                None => Default::default(),
            };
            let handler: Option<super::events::EventHandler> = match op.payload.get("handler") {
                Some(v) if !v.is_null() => Some(
                    serde_json::from_value(v.clone())
                        .map_err(|e| format!("invalid handler in subscription.create: {e}"))?,
                ),
                _ => None,
            };
            let sub = super::events::Subscription {
                id: sub_id.clone(),
                owner_surface_id: owner.into(),
                event_types: event_types.clone(),
                source_filter: source_filter.clone(),
                target: target.clone(),
                handler: handler.clone(),
                enabled: true,
            };
            deferred.push(super::outbox::DeferredBusEffect::AddSubscription(sub));
            db.conn()
                .execute(
                    "INSERT OR REPLACE INTO surface_subscriptions (
                        id, owner_surface_id, source_filter_json, target_json,
                        event_types_json, handler_json, enabled, created_at, updated_at
                     ) VALUES (?1,?2,?3,?4,?5,?6,1,datetime('now'),datetime('now'))",
                    params![
                        sub_id,
                        owner,
                        serde_json::to_string(&source_filter).unwrap_or_else(|_| "{}".into()),
                        serde_json::to_string(&target).unwrap_or_else(|_| "{}".into()),
                        serde_json::to_string(&event_types).unwrap_or_else(|_| "[]".into()),
                        serde_json::to_string(&handler).unwrap_or_else(|_| "null".into())
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
            let owner_surf_id: String = db
                .conn()
                .query_row(
                    "SELECT owner_surface_id FROM surface_subscriptions WHERE id = ?1",
                    [sid],
                    |r| r.get(0),
                )
                .map_err(|_| format!("subscription '{sid}' not found"))?;
            let surf =
                super::surfaces::get_surface(db, &owner_surf_id).map_err(|e| e.to_string())?;
            if let Some(txn_conv) = txn.conversation_id.as_deref() {
                if surf.conversation_id.as_deref() != Some(txn_conv) {
                    return Err(format!(
                        "subscription '{sid}' does not belong to conversation '{txn_conv}'"
                    ));
                }
            }
            if let Some(txn_proj) = txn.project_id.as_deref() {
                if surf.project_id.as_deref() != Some(txn_proj) {
                    return Err(format!(
                        "subscription '{sid}' does not belong to project '{txn_proj}'"
                    ));
                }
            }
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
            let claimed_target: super::events::EventRef = match op.payload.get("target") {
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid target in event.dispatch: {e}"))?,
                None => super::events::EventRef {
                    surface_id: op.target.surface_id.clone(),
                    tool_id: op.target.tool_id.clone(),
                    conversation_id: op.target.conversation_id.clone(),
                    project_id: op.target.project_id.clone(),
                    component_id: op.target.component_id.clone(),
                    application_id: op.target.application_id.clone(),
                },
            };
            // Fail-closed: never trust model-asserted application/project/conversation.
            let target_ref = super::events::resolve_authoritative_event_target(
                db,
                &claimed_target,
                txn.conversation_id.as_deref(),
                txn.project_id.as_deref(),
            )?;
            // Authoritative event origin: derive from authoritative surface record and transaction context.
            // Do NOT accept untrusted spoofed source from op.payload.
            let src_surface_id = op.target.surface_id.as_deref().unwrap_or("");
            let (auth_tool_id, auth_conv_id, auth_proj_id) = if !src_surface_id.is_empty() {
                if let Ok(s) = super::surfaces::get_surface(db, src_surface_id) {
                    (s.tool_id, s.conversation_id, s.project_id)
                } else {
                    (None, txn.conversation_id.clone(), txn.project_id.clone())
                }
            } else {
                (None, txn.conversation_id.clone(), txn.project_id.clone())
            };
            // application_id is NOT tool_id. Only set when a manifest exists for that id.
            let auth_application_id =
                crate::application_kernel::manifest::authoritative_application_id(
                    db,
                    auth_tool_id.as_deref(),
                );
            let source_ref = super::events::EventRef {
                surface_id: if src_surface_id.is_empty() {
                    None
                } else {
                    Some(src_surface_id.to_string())
                },
                tool_id: auth_tool_id,
                conversation_id: auth_conv_id,
                project_id: auth_proj_id,
                component_id: op.target.component_id.clone(),
                application_id: auth_application_id,
            };
            let ev = super::events::SurfaceEvent {
                id: format!("evt-{}", Uuid::new_v4()),
                event_type: event_type.clone(),
                scope: op
                    .payload
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or("surface")
                    .into(),
                source: source_ref,
                target: target_ref,
                payload: op.payload.get("payload").cloned().unwrap_or(json!({})),
                idempotency_key: op.idempotency_key.clone(),
            };
            deferred.push(super::outbox::DeferredBusEffect::Dispatch(ev.clone()));
            db.conn()
                .execute(
                    "INSERT INTO surface_events (
                        id, source_json, target_json, scope, event_type, payload_json,
                        idempotency_key, status, created_at, processed_at
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,'pending',datetime('now'),NULL)",
                    params![
                        ev.id,
                        serde_json::to_string(&ev.source).unwrap_or_else(|_| "{}".into()),
                        serde_json::to_string(&ev.target).unwrap_or_else(|_| "{}".into()),
                        ev.scope,
                        ev.event_type,
                        ev.payload.to_string(),
                        ev.idempotency_key
                    ],
                )
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        "interactive.action" => {
            let sid = resolve_effective_surface_id(op)
                .ok_or_else(|| "interactive.action requires surfaceId".to_string())?;
            let action_id = op
                .payload
                .get("actionId")
                .and_then(Value::as_str)
                .ok_or_else(|| "interactive.action requires actionId".to_string())?;
            let expected = op
                .payload
                .get("stateRevision")
                .and_then(Value::as_i64)
                .ok_or_else(|| "interactive.action requires stateRevision".to_string())?;
            let params_v = op
                .payload
                .get("params")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let event_id: String = format!("op-{}-{}", txn.id, op.id)
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
                .take(128)
                .collect();
            super::interactive::dispatch(
                db,
                &sid,
                expected,
                &event_id,
                action_id,
                &params_v,
                super::interactive::Origin::Ai,
            )
            .map_err(|e| format!("interactive.action rejected: {e}"))?;
            Ok(Some(get_surface(db, &sid).map_err(|e| e.to_string())?))
        }
        // Compiler emits these; they are durable metadata / renderer-scoped nav.
        "setting.create" => {
            let app_id = op
                .target
                .application_id
                .as_deref()
                .or_else(|| op.payload.get("applicationId").and_then(Value::as_str))
                .ok_or_else(|| "setting.create requires applicationId".to_string())?;
            crate::security::assert_not_protected(app_id)?;
            let setting_id = op
                .payload
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| "setting.create requires id".to_string())?;
            if setting_id.trim().is_empty() {
                return Err("setting.create id must not be empty".into());
            }
            crate::security::assert_not_protected(setting_id)?;
            // Bind identity without treating tool_id as an existing surface id.
            // Explicit surface_id (or payload surfaceId) must host this application;
            // otherwise tool_id must equal applicationId.
            let explicit_surface_id = op.target.surface_id.clone().or_else(|| {
                op.payload
                    .get("surfaceId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
            if let Some(sid) = explicit_surface_id {
                let surface = get_surface(db, &sid).map_err(|e| e.to_string())?;
                let surface_app =
                    crate::application_kernel::manifest::authoritative_application_id(
                        db,
                        surface.tool_id.as_deref(),
                    )
                    .or_else(|| surface.tool_id.clone());
                if surface_app.as_deref() != Some(app_id) {
                    return Err(format!(
                        "setting.create applicationId '{app_id}' does not match target surface application"
                    ));
                }
            } else if let Some(tid) = op.target.tool_id.as_deref().filter(|t| !t.is_empty()) {
                if tid != app_id {
                    return Err(format!(
                        "setting.create applicationId '{app_id}' does not match target toolId '{tid}'"
                    ));
                }
            } else {
                return Err(
                    "setting.create requires a surface or tool target binding applicationId".into(),
                );
            }
            let mut rec = crate::application_kernel::manifest::get_manifest(db, app_id)
                .map_err(|e| format!("setting.create: unknown application: {e}"))?;
            if !rec.manifest.settings.iter().any(|s| s == setting_id) {
                rec.manifest.settings.push(setting_id.to_string());
                crate::application_kernel::manifest::upsert_manifest(db, rec.manifest)
                    .map_err(|e| e.to_string())?;
            }
            Ok(None)
        }
        "route.navigate" => {
            let route_id = op
                .payload
                .get("routeId")
                .and_then(Value::as_str)
                .ok_or_else(|| "route.navigate requires routeId".to_string())?;
            if route_id == "settings" || route_id == "recovery" {
                return Err("cannot navigate to protected routes via generated apps".into());
            }
            // Renderer-scoped: store intended route on surface state when a surface is targeted.
            if let Some(sid) = resolve_effective_surface_id(op) {
                let (mut state, rev) = super::surfaces::get_surface_state_with_revision(db, &sid)
                    .map_err(|e| e.to_string())?;
                if let Some(obj) = state.as_object_mut() {
                    obj.insert("activeRoute".into(), json!(route_id));
                } else {
                    state = json!({ "activeRoute": route_id });
                }
                super::surfaces::save_surface_state_occ(db, &sid, &state, rev)
                    .map_err(|e| e.to_string())?;
                return Ok(Some(get_surface(db, &sid).map_err(|e| e.to_string())?));
            }
            Ok(None)
        }
        other => Err(format!("unsupported operation in apply: {other}")),
    }
}

/// Keys owned by the interactive rules engine only change through its action
/// log, so a transaction undo restores everything else from the snapshot.
fn keep_engine_owned_state(db: &Database, sid: &str, snapshot: &Value) -> DbResult<Value> {
    let Ok(surface) = get_surface(db, sid) else {
        return Ok(snapshot.clone());
    };
    let Some(rules) = super::interactive::interactive_definition_of(&surface.definition)
        .map_err(DbError::Invalid)?
    else {
        return Ok(snapshot.clone());
    };
    let (current, _) = super::surfaces::get_surface_state_with_revision(db, sid)?;
    let mut restored = snapshot.as_object().cloned().unwrap_or_default();
    for key in rules.owned_keys() {
        match current.get(&key) {
            Some(v) => restored.insert(key, v.clone()),
            None => restored.remove(&key),
        };
    }
    Ok(Value::Object(restored))
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
        .optional()?
        .flatten();
    let prev = prev_json.ok_or_else(|| {
        DbError::Invalid(format!(
            "transaction '{transaction_id}' has no previous snapshot to undo to"
        ))
    })?;

    let map = match serde_json::from_str::<Value>(&prev) {
        Ok(Value::Object(m)) => m,
        Ok(_) => return Err(DbError::Invalid("invalid snapshot JSON structure".into())),
        Err(e) => return Err(DbError::Invalid(format!("corrupt snapshot JSON: {e}"))),
    };

    // Pre-flight OCC check: verify resources haven't been mutated since this transaction
    for (sid, snap) in &map {
        if sid == "created_surfaces" {
            continue;
        }
        if let Ok(current_s) = get_surface(db, sid) {
            if let Some(expected_rev) = snap.get("expectedPostRevision").and_then(|v| v.as_i64()) {
                if current_s.current_revision != expected_rev {
                    return Err(DbError::Conflict(format!(
                        "cannot undo transaction: surface '{sid}' was modified after transaction (expected revision {expected_rev}, current {})",
                        current_s.current_revision
                    )));
                }
            }
        }
    }

    db.conn()
        .execute_batch("SAVEPOINT undo_sp")
        .map_err(DbError::Sqlite)?;

    let res = (|| -> DbResult<()> {
        // 1. Delete surfaces created by this transaction
        if let Some(created) = map.get("created_surfaces").and_then(|v| v.as_array()) {
            for cid_val in created {
                if let Some(cid) = cid_val.as_str() {
                    delete_surface(
                        db,
                        cid,
                        DeleteSurfaceOptions {
                            delete_linked_tool: true,
                        },
                    )?;
                }
            }
        }

        // 2. Restore modified surfaces
        for (sid, snap) in &map {
            if sid == "created_surfaces" {
                continue;
            }
            let restored_state = match snap.get("state") {
                Some(st) => Some(keep_engine_owned_state(db, sid, st)?),
                None => None,
            };
            if let Some(def) = snap.get("definition") {
                let prev_rev = snap
                    .get("revision")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| {
                        DbError::Invalid(format!(
                            "snapshot for surface '{sid}' missing revision for undo"
                        ))
                    })?;
                if prev_rev < 0 {
                    return Err(DbError::Invalid(format!(
                        "invalid snapshot revision {prev_rev} for surface '{sid}'"
                    )));
                }
                // Direct restore (do not call update_surface_definition): that would
                // insert a new surface_versions row and bump revision, breaking LIFO
                // multi-turn undo OCC and UNIQUE(surface_id, revision) on re-apply.
                let def_json = serde_json::to_string(def)?;
                let now = now_rfc3339();
                db.conn().execute(
                    "UPDATE surfaces SET definition_json = ?1, current_revision = ?2, updated_at = ?3,
                     name = COALESCE(json_extract(?1, '$.name'), name) WHERE id = ?4",
                    params![def_json, prev_rev, now, sid],
                )?;
                db.conn().execute(
                    "DELETE FROM surface_versions WHERE surface_id = ?1 AND revision > ?2",
                    params![sid, prev_rev],
                )?;
                if let Ok(current) = get_surface(db, sid) {
                    if let Some(tool_id) = current.tool_id.as_ref() {
                        let mut tool: crate::ai::ToolDefinition =
                            if def.get("sections").is_some() {
                                if let Ok(doc) =
                                    crate::runtime_v2::SoftwareDocument::from_value(def)
                                {
                                    doc.to_tool_definition()
                                } else {
                                    serde_json::from_value(def.clone()).unwrap_or_else(|_| {
                                        crate::ai::ToolDefinition {
                                            id: tool_id.clone(),
                                            name: current.name.clone(),
                                            description: String::new(),
                                            layout: json!({ "type": "single-column" }),
                                            components: Vec::new(),
                                            ..Default::default()
                                        }
                                    })
                                }
                            } else {
                                serde_json::from_value(def.clone()).unwrap_or_else(|_| {
                                    crate::ai::ToolDefinition {
                                        id: tool_id.clone(),
                                        name: current.name.clone(),
                                        description: String::new(),
                                        layout: json!({ "type": "single-column" }),
                                        components: Vec::new(),
                                        ..Default::default()
                                    }
                                })
                            };
                        // tools.id is authoritative; keep current_version coupled to
                        // surface revision the same way update_surface_definition does.
                        tool.id = tool_id.clone();
                        let layout = crate::ai::layout_type_string(&tool.layout);
                        let tool_def_json = serde_json::to_string(&tool)?;
                        db.conn().execute(
                            "UPDATE tools SET name = ?1, description = ?2, layout = ?3, definition_json = ?4,
                             current_version = ?5, updated_at = ?6 WHERE id = ?7",
                            params![
                                tool.name,
                                tool.description,
                                layout,
                                tool_def_json,
                                prev_rev,
                                now,
                                tool_id
                            ],
                        )?;
                    }
                }
            }
            if let Some(st) = restored_state {
                super::surfaces::save_surface_state(db, sid, &st)?;
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
            db.conn()
                .execute_batch("RELEASE SAVEPOINT undo_sp")
                .map_err(DbError::Sqlite)?;
            get_transaction(db, transaction_id)
        }
        Err(e) => {
            let rb_res = db
                .conn()
                .execute_batch("ROLLBACK TO SAVEPOINT undo_sp; RELEASE SAVEPOINT undo_sp");
            if let Err(rb_err) = rb_res {
                tracing::error!(error = %rb_err, original_error = %e, "ROLLBACK failed during undo_transaction");
                return Err(DbError::Sqlite(rb_err));
            }
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

/// Authoritative `component.replace` with dimension-aware preservation + OCC invalidation.
fn apply_component_replace_with_preservation(
    db: &mut Database,
    surface_id: &str,
    doc: &mut super::software_document::SoftwareDocument,
    op: &AppOperation,
) -> Result<(), String> {
    let cid = op
        .target
        .component_id
        .as_deref()
        .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
        .ok_or_else(|| "componentId required".to_string())?;
    let old = doc
        .find_component(cid)
        .cloned()
        .ok_or_else(|| format!("component '{cid}' not found"))?;
    let mut new_comp: ToolComponent = serde_json::from_value(
        op.payload
            .get("component")
            .cloned()
            .unwrap_or_else(|| op.payload.clone()),
    )
    .map_err(|e| format!("invalid component payload: {e}"))?;
    // Stable identity: replace keeps the targeted component id.
    new_comp.id = cid.to_string();

    let policy = resolve_policy_for_apply(db, surface_id, Some(cid), &op.payload);
    let preserved = apply_preservation_on_replace(&old, &mut new_comp, policy);
    if preserved {
        let _ = super::preservation::overlay_live_state_on_component(
            db,
            surface_id,
            &old,
            &mut new_comp,
            policy,
        );
    }
    let old_vk = component_value_key(&old).map(|s| s.to_string());
    let new_vk = component_value_key(&new_comp).map(|s| s.to_string());
    let incoming_key = prop_preservation_key(&new_comp);
    let stored_key = prop_preservation_key(&old);
    let compatible = old.component_type == new_comp.component_type;

    doc.replace_component(cid, new_comp.clone())?;

    let _ = upsert_preservation(
        db,
        surface_id,
        cid,
        policy,
        incoming_key.or(stored_key),
        &new_comp.component_type,
        None,
    );

    let must_reset = !preserved
        || !should_preserve(policy, incoming_key, stored_key, compatible)
        || matches!(
            policy,
            PreservationPolicy::Replace | PreservationPolicy::ResetExplicitly
        );
    if must_reset {
        invalidate_component_live_state(db, surface_id, cid, old_vk.as_deref())
            .map_err(|e| e.to_string())?;
    } else if old_vk.as_deref() != new_vk.as_deref() {
        // Continuity kept, but valueKey renamed — drop the orphaned binding only.
        if let Some(ref ovk) = old_vk {
            clear_surface_state_keys(db, surface_id, &[ovk.as_str()]).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// `component.update_props`: honor reset/replace policies; otherwise leave state alone.
fn maybe_preserve_or_reset_on_update_props(
    db: &mut Database,
    surface_id: &str,
    doc: &mut super::software_document::SoftwareDocument,
    op: &AppOperation,
) -> Result<(), String> {
    let cid = op
        .target
        .component_id
        .as_deref()
        .or_else(|| op.payload.get("componentId").and_then(|v| v.as_str()))
        .ok_or_else(|| "componentId required".to_string())?;
    let old = doc.find_component(cid).cloned();
    let old_vk = old
        .as_ref()
        .and_then(component_value_key)
        .map(|s| s.to_string());
    let policy = resolve_policy_for_apply(db, surface_id, Some(cid), &op.payload);
    if matches!(
        policy,
        PreservationPolicy::Replace | PreservationPolicy::ResetExplicitly
    ) {
        invalidate_component_live_state(db, surface_id, cid, old_vk.as_deref())
            .map_err(|e| e.to_string())?;
    }
    doc.apply_operation(op)?;
    if let Some(comp) = doc.find_component(cid) {
        let new_vk = component_value_key(comp).map(|s| s.to_string());
        // valueKey rename under a preserving update_props must not leave an orphan binding.
        if !matches!(
            policy,
            PreservationPolicy::Replace | PreservationPolicy::ResetExplicitly
        ) && old_vk.as_deref() != new_vk.as_deref()
        {
            if let Some(ref ovk) = old_vk {
                clear_surface_state_keys(db, surface_id, &[ovk.as_str()])
                    .map_err(|e| e.to_string())?;
            }
        }
        let _ = upsert_preservation(
            db,
            surface_id,
            cid,
            policy,
            prop_preservation_key(comp),
            &comp.component_type,
            None,
        );
    }
    Ok(())
}

/// Full definition swap: preserve prop-level live dimensions for matching component ids.
fn apply_definition_preservation(
    db: &mut Database,
    surface_id: &str,
    prior: &super::software_document::SoftwareDocument,
    next: &mut super::software_document::SoftwareDocument,
    payload: &Value,
) -> Result<(), String> {
    let prior_flat = prior.flatten_components();
    let next_ids: std::collections::HashSet<String> = next
        .flatten_components()
        .into_iter()
        .map(|c| c.id)
        .collect();

    for old in &prior_flat {
        if !next_ids.contains(&old.id) {
            invalidate_component_live_state(db, surface_id, &old.id, component_value_key(old))
                .map_err(|e| e.to_string())?;
            continue;
        }
        let mut new_comp = next
            .find_component(&old.id)
            .cloned()
            .ok_or_else(|| format!("component '{}' missing after flatten", old.id))?;
        let policy = resolve_policy_for_apply(db, surface_id, Some(&old.id), payload);
        let preserved = apply_preservation_on_replace(old, &mut new_comp, policy);
        if preserved {
            let _ = super::preservation::overlay_live_state_on_component(
                db,
                surface_id,
                old,
                &mut new_comp,
                policy,
            );
        }
        let incoming_key = prop_preservation_key(&new_comp);
        let stored_key = prop_preservation_key(old);
        let compatible = old.component_type == new_comp.component_type;
        let _ = upsert_preservation(
            db,
            surface_id,
            &old.id,
            policy,
            incoming_key.or(stored_key),
            &new_comp.component_type,
            None,
        );
        let must_reset = !preserved
            || matches!(
                policy,
                PreservationPolicy::Replace | PreservationPolicy::ResetExplicitly
            )
            || !should_preserve(policy, incoming_key, stored_key, compatible);
        let old_vk = component_value_key(old).map(|s| s.to_string());
        let new_vk = component_value_key(&new_comp).map(|s| s.to_string());
        if must_reset {
            invalidate_component_live_state(db, surface_id, &old.id, old_vk.as_deref())
                .map_err(|e| e.to_string())?;
        } else {
            if old_vk.as_deref() != new_vk.as_deref() {
                if let Some(ref ovk) = old_vk {
                    clear_surface_state_keys(db, surface_id, &[ovk.as_str()])
                        .map_err(|e| e.to_string())?;
                }
            }
            if let Some(comp) = next.find_component_mut(&old.id) {
                *comp = new_comp;
            }
        }
    }
    Ok(())
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
            ..Default::default()
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
        let doc =
            crate::runtime_v2::SoftwareDocument::from_value(&updated_surface.definition).unwrap();
        assert_eq!(doc.sections[0].components.len(), 3);
        assert_eq!(doc.sections[0].components[1].id, "c1");
        assert_eq!(doc.sections[0].components[2].id, "c2");

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
        let comps = if let Ok(doc) =
            crate::runtime_v2::SoftwareDocument::from_value(&current_surf.definition)
        {
            doc.to_tool_definition().components
        } else {
            serde_json::from_value(
                current_surf
                    .definition
                    .get("components")
                    .cloned()
                    .unwrap_or(json!([])),
            )
            .unwrap_or_default()
        };
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
        let doc_after =
            crate::runtime_v2::SoftwareDocument::from_value(&surf_after.definition).unwrap();
        assert_eq!(doc_after.sections[0].components.len(), 1);
        assert_eq!(doc_after.sections[0].components[0].id, "c0");
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
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "DocIntegrity", None).unwrap();

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
                nullable: true,
                scope: StateScope::Session,
                description: Some("Action result".into()),
                preservation_policy: None,
                ..Default::default()
            },
        ];
        doc.action_contracts = vec![ActionContract {
            action_id: "btn-save-notes-action".into(),
            action_name: "tool_state.set".into(),
            component_id: Some("btn-save-notes".into()),
            descriptor_hash: None,
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
            loaded_doc
                .capability_packs
                .contains(&"coreside.charts".to_string()),
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

    #[test]
    fn create_transaction_rejects_deleted_or_nonexistent_conversation() {
        let mut db = test_db();
        let cid = "deleted-conv-id";
        let res = create_transaction(&mut db, Some(cid), None, None, "test_turn", &[], false);
        match res {
            Err(DbError::NotFound(msg)) => {
                assert!(msg.contains("conversation deleted-conv-id"));
            }
            other => panic!("expected DbError::NotFound, got {:?}", other),
        }
    }

    #[test]
    fn create_transaction_after_conversation_deletion_is_blocked() {
        let mut db = test_db();
        let conv = crate::db::create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Test Chat", None)
            .unwrap();
        crate::db::delete_conversation(&mut db, &conv.id).unwrap();

        let res = create_transaction(&mut db, Some(&conv.id), None, None, "late_turn", &[], false);
        assert!(matches!(res, Err(DbError::NotFound(_))));
    }

    #[test]
    fn state_set_enforces_type_contracts_and_rejects_undeclared_keys() {
        let mut db = test_db();
        let conv =
            crate::db::create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Contract Chat", None)
                .unwrap();

        let doc = json!({
            "schemaVersion": "coreside.document/v2",
            "documentId": "doc-game-1",
            "title": "Game App",
            "stateContracts": [
                {
                    "key": "score",
                    "type": "number",
                    "initial": 0,
                    "writePolicy": "model",
                    "readPolicy": "all"
                },
                {
                    "key": "player_name",
                    "type": "string",
                    "initial": "Player 1",
                    "writePolicy": "model",
                    "readPolicy": "all"
                }
            ],
            "sections": [
                {
                    "id": "main",
                    "components": []
                }
            ]
        });

        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Game Surface",
            &doc,
            &["coreside.core".to_string()],
        )
        .unwrap();

        // 1. Valid state.set matching contract types succeeds
        let valid_txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "valid_state_set",
            &[op(
                "state.set",
                Some(&surface.id),
                json!({ "values": { "score": 100, "player_name": "Alice" } }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &valid_txn.id).unwrap();
        assert_eq!(res.transaction.status, "applied");

        // 2. Invalid type (score as string instead of number) is rejected
        let invalid_type_txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "invalid_type_set",
            &[op(
                "state.set",
                Some(&surface.id),
                json!({ "values": { "score": "not_a_number" } }),
            )],
            false,
        )
        .unwrap();
        let err_res = apply_transaction(&mut db, &invalid_type_txn.id).unwrap();
        assert_eq!(err_res.transaction.status, "failed");
        assert!(
            err_res
                .conflicts
                .iter()
                .any(|c| c.contains("does not match contract type")),
            "expected contract type conflict, got {:?}",
            err_res.conflicts
        );

        // 3. Undeclared key is rejected because explicit contracts exist
        let undeclared_txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "undeclared_key_set",
            &[op(
                "state.set",
                Some(&surface.id),
                json!({ "values": { "undeclared_secret": 999 } }),
            )],
            false,
        )
        .unwrap();
        let undeclared_res = apply_transaction(&mut db, &undeclared_txn.id).unwrap();
        assert_eq!(undeclared_res.transaction.status, "failed");
        assert!(
            undeclared_res
                .conflicts
                .iter()
                .any(|c| c.contains("has no declared state contract")),
            "expected undeclared state key conflict, got {:?}",
            undeclared_res.conflicts
        );
    }

    #[test]
    fn state_set_rejects_user_write_policy_and_opaque_keys() {
        let mut db = test_db();
        let conv = crate::db::create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Auth Chat", None)
            .unwrap();

        // Explicit contracts: user-writable draft must not accept model state.set.
        let doc = json!({
            "schemaVersion": "coreside.document/v2",
            "documentId": "auth-doc",
            "title": "Auth Doc",
            "stateContracts": [
                {
                    "key": "draft",
                    "type": "string",
                    "initial": "",
                    "writePolicy": "user",
                    "origin": "user"
                },
                {
                    "key": "label",
                    "type": "string",
                    "initial": "",
                    "writePolicy": "model",
                    "origin": "model"
                }
            ],
            "sections": [{ "id": "main", "components": [] }]
        });
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Auth Surface",
            &doc,
            &["coreside.core".to_string()],
        )
        .unwrap();

        let user_txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "user_write_blocked",
            &[op(
                "state.set",
                Some(&surface.id),
                json!({ "values": { "draft": "hijack" } }),
            )],
            false,
        )
        .unwrap();
        let user_res = apply_transaction(&mut db, &user_txn.id).unwrap();
        assert_eq!(user_res.transaction.status, "failed");
        assert!(
            user_res
                .conflicts
                .iter()
                .any(|c| c.contains("user") || c.contains("cannot write")),
            "expected user writePolicy rejection, got {:?}",
            user_res.conflicts
        );

        // Opaque key: empty contracts + pre-existing state key must reject model write.
        let legacy = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Legacy Surface",
            &json!({
                "id": "legacy-doc",
                "name": "Legacy",
                "components": []
            }),
            &[],
        )
        .unwrap();
        let (mut st, rev) =
            crate::runtime_v2::surfaces::get_surface_state_with_revision(&db, &legacy.id).unwrap();
        if let Some(obj) = st.as_object_mut() {
            obj.insert("secretVault".into(), json!("opaque-value"));
        }
        crate::runtime_v2::surfaces::save_surface_state_occ(&mut db, &legacy.id, &st, rev).unwrap();

        let opaque_txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "opaque_blocked",
            &[op(
                "state.set",
                Some(&legacy.id),
                json!({ "values": { "secretVault": "stolen" } }),
            )],
            false,
        )
        .unwrap();
        let opaque_res = apply_transaction(&mut db, &opaque_txn.id).unwrap();
        assert_eq!(opaque_res.transaction.status, "failed");
        assert!(
            opaque_res.conflicts.iter().any(|c| c.contains("opaque")),
            "expected opaque key rejection, got {:?}",
            opaque_res.conflicts
        );
        let (after, _) =
            crate::runtime_v2::surfaces::get_surface_state_with_revision(&db, &legacy.id).unwrap();
        assert_eq!(after["secretVault"], json!("opaque-value"));
    }

    #[test]
    fn state_set_rejects_system_write_policy() {
        let mut db = test_db();
        let conv = crate::db::create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Auth Chat", None)
            .unwrap();

        // System-owned keys (e.g. runtime-managed metadata) must reject model writes
        // just like user-owned keys, even though the write-policy string differs.
        let doc = json!({
            "schemaVersion": "coreside.document/v2",
            "documentId": "auth-doc-system",
            "title": "Auth Doc System",
            "stateContracts": [
                {
                    "key": "systemLock",
                    "type": "boolean",
                    "initial": false,
                    "writePolicy": "system",
                    "origin": "system"
                }
            ],
            "sections": [{ "id": "main", "components": [] }]
        });
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Auth Surface System",
            &doc,
            &["coreside.core".to_string()],
        )
        .unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "system_write_blocked",
            &[op(
                "state.set",
                Some(&surface.id),
                json!({ "values": { "systemLock": true } }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(res.transaction.status, "failed");
        assert!(
            res.conflicts
                .iter()
                .any(|c| c.contains("system") || c.contains("cannot write")),
            "expected system writePolicy rejection, got {:?}",
            res.conflicts
        );
        let (state, _) =
            crate::runtime_v2::surfaces::get_surface_state_with_revision(&db, &surface.id).unwrap();
        // Rejection must leave state untouched (no seed of initial contracts into state bag).
        assert_ne!(state.get("systemLock"), Some(&json!(true)));
    }

    #[test]
    fn state_set_legacy_synthesis_allows_brand_new_opaque_key() {
        // Contrast case for `state_set_rejects_user_write_policy_and_opaque_keys`:
        // when contracts are empty, a model write to a key that is NOT already
        // present in current state must still succeed via legacy synthesis — the
        // opaque-key rejection only applies to *pre-existing* undeclared keys.
        let mut db = test_db();
        let conv =
            crate::db::create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Legacy Chat", None)
                .unwrap();
        let legacy = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Legacy Surface New Key",
            &json!({
                "id": "legacy-doc-new-key",
                "name": "Legacy New Key",
                "components": []
            }),
            &[],
        )
        .unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "legacy_new_key_allowed",
            &[op(
                "state.set",
                Some(&legacy.id),
                json!({ "values": { "brandNewKey": "hello" } }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(
            res.transaction.status, "applied",
            "expected legacy synthesis to allow a brand-new opaque key, got conflicts {:?}",
            res.conflicts
        );
        let (state, _) =
            crate::runtime_v2::surfaces::get_surface_state_with_revision(&db, &legacy.id).unwrap();
        assert_eq!(state["brandNewKey"], json!("hello"));
    }

    #[test]
    fn subscription_mutation_rejects_cross_conversation_target() {
        let mut db = test_db();
        let conv_a =
            crate::db::create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat A", None).unwrap();
        let conv_b =
            crate::db::create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat B", None).unwrap();

        let def = json!({
            "id": "surf-b",
            "name": "Surface B",
            "layout": "stack",
            "components": []
        });
        let surface_b =
            create_inline_surface(&mut db, &conv_b.id, None, None, "Surface B", &def, &[]).unwrap();

        // Transaction under conv_a tries to create subscription on surface_b (conv_b)
        let cross_txn = create_transaction(
            &mut db,
            Some(&conv_a.id),
            None,
            None,
            "cross_subscription",
            &[op(
                "subscription.create",
                Some(&surface_b.id),
                json!({
                    "subscriptionId": "sub-cross-1",
                    "channel": "chat",
                    "eventTypes": ["message"]
                }),
            )],
            false,
        )
        .unwrap();

        let res = apply_transaction(&mut db, &cross_txn.id).unwrap();
        assert_eq!(res.transaction.status, "failed");
        assert!(
            res.conflicts
                .iter()
                .any(|c| c.contains("Forbidden") || c.contains("cross-chat violation")),
            "cross-conversation subscription must fail with conflict, got {:?}",
            res.conflicts
        );
    }

    /// Authoritative path: component.replace must preserve user-input props via apply_transaction.
    #[test]
    fn component_replace_preserves_user_input_through_apply_transaction() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "PresTxn", None).unwrap();
        let def = json!({
            "id": "task-tracker",
            "name": "Task Tracker",
            "layout": "stack",
            "components": [{
                "id": "title-input",
                "type": "textInput",
                "valueKey": "taskTitle",
                "props": {
                    "label": "Title",
                    "value": "user-typing",
                    "preservationKey": "focus:taskTitle",
                    "preservationPolicy": "preserve_user_input"
                }
            }]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Tracker", &def, &[]).unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "replace_label",
            &[op(
                "component.replace",
                Some(&surface.id),
                json!({
                    "componentId": "title-input",
                    "preservationPolicy": "preserve_user_input",
                    "component": {
                        "id": "title-input",
                        "type": "textInput",
                        "valueKey": "taskTitle",
                        "props": {
                            "label": "Task title",
                            "preservationKey": "focus:taskTitle"
                        }
                    }
                }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(
            res.transaction.status, "applied",
            "conflicts: {:?}",
            res.conflicts
        );

        let surf = get_surface(&db, &surface.id).unwrap();
        let doc = crate::runtime_v2::SoftwareDocument::from_value(&surf.definition).unwrap();
        let comp = doc
            .find_component("title-input")
            .expect("component survives");
        assert_eq!(
            comp.props.as_ref().and_then(|p| p.get("value")),
            Some(&json!("user-typing")),
            "live value must survive replace through apply_transaction"
        );
        assert_eq!(
            comp.props.as_ref().and_then(|p| p.get("label")),
            Some(&json!("Task title")),
            "agent label change must apply"
        );

        // Reset policy must drop the live value.
        let reset_txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "reset_input",
            &[op(
                "component.replace",
                Some(&surface.id),
                json!({
                    "componentId": "title-input",
                    "preservationPolicy": "reset_explicitly",
                    "component": {
                        "id": "title-input",
                        "type": "textInput",
                        "valueKey": "taskTitle",
                        "props": { "label": "Title", "value": "" }
                    }
                }),
            )],
            false,
        )
        .unwrap();
        let reset_res = apply_transaction(&mut db, &reset_txn.id).unwrap();
        assert_eq!(reset_res.transaction.status, "applied");
        let surf2 = get_surface(&db, &surface.id).unwrap();
        let doc2 = crate::runtime_v2::SoftwareDocument::from_value(&surf2.definition).unwrap();
        let comp2 = doc2.find_component("title-input").unwrap();
        assert_eq!(
            comp2.props.as_ref().and_then(|p| p.get("value")),
            Some(&json!("")),
            "reset_explicitly must not keep prior value"
        );
    }

    #[test]
    fn preserve_focus_policy_does_not_merge_value_on_replace() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "PresDim", None).unwrap();
        let def = json!({
            "id": "d",
            "name": "Dim",
            "layout": "stack",
            "components": [{
                "id": "field",
                "type": "textInput",
                "props": {
                    "value": "typed",
                    "scrollTop": 99,
                    "label": "Old"
                }
            }]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Dim", &def, &[]).unwrap();
        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "focus_only",
            &[op(
                "component.replace",
                Some(&surface.id),
                json!({
                    "componentId": "field",
                    "preservationPolicy": "preserve_focus",
                    "component": {
                        "id": "field",
                        "type": "textInput",
                        "props": { "label": "New" }
                    }
                }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(res.transaction.status, "applied");
        let surf = get_surface(&db, &surface.id).unwrap();
        let doc = crate::runtime_v2::SoftwareDocument::from_value(&surf.definition).unwrap();
        let props = doc.find_component("field").unwrap().props.as_ref().unwrap();
        assert!(
            props.get("value").is_none(),
            "preserve_focus must not copy value"
        );
        assert!(
            props.get("scrollTop").is_none(),
            "preserve_focus must not copy scroll"
        );
        assert_eq!(props.get("label"), Some(&json!("New")));
    }

    /// Authoritative path: removing a bound input must drop its valueKey from surface state.
    #[test]
    fn component_remove_clears_value_key_state_through_apply_transaction() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "RemVk", None).unwrap();
        let def = json!({
            "id": "form",
            "name": "Form",
            "layout": "stack",
            "components": [
                {
                    "id": "title-input",
                    "type": "textInput",
                    "valueKey": "taskTitle",
                    "props": { "label": "Task title" }
                },
                {
                    "id": "keep-input",
                    "type": "textInput",
                    "valueKey": "notes",
                    "props": { "label": "Session notes" }
                }
            ]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Form", &def, &[]).unwrap();
        crate::runtime_v2::surfaces::save_surface_state_occ(
            &mut db,
            &surface.id,
            &json!({
                "taskTitle": "user typed title",
                "title-input": "legacy-id-key",
                "title-input:draft": "partial",
                "notes": "keep me",
                "unrelated": true
            }),
            1,
        )
        .unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "remove_title",
            &[op(
                "component.remove",
                Some(&surface.id),
                json!({ "componentId": "title-input" }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(
            res.transaction.status, "applied",
            "conflicts: {:?}",
            res.conflicts
        );

        let surf = get_surface(&db, &surface.id).unwrap();
        let doc = crate::runtime_v2::SoftwareDocument::from_value(&surf.definition).unwrap();
        assert!(
            doc.find_component("title-input").is_none(),
            "removed component must leave definition"
        );
        assert!(doc.find_component("keep-input").is_some());

        let (state, _) =
            crate::runtime_v2::surfaces::get_surface_state_with_revision(&db, &surface.id).unwrap();
        assert!(
            state.get("taskTitle").is_none(),
            "valueKey state must clear on remove, got {state}"
        );
        assert!(state.get("title-input").is_none());
        assert!(state.get("title-input:draft").is_none());
        assert_eq!(state.get("notes"), Some(&json!("keep me")));
        assert_eq!(state.get("unrelated"), Some(&json!(true)));
    }

    /// event.dispatch sets application_id when surface create auto-registers a manifest.
    #[test]
    fn event_dispatch_sets_application_id_when_surface_create_registers_manifest() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "EvtNoApp", None).unwrap();
        let tool = ToolDefinition {
            id: "tool-auto-manifest".into(),
            name: "Auto Manifest Tool".into(),
            description: String::new(),
            layout: json!("stack"),
            components: vec![ToolComponent {
                id: "t".into(),
                component_type: "text".into(),
                value_key: None,
                props: Some(json!({"text": "hi"})),
                children: None,
                ..Default::default()
            }],
            ..Default::default()
        };
        let saved = apply_tool_change(&mut db, DEFAULT_WORKSPACE_ID, &tool, "create", None, "seed")
            .unwrap();
        let surface = upsert_surface_from_tool(
            &mut db,
            &saved.definition,
            DEFAULT_WORKSPACE_ID,
            saved.current_version,
        )
        .unwrap();
        // First create always registers an application manifest.
        assert!(
            crate::application_kernel::manifest::get_manifest(&db, "tool-auto-manifest").is_ok()
        );
        db.conn()
            .execute(
                "UPDATE surfaces SET conversation_id = ?1 WHERE id = ?2",
                rusqlite::params![conv.id, surface.id],
            )
            .unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "dispatch",
            &[op(
                "event.dispatch",
                Some(&surface.id),
                json!({
                    "eventType": "custom.ping",
                    "scope": "surface",
                    "payload": { "ok": true }
                }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(
            res.transaction.status, "applied",
            "conflicts: {:?}",
            res.conflicts
        );

        let source_json: String = db
            .conn()
            .query_row(
                "SELECT source_json FROM surface_events ORDER BY created_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let source: serde_json::Value = serde_json::from_str(&source_json).unwrap();
        assert_eq!(
            source.get("toolId").and_then(|v| v.as_str()),
            Some("tool-auto-manifest"),
        );
        assert_eq!(
            source.get("applicationId").and_then(|v| v.as_str()),
            Some("tool-auto-manifest"),
            "application_id must come from the auto-registered manifest, got {source}"
        );
    }

    /// Surfaces inserted without a manifest must not invent application_id from tool_id.
    #[test]
    fn event_dispatch_omits_application_id_when_tool_has_no_manifest() {
        let mut db = test_db();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "EvtLegacyNoApp", None).unwrap();
        // Bypass upsert_surface_from_tool so no auto-manifest is created.
        let surface_id = "surf-tool-legacy-no-manifest";
        let now = crate::db::now_rfc3339();
        db.conn()
            .execute(
                "INSERT INTO tools (id, workspace_id, name, description, layout, definition_json, current_version, created_at, updated_at)
                 VALUES ('tool-legacy-no-manifest', ?1, 'Legacy', '', 'stack', '{}', 1, ?2, ?2)",
                rusqlite::params![DEFAULT_WORKSPACE_ID, now],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO surfaces (
                    id, instance_id, surface_type, placement, owner_type, owner_id,
                    conversation_id, tool_id, name, definition_json, current_revision,
                    lifecycle_state, archived, capability_packs_json, created_at, updated_at
                 ) VALUES (?1, 'inst-legacy', 'tool', 'tool_canvas', 'workspace', ?2, ?3,
                    'tool-legacy-no-manifest', 'Legacy', '{\"components\":[]}', 1, 'active', 0, '[]', ?4, ?4)",
                rusqlite::params![surface_id, DEFAULT_WORKSPACE_ID, conv.id, now],
            )
            .unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "dispatch",
            &[op(
                "event.dispatch",
                Some(surface_id),
                json!({
                    "eventType": "custom.ping",
                    "scope": "surface",
                    "payload": { "ok": true }
                }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(
            res.transaction.status, "applied",
            "conflicts: {:?}",
            res.conflicts
        );

        let source_json: String = db
            .conn()
            .query_row(
                "SELECT source_json FROM surface_events ORDER BY created_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let source: serde_json::Value = serde_json::from_str(&source_json).unwrap();
        assert_eq!(
            source.get("toolId").and_then(|v| v.as_str()),
            Some("tool-legacy-no-manifest"),
        );
        assert!(
            source.get("applicationId").is_none()
                || source.get("applicationId") == Some(&json!(null)),
            "application_id must not equal tool_id without a manifest, got {source}"
        );
    }

    #[test]
    fn event_dispatch_sets_application_id_when_manifest_exists() {
        use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
        use std::collections::HashMap;

        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "EvtWithApp", None).unwrap();
        let tool_id = "app-with-manifest";
        let tool = ToolDefinition {
            id: tool_id.into(),
            name: "Manifested Tool".into(),
            description: String::new(),
            layout: json!("stack"),
            components: vec![ToolComponent {
                id: "t".into(),
                component_type: "text".into(),
                value_key: None,
                props: Some(json!({"text": "hi"})),
                children: None,
                ..Default::default()
            }],
            ..Default::default()
        };
        let saved = apply_tool_change(&mut db, DEFAULT_WORKSPACE_ID, &tool, "create", None, "seed")
            .unwrap();
        let surface = upsert_surface_from_tool(
            &mut db,
            &saved.definition,
            DEFAULT_WORKSPACE_ID,
            saved.current_version,
        )
        .unwrap();
        db.conn()
            .execute(
                "UPDATE surfaces SET conversation_id = ?1 WHERE id = ?2",
                rusqlite::params![conv.id, surface.id],
            )
            .unwrap();
        upsert_manifest(
            &mut db,
            ApplicationManifest {
                schema_version: "1".into(),
                application_id: tool_id.into(),
                instance_id: "instance-1".into(),
                name: "Manifested".into(),
                description: String::new(),
                version: 1,
                surfaces: vec![],
                routes: vec![],
                data_models: vec![],
                settings: vec![],
                capabilities: vec!["coreside.core".into()],
                permissions: vec![],
                events: vec![],
                tests: vec![],
                search_keywords: vec![],
                tags: vec![],
                agent_description: String::new(),
                project_id: None,
                conversation_id: None,
                organization_id: None,
                ownership: None,
                application_action_access: vec![],
                surface_action_access: HashMap::new(),
                component_action_access: HashMap::new(),
                action_descriptor_hashes: HashMap::new(),
            },
        )
        .unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "dispatch",
            &[op(
                "event.dispatch",
                Some(&surface.id),
                json!({
                    "eventType": "custom.ping",
                    "scope": "surface",
                    "payload": {}
                }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(
            res.transaction.status, "applied",
            "conflicts: {:?}",
            res.conflicts
        );

        let source_json: String = db
            .conn()
            .query_row(
                "SELECT source_json FROM surface_events ORDER BY created_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let source: serde_json::Value = serde_json::from_str(&source_json).unwrap();
        assert_eq!(
            source.get("applicationId").and_then(|v| v.as_str()),
            Some(tool_id),
            "manifested tool must set application_id, got {source}"
        );
    }

    #[test]
    fn event_dispatch_rejects_spoofed_target_application_id() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "EvtSpoof", None).unwrap();
        let tool = ToolDefinition {
            id: "tool-local".into(),
            name: "Local".into(),
            description: String::new(),
            layout: json!("stack"),
            components: vec![ToolComponent {
                id: "t".into(),
                component_type: "text".into(),
                value_key: None,
                props: Some(json!({"text": "hi"})),
                children: None,
                ..Default::default()
            }],
            ..Default::default()
        };
        let saved = apply_tool_change(&mut db, DEFAULT_WORKSPACE_ID, &tool, "create", None, "seed")
            .unwrap();
        let surface = upsert_surface_from_tool(
            &mut db,
            &saved.definition,
            DEFAULT_WORKSPACE_ID,
            saved.current_version,
        )
        .unwrap();
        db.conn()
            .execute(
                "UPDATE surfaces SET conversation_id = ?1 WHERE id = ?2",
                rusqlite::params![conv.id, surface.id],
            )
            .unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "spoof-target",
            &[op(
                "event.dispatch",
                Some(&surface.id),
                json!({
                    "eventType": "custom.ping",
                    "scope": "surface",
                    "target": {
                        "surfaceId": surface.id,
                        "applicationId": "app-other-spoofed"
                    },
                    "payload": {}
                }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_ne!(
            res.transaction.status, "applied",
            "spoofed applicationId must fail closed, conflicts={:?}",
            res.conflicts
        );
        assert!(
            res.conflicts
                .iter()
                .any(|c| c.contains("spoof") || c.contains("application")),
            "expected spoof denial in conflicts: {:?}",
            res.conflicts
        );
    }

    #[test]
    fn component_replace_preserves_live_surface_state_over_empty_definition_props() {
        let mut db = test_db();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "LivePreserve", None).unwrap();
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Tracker",
            &json!({
                "id": "tool-tracker",
                "name": "Tracker",
                "layout": "stack",
                "components": [{
                    "id": "title-input",
                    "type": "textInput",
                    "valueKey": "taskTitle",
                    "props": { "label": "Title", "value": "" }
                }]
            }),
            &[],
        )
        .unwrap();
        crate::runtime_v2::surfaces::save_surface_state_occ(
            &mut db,
            &surface.id,
            &json!({ "taskTitle": "Buy milk" }),
            1,
        )
        .unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "replace-input",
            &[op(
                "component.replace",
                Some(&surface.id),
                json!({
                    "componentId": "title-input",
                    "component": {
                        "id": "title-input",
                        "type": "textInput",
                        "valueKey": "taskTitle",
                        "props": { "label": "Title", "value": "", "placeholder": "Task title" }
                    }
                }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(
            res.transaction.status, "applied",
            "conflicts: {:?}",
            res.conflicts
        );

        let (state, _) =
            crate::runtime_v2::surfaces::get_surface_state_with_revision(&db, &surface.id).unwrap();
        assert_eq!(
            state.get("taskTitle"),
            Some(&json!("Buy milk")),
            "live surface_state must survive replace, got {state}"
        );
        let surf = get_surface(&db, &surface.id).unwrap();
        let doc = crate::runtime_v2::SoftwareDocument::from_value(&surf.definition).unwrap();
        let comp = doc.find_component("title-input").unwrap();
        assert_eq!(
            comp.props.as_ref().and_then(|p| p.get("value")),
            Some(&json!("Buy milk")),
            "definition props.value must overlay live state, got {:?}",
            comp.props
        );
    }

    #[test]
    fn setting_create_and_route_navigate_apply_without_unsupported_operation() {
        use crate::application_kernel::manifest::{ensure_manifest_for_tool, get_manifest};

        let mut db = test_db();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "SettingRoute", None).unwrap();
        let def = json!({
            "id": "d",
            "name": "Nav Surface",
            "layout": "stack",
            "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Nav Surface", &def, &[]).unwrap();
        ensure_manifest_for_tool(
            &mut db,
            "app-setting-route",
            "Setting Route App",
            &surface.id,
        )
        .unwrap();

        let mut setting_op = op(
            "setting.create",
            None,
            json!({
                "id": "setting.daily-goal",
                "label": "Daily goal",
                "valueType": "integer",
                "default": 3,
                "applicationId": "app-setting-route",
            }),
        );
        setting_op.target.application_id = Some("app-setting-route".into());
        // Identity binding: tool_id must match applicationId (no cross-app spoof).
        setting_op.target.tool_id = Some("app-setting-route".into());

        let navigate_op = op(
            "route.navigate",
            Some(&surface.id),
            json!({ "routeId": "overview" }),
        );

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "setting+navigate",
            &[setting_op, navigate_op],
            false,
        )
        .unwrap();
        let result = apply_transaction(&mut db, &txn.id).unwrap();
        assert_eq!(
            result.transaction.status, "applied",
            "expected applied, conflicts={:?}",
            result.conflicts
        );
        assert!(
            result
                .conflicts
                .iter()
                .all(|c| !c.contains("unsupported operation")),
            "setting.create / route.navigate must not hit unsupported operation, got {:?}",
            result.conflicts
        );

        let manifest = get_manifest(&db, "app-setting-route").unwrap();
        assert!(
            manifest
                .manifest
                .settings
                .iter()
                .any(|s| s == "setting.daily-goal"),
            "setting.create must append setting id to manifest, got {:?}",
            manifest.manifest.settings
        );
        let (state, _) =
            crate::runtime_v2::surfaces::get_surface_state_with_revision(&db, &surface.id).unwrap();
        assert_eq!(
            state.get("activeRoute"),
            Some(&json!("overview")),
            "route.navigate must store activeRoute on surface state"
        );
    }

    #[test]
    fn setting_create_rejects_unbound_application_id_spoof() {
        use crate::application_kernel::manifest::ensure_manifest_for_tool;

        let mut db = test_db();
        let conv =
            create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "SettingSpoof", None).unwrap();
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Victim",
            &json!({
                "id": "d",
                "name": "Victim",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "hi"}}]
            }),
            &[],
        )
        .unwrap();
        ensure_manifest_for_tool(&mut db, "app-victim", "Victim", &surface.id).unwrap();
        ensure_manifest_for_tool(&mut db, "app-attacker", "Attacker", "surf-unrelated").unwrap();

        let mut setting_op = op(
            "setting.create",
            None,
            json!({
                "id": "setting.pwned",
                "applicationId": "app-victim",
            }),
        );
        setting_op.target.application_id = Some("app-victim".into());
        // No surface/tool binding — must fail closed.
        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "spoof-setting",
            &[setting_op],
            false,
        )
        .unwrap();
        let result = apply_transaction(&mut db, &txn.id).unwrap();
        assert_ne!(
            result.transaction.status, "applied",
            "unbound setting.create must fail, conflicts={:?}",
            result.conflicts
        );
        assert!(
            result
                .conflicts
                .iter()
                .any(|c| c.contains("requires a surface or tool target")),
            "expected binding denial, got {:?}",
            result.conflicts
        );
    }

    #[test]
    fn apply_one_rejects_state_patch_when_application_suspended() {
        use crate::application_kernel::manifest::{upsert_manifest, ApplicationManifest};
        use std::collections::HashMap;

        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "SuspState", None).unwrap();
        upsert_manifest(
            &mut db,
            ApplicationManifest {
                schema_version: "1".into(),
                application_id: "app-susp".into(),
                instance_id: "inst-susp".into(),
                name: "Susp".into(),
                description: String::new(),
                version: 1,
                surfaces: vec![],
                routes: vec![],
                data_models: vec![],
                settings: vec![],
                capabilities: vec!["coreside.core".into()],
                permissions: vec![],
                events: vec![],
                tests: vec![],
                search_keywords: vec![],
                tags: vec![],
                agent_description: String::new(),
                project_id: None,
                conversation_id: Some(conv.id.clone()),
                organization_id: None,
                ownership: None,
                application_action_access: vec![],
                surface_action_access: HashMap::new(),
                component_action_access: HashMap::new(),
                action_descriptor_hashes: HashMap::new(),
            },
        )
        .unwrap();
        db.conn()
            .execute(
                "INSERT OR IGNORE INTO tools (
                    id, workspace_id, name, description, layout, definition_json,
                    current_version, created_at, updated_at
                 ) VALUES ('app-susp', ?1, 'Susp', '', 'stack', '{}', 1, datetime('now'), datetime('now'))",
                [DEFAULT_WORKSPACE_ID],
            )
            .unwrap();
        db.conn()
            .execute(
                "UPDATE application_manifests SET lifecycle_state = 'suspended', disabled = 1
                 WHERE application_id = 'app-susp'",
                [],
            )
            .unwrap();

        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Susp Surface",
            &json!({
                "id": "doc-susp",
                "name": "Susp",
                "layout": "stack",
                "components": [{"id": "t", "type": "text", "props": {"text": "x"}}]
            }),
            &[],
        )
        .unwrap();
        crate::runtime_v2::surfaces::bind_surface_tool_id(&mut db, &surface.id, "app-susp")
            .unwrap();

        let txn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "patch-while-suspended",
            &[op(
                "state.patch",
                Some(&surface.id),
                json!({ "patch": { "n": 1 } }),
            )],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut db, &txn.id).unwrap();
        assert_ne!(res.transaction.status, "applied");
        assert!(
            res.conflicts
                .iter()
                .any(|c| c.contains("suspended") || c.contains("disabled")),
            "expected suspended/disabled conflict, got {:?}",
            res.conflicts
        );
    }

    #[test]
    fn undo_one_turn_and_two_turn_application_evolution() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "MultiUndo", None).unwrap();
        let def = json!({
            "id": "doc",
            "name": "Tracker",
            "layout": "stack",
            "components": [
                {"id": "title", "type": "heading", "props": {"text": "V0"}},
                {"id": "body", "type": "text", "props": {"text": "base"}}
            ]
        });
        let surface =
            create_inline_surface(&mut db, &conv.id, None, None, "Tracker", &def, &[]).unwrap();
        let rev0 = surface.current_revision;

        let turn1 = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "evolve-v1",
            &[op(
                "component.update_props",
                Some(&surface.id),
                json!({ "componentId": "title", "props": { "text": "V1" } }),
            )],
            false,
        )
        .unwrap();
        let r1 = apply_transaction(&mut db, &turn1.id).unwrap();
        assert_eq!(r1.transaction.status, "applied", "{:?}", r1.conflicts);
        let after1 = get_surface(&db, &surface.id).unwrap();
        assert!(after1.current_revision > rev0);
        let doc1 = crate::runtime_v2::SoftwareDocument::from_value(&after1.definition).unwrap();
        assert_eq!(
            doc1.find_component("title")
                .and_then(|c| c.props.as_ref())
                .and_then(|p| p.get("text")),
            Some(&json!("V1"))
        );

        // One-turn undo restores V0.
        let undone1 = undo_transaction(&mut db, &turn1.id).unwrap();
        assert_eq!(undone1.status, "reverted");
        let restored0 = get_surface(&db, &surface.id).unwrap();
        let doc0 = crate::runtime_v2::SoftwareDocument::from_value(&restored0.definition).unwrap();
        assert_eq!(
            doc0.find_component("title")
                .and_then(|c| c.props.as_ref())
                .and_then(|p| p.get("text")),
            Some(&json!("V0"))
        );

        // Re-apply turn1, then turn2, then undo both (LIFO).
        let turn1b = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "evolve-v1-b",
            &[op(
                "component.update_props",
                Some(&surface.id),
                json!({ "componentId": "title", "props": { "text": "V1" } }),
            )],
            false,
        )
        .unwrap();
        let r1b = apply_transaction(&mut db, &turn1b.id).unwrap();
        assert_eq!(
            r1b.transaction.status,
            "applied",
            "turn1b conflicts={:?} restored_rev={}",
            r1b.conflicts,
            get_surface(&db, &surface.id).unwrap().current_revision
        );
        let turn2 = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "evolve-v2",
            &[op(
                "component.update_props",
                Some(&surface.id),
                json!({ "componentId": "title", "props": { "text": "V2" } }),
            )],
            false,
        )
        .unwrap();
        assert_eq!(
            apply_transaction(&mut db, &turn2.id)
                .unwrap()
                .transaction
                .status,
            "applied"
        );
        let at_v2 = get_surface(&db, &surface.id).unwrap();
        let doc_v2 = crate::runtime_v2::SoftwareDocument::from_value(&at_v2.definition).unwrap();
        assert_eq!(
            doc_v2
                .find_component("title")
                .and_then(|c| c.props.as_ref())
                .and_then(|p| p.get("text")),
            Some(&json!("V2"))
        );

        undo_transaction(&mut db, &turn2.id).unwrap();
        let mid = get_surface(&db, &surface.id).unwrap();
        let doc_mid = crate::runtime_v2::SoftwareDocument::from_value(&mid.definition).unwrap();
        assert_eq!(
            doc_mid
                .find_component("title")
                .and_then(|c| c.props.as_ref())
                .and_then(|p| p.get("text")),
            Some(&json!("V1")),
            "undoing second turn must leave first turn applied"
        );

        undo_transaction(&mut db, &turn1b.id).unwrap();
        let end = get_surface(&db, &surface.id).unwrap();
        let doc_end = crate::runtime_v2::SoftwareDocument::from_value(&end.definition).unwrap();
        assert_eq!(
            doc_end
                .find_component("title")
                .and_then(|c| c.props.as_ref())
                .and_then(|p| p.get("text")),
            Some(&json!("V0")),
            "two-turn undo must restore original definition"
        );
    }

    #[test]
    fn undo_restores_linked_tool_current_version() {
        let mut db = test_db();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "ToolUndo", None).unwrap();
        let original = ToolDefinition {
            id: "tool-undo-ver".into(),
            name: "Undo Ver".into(),
            description: String::new(),
            layout: json!({"type": "single-column"}),
            components: vec![ToolComponent {
                id: "title".into(),
                component_type: "heading".into(),
                value_key: None,
                props: Some(json!({"text": "V0"})),
                children: None,
                ..Default::default()
            }],
            ..Default::default()
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
        let before = get_tool(&db, &original.id).unwrap();
        let rev0 = get_surface(&db, &surface.id).unwrap().current_revision;
        assert_eq!(before.current_version, rev0);

        let turn = create_transaction(
            &mut db,
            Some(&conv.id),
            None,
            None,
            "evolve-tool",
            &[op(
                "component.update_props",
                Some(&surface.id),
                json!({ "componentId": "title", "props": { "text": "V1" } }),
            )],
            false,
        )
        .unwrap();
        let applied = apply_transaction(&mut db, &turn.id).unwrap();
        assert_eq!(applied.transaction.status, "applied", "{:?}", applied.conflicts);
        let mid_tool = get_tool(&db, &original.id).unwrap();
        let mid_surf = get_surface(&db, &surface.id).unwrap();
        assert!(mid_surf.current_revision > rev0);
        assert_eq!(
            mid_tool.current_version, mid_surf.current_revision,
            "apply must keep tools.current_version coupled to surface revision"
        );

        undo_transaction(&mut db, &turn.id).unwrap();
        let end_tool = get_tool(&db, &original.id).unwrap();
        let end_surf = get_surface(&db, &surface.id).unwrap();
        assert_eq!(end_surf.current_revision, rev0);
        assert_eq!(
            end_tool.current_version, rev0,
            "undo must restore tools.current_version with surface revision"
        );
        assert_eq!(end_tool.id, original.id);
        let doc = crate::runtime_v2::SoftwareDocument::from_value(&end_surf.definition).unwrap();
        assert_eq!(
            doc.find_component("title")
                .and_then(|c| c.props.as_ref())
                .and_then(|p| p.get("text")),
            Some(&json!("V0"))
        );
    }
}
