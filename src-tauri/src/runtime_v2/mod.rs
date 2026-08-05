//! Generative Interface Runtime V2 — safe Partial Update adaptations.

pub mod app_routes;
pub mod branch;
pub mod context_ledger;
pub mod continuity;
pub mod drafts;
pub mod events;
pub mod limits;
pub mod operations;
pub mod packs;
pub mod patch;
pub mod patch_scheduler;
pub mod preservation;
pub mod preview_transaction;
pub mod provider_conformance;
pub mod queue;
pub mod streaming;
pub mod surfaces;
pub mod transactions;
pub mod turn_timeline;

#[allow(unused_imports)]
pub use app_routes::{
    get_route_state, navigate_route, set_route_state, NavigateResult, RouteState,
};
#[allow(unused_imports)]
pub use branch::{
    branch_from_message, create_snapshot, delete_snapshot, get_snapshot, list_branches,
    list_snapshots, ChatBranchRecord, SnapshotRecord,
};
#[allow(unused_imports)]
pub use context_ledger::{
    append_ledger_entry, get_ledger_entry, ledger_submission_id, list_ledger_entries,
    list_ledger_entries_for_inject, mark_ledger_consumed, should_consume_after_inject,
    ContextLedgerEntry,
};
#[allow(unused_imports)]
pub use continuity::{
    get_continuity, save_continuity, suspend_surface, ContinuitySnapshot, SuspensionState,
};
#[allow(unused_imports)]
pub use drafts::{
    delete_draft, delete_drafts_for_conversation, delete_drafts_for_surface, get_draft, save_draft,
    DraftConflict, SurfaceDraft,
};
#[allow(unused_imports)]
pub use events::{EventBus, EventBusError, EventRef, Subscription, SurfaceEvent};
#[allow(unused_imports)]
pub use operations::{
    tool_change_to_operations, validate_operations, AgentResponseV2, AppOperation,
    SCHEMA_VERSION_V2,
};
#[allow(unused_imports)]
pub use packs::{
    agent_pack_catalog_markdown, bundled_packs, validate_component_type_allowed,
    validate_definition_components,
};
#[allow(unused_imports)]
pub use patch::{apply_component_op, check_revision, PatchConflict};
#[allow(unused_imports)]
pub use patch_scheduler::{
    detect_dependency_cycle, flush_scheduler, get_scheduled_patch, preview_rate_limit_hz,
    record_manual_edit_provenance, schedule_and_apply, schedule_patches, topological_order,
    validate_agent_priority, PatchPriority, ScheduleAndApplyResult, ScheduleRequest,
    ScheduledPatch,
};
#[allow(unused_imports)]
pub use preservation::{
    apply_preservation_on_replace, get_preservation, invalidate_component_live_state,
    list_preservation_for_surface, resolve_policy_for_apply, should_preserve, upsert_preservation,
    PreservationPolicy, PreservationRecord,
};
#[allow(unused_imports)]
pub use provider_conformance::{
    get_provider_profile, seed_provider_profiles, select_application_profile,
    ProviderConformanceRecord, ProviderProfile,
};
#[allow(unused_imports)]
pub use queue::{
    activate_next, cancel as cancel_queue_item, complete as complete_queue_item, enqueue, get_item,
    list_queue, recover_stale_active, remove_queued, requeue as requeue_queue_item, QueueItem,
};
#[allow(unused_imports)]
pub use preview_transaction::{
    ingest_live_chunk, ingest_live_chunk_with_seed, PreviewOpEvent, PreviewPaintEvent,
    PreviewSurfaceModel, PreviewTransaction, RejectedPreviewOp,
};
#[allow(unused_imports)]
pub use streaming::{NdjsonFrameParser, StreamEvent, StreamParseError, StreamParseErrorKind};
#[allow(unused_imports)]
pub use surfaces::{
    archive_surface, create_inline_surface, delete_surface, get_surface, get_surface_state,
    list_inline_surfaces, promote_inline_to_tool, restore_surface, save_surface_state,
    update_surface_definition, upsert_surface_from_tool, DeleteSurfaceOptions, SurfaceRecord,
};
#[allow(unused_imports)]
pub use transactions::{
    apply_transaction, apply_transaction_with_bus, create_transaction, get_transaction,
    list_transactions, undo_transaction, AppTransactionRecord, ApplyResult,
};
pub use turn_timeline::{
    list_turn_timeline_events, try_append_turn_timeline_event, TurnTimelineEvent,
};

use serde_json::Value;
use uuid::Uuid;

use crate::db::{now_rfc3339, Database, DbResult};
use crate::security::redact_secrets;
use rusqlite::params;

/// Store redacted developer diagnostics (Developer Mode).
pub fn store_diagnostics(
    db: &mut Database,
    conversation_id: Option<&str>,
    turn_id: Option<&str>,
    payload: &Value,
    api_key: Option<&str>,
) -> DbResult<String> {
    let id = format!("diag-{}", Uuid::new_v4());
    let now = now_rfc3339();
    let raw = payload.to_string();
    let redacted = redact_secrets(&raw, api_key);
    db.conn().execute(
        "INSERT INTO developer_diagnostics (id, conversation_id, turn_id, payload_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, conversation_id, turn_id, redacted, now],
    )?;
    // Retention trim
    db.conn().execute(
        "DELETE FROM developer_diagnostics WHERE id NOT IN (
            SELECT id FROM developer_diagnostics ORDER BY created_at DESC LIMIT ?1
         )",
        [limits::MAX_DIAGNOSTICS_RETENTION as i64],
    )?;
    Ok(id)
}

pub fn list_diagnostics(
    db: &Database,
    conversation_id: &str,
    limit: usize,
) -> DbResult<Vec<Value>> {
    let mut stmt = db.conn().prepare(
        "SELECT payload_json FROM developer_diagnostics
         WHERE conversation_id = ?1 ORDER BY created_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![conversation_id, limit as i64], |row| {
        let s: String = row.get(0)?;
        Ok(serde_json::from_str(&s).unwrap_or(Value::Null))
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(crate::db::DbError::Sqlite)?);
    }
    Ok(out)
}
