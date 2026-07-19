//! Tauri commands for Generative Interface Runtime V2.

use serde::Deserialize;
use serde_json::Value;
use tauri::State;

use super::CommandError;
use crate::runtime_v2::{
    self, activate_next, append_ledger_entry, branch_from_message,
    bundled_packs, cancel_queue_item, complete_queue_item, create_inline_surface, create_snapshot,
    delete_snapshot, enqueue, flush_scheduler, get_continuity, get_draft, get_provider_profile,
    get_route_state, get_snapshot, get_surface, get_surface_state, get_transaction, list_branches,
    list_diagnostics, list_inline_surfaces, list_ledger_entries, list_queue, list_transactions,
    navigate_route, promote_inline_to_tool, recover_stale_active, remove_queued, save_continuity,
    save_draft, save_surface_state, schedule_and_apply, schedule_patches, set_route_state,
    store_diagnostics, suspend_surface, undo_transaction, update_surface_definition,
    AgentResponseV2, AppOperation, AppTransactionRecord, ApplyResult, ChatBranchRecord,
    ContinuitySnapshot, ContextLedgerEntry, NavigateResult, PatchPriority, ProviderConformanceRecord,
    QueueItem, RouteState, ScheduleRequest, ScheduledPatch,
    SnapshotRecord, SurfaceDraft, SurfaceRecord, SuspensionState,
};
use crate::runtime_v2::packs::CapabilityPackMeta as PackMeta;
use crate::state::AppState;

#[tauri::command]
pub fn list_capability_packs() -> Result<Vec<PackMeta>, CommandError> {
    Ok(bundled_packs())
}

#[tauri::command]
pub fn list_conversation_surfaces(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<SurfaceRecord>, CommandError> {
    let db = state.db.lock();
    Ok(list_inline_surfaces(&db, &conversation_id)?)
}

#[tauri::command]
pub fn get_surface_cmd(
    state: State<'_, AppState>,
    surface_id: String,
) -> Result<SurfaceRecord, CommandError> {
    let db = state.db.lock();
    Ok(get_surface(&db, &surface_id)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInlineSurfaceArgs {
    pub conversation_id: String,
    pub message_id: Option<String>,
    pub project_id: Option<String>,
    pub name: String,
    pub definition: Value,
    pub capability_packs: Option<Vec<String>>,
}

#[tauri::command]
pub fn create_inline_surface_cmd(
    state: State<'_, AppState>,
    args: CreateInlineSurfaceArgs,
) -> Result<SurfaceRecord, CommandError> {
    let mut db = state.db.lock();
    let packs = args.capability_packs.unwrap_or_default();
    Ok(create_inline_surface(
        &mut db,
        &args.conversation_id,
        args.message_id.as_deref(),
        args.project_id.as_deref(),
        &args.name,
        &args.definition,
        &packs,
    )?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSurfaceArgs {
    pub surface_id: String,
    pub definition: Value,
    pub change_summary: Option<String>,
    pub base_revision: Option<i64>,
}

#[tauri::command]
pub fn update_surface_cmd(
    state: State<'_, AppState>,
    args: UpdateSurfaceArgs,
) -> Result<SurfaceRecord, CommandError> {
    let mut db = state.db.lock();
    Ok(update_surface_definition(
        &mut db,
        &args.surface_id,
        &args.definition,
        args.change_summary.as_deref().unwrap_or("update"),
        args.base_revision,
    )?)
}

#[tauri::command]
pub fn promote_surface_cmd(
    state: State<'_, AppState>,
    surface_id: String,
) -> Result<SurfaceRecord, CommandError> {
    let mut db = state.db.lock();
    Ok(promote_inline_to_tool(
        &mut db,
        &surface_id,
        "ws-personal-default",
    )?)
}

#[tauri::command]
pub fn save_surface_state_cmd(
    state: State<'_, AppState>,
    surface_id: String,
    state_json: Value,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    Ok(save_surface_state(&mut db, &surface_id, &state_json)?)
}

#[tauri::command]
pub fn get_surface_state_cmd(
    state: State<'_, AppState>,
    surface_id: String,
) -> Result<Value, CommandError> {
    let db = state.db.lock();
    Ok(get_surface_state(&db, &surface_id)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDraftArgs {
    pub surface_id: String,
    pub component_id: String,
    pub window_id: Option<String>,
    pub base_revision: i64,
    pub draft: Value,
    pub form_id: Option<String>,
    pub persistence_policy: Option<String>,
    pub force: Option<bool>,
}

#[tauri::command]
pub fn get_draft_cmd(
    state: State<'_, AppState>,
    surface_id: String,
    component_id: String,
    window_id: Option<String>,
) -> Result<Option<SurfaceDraft>, CommandError> {
    let db = state.db.lock();
    Ok(get_draft(
        &db,
        &surface_id,
        &component_id,
        window_id.as_deref().unwrap_or("main"),
    )?)
}

#[tauri::command]
pub fn save_draft_cmd(
    state: State<'_, AppState>,
    args: SaveDraftArgs,
) -> Result<SurfaceDraft, CommandError> {
    let mut db = state.db.lock();
    save_draft(
        &mut db,
        &args.surface_id,
        &args.component_id,
        args.window_id.as_deref().unwrap_or("main"),
        args.base_revision,
        &args.draft,
        args.form_id.as_deref(),
        args.persistence_policy.as_deref(),
        args.force.unwrap_or(false),
    )
    .map_err(|c| {
        CommandError::new(
            "draft_conflict",
            format!(
                "draft revision conflict: stored {}, requested {}",
                c.stored_revision, c.requested_revision
            ),
        )
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulePatchesArgs {
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub surface_id: Option<String>,
    pub priority: String,
    pub operations: Vec<AppOperation>,
    pub source_type: String,
    pub from_agent: Option<bool>,
    pub apply_immediately: Option<bool>,
    pub approval_granted: Option<bool>,
}

#[tauri::command]
pub fn schedule_patches_cmd(
    state: State<'_, AppState>,
    args: SchedulePatchesArgs,
) -> Result<Vec<ScheduledPatch>, CommandError> {
    let mut db = state.db.lock();
    let priority = PatchPriority::parse(&args.priority)
        .ok_or_else(|| CommandError::new("invalid", "unknown patch priority"))?;
    let req = ScheduleRequest {
        conversation_id: args.conversation_id,
        turn_id: args.turn_id,
        surface_id: args.surface_id,
        priority,
        operations: args.operations,
        source_type: args.source_type,
        from_agent: args.from_agent.unwrap_or(false),
    };
    if args.apply_immediately.unwrap_or(false) {
        let mut bus = state.event_bus.lock();
        let mut bus_opt = Some(&mut *bus);
        let result = schedule_and_apply(
            &mut db,
            &mut bus_opt,
            req,
            args.approval_granted.unwrap_or(true),
        )?;
        return Ok(result.scheduled);
    }
    Ok(schedule_patches(&mut db, &req)?)
}

#[tauri::command]
pub fn flush_patch_scheduler_cmd(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    source_type: Option<String>,
    approval_granted: Option<bool>,
) -> Result<Vec<crate::application_kernel::ChangeResult>, CommandError> {
    let mut db = state.db.lock();
    let mut bus = state.event_bus.lock();
    let mut bus_opt = Some(&mut *bus);
    Ok(flush_scheduler(
        &mut db,
        &mut bus_opt,
        conversation_id.as_deref(),
        source_type.as_deref().unwrap_or("user"),
        approval_granted.unwrap_or(true),
    )?)
}

#[tauri::command]
pub fn get_route_state_cmd(
    state: State<'_, AppState>,
    application_id: String,
    window_id: Option<String>,
) -> Result<RouteState, CommandError> {
    let db = state.db.lock();
    Ok(get_route_state(
        &db,
        &application_id,
        window_id.as_deref().unwrap_or("main"),
    )?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRouteStateArgs {
    pub application_id: String,
    pub window_id: Option<String>,
    pub current_route_id: Option<String>,
    pub route_params: Value,
    pub history: Vec<Value>,
    pub history_index: i64,
}

#[tauri::command]
pub fn set_route_state_cmd(
    state: State<'_, AppState>,
    args: SetRouteStateArgs,
) -> Result<RouteState, CommandError> {
    let mut db = state.db.lock();
    Ok(set_route_state(
        &mut db,
        &args.application_id,
        args.window_id.as_deref().unwrap_or("main"),
        args.current_route_id.as_deref(),
        &args.route_params,
        &args.history,
        args.history_index,
    )?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateRouteArgs {
    pub application_id: String,
    pub window_id: Option<String>,
    pub route_id: String,
    pub route_params: Value,
    pub push_history: Option<bool>,
}

#[tauri::command]
pub fn navigate_route_cmd(
    state: State<'_, AppState>,
    args: NavigateRouteArgs,
) -> Result<NavigateResult, CommandError> {
    let mut db = state.db.lock();
    Ok(navigate_route(
        &mut db,
        &args.application_id,
        args.window_id.as_deref().unwrap_or("main"),
        &args.route_id,
        &args.route_params,
        args.push_history.unwrap_or(true),
    )?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendLedgerArgs {
    pub conversation_id: String,
    pub project_id: Option<String>,
    pub branch_id: Option<String>,
    pub entry_type: String,
    pub visibility: Option<String>,
    pub payload: Value,
    pub summary: String,
    pub expiration_class: Option<String>,
}

#[tauri::command]
pub fn append_context_ledger_cmd(
    state: State<'_, AppState>,
    args: AppendLedgerArgs,
) -> Result<ContextLedgerEntry, CommandError> {
    let mut db = state.db.lock();
    Ok(append_ledger_entry(
        &mut db,
        &args.conversation_id,
        args.project_id.as_deref(),
        args.branch_id.as_deref(),
        &args.entry_type,
        args.visibility.as_deref().unwrap_or("model_context_only"),
        &args.payload,
        &args.summary,
        args.expiration_class.as_deref(),
    )?)
}

#[tauri::command]
pub fn list_context_ledger_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    project_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ContextLedgerEntry>, CommandError> {
    let db = state.db.lock();
    Ok(list_ledger_entries(
        &db,
        &conversation_id,
        project_id.as_deref(),
        limit.unwrap_or(50),
    )?)
}

#[tauri::command]
pub fn get_provider_profile_cmd(
    state: State<'_, AppState>,
    provider_id: String,
    model_id: Option<String>,
) -> Result<ProviderConformanceRecord, CommandError> {
    let mut db = state.db.lock();
    Ok(get_provider_profile(
        &mut db,
        &provider_id,
        model_id.as_deref().unwrap_or("*"),
    )?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveContinuityArgs {
    pub surface_id: String,
    pub window_id: Option<String>,
    pub focus: Value,
    pub scroll: Value,
    pub media: Value,
    pub suspension_state: Option<String>,
}

#[tauri::command]
pub fn get_continuity_cmd(
    state: State<'_, AppState>,
    surface_id: String,
    window_id: Option<String>,
) -> Result<ContinuitySnapshot, CommandError> {
    let db = state.db.lock();
    Ok(get_continuity(
        &db,
        &surface_id,
        window_id.as_deref().unwrap_or("main"),
    )?)
}

#[tauri::command]
pub fn save_continuity_cmd(
    state: State<'_, AppState>,
    args: SaveContinuityArgs,
) -> Result<ContinuitySnapshot, CommandError> {
    let mut db = state.db.lock();
    let suspension = args
        .suspension_state
        .as_deref()
        .map(SuspensionState::parse)
        .unwrap_or(SuspensionState::Active);
    Ok(save_continuity(
        &mut db,
        &args.surface_id,
        args.window_id.as_deref().unwrap_or("main"),
        &args.focus,
        &args.scroll,
        &args.media,
        suspension,
    )?)
}

#[tauri::command]
pub fn suspend_surface_cmd(
    state: State<'_, AppState>,
    surface_id: String,
    window_id: Option<String>,
) -> Result<ContinuitySnapshot, CommandError> {
    let mut db = state.db.lock();
    Ok(suspend_surface(
        &mut db,
        &surface_id,
        window_id.as_deref().unwrap_or("main"),
    )?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOperationsArgs {
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub turn_id: Option<String>,
    pub summary: Option<String>,
    pub operations: Vec<AppOperation>,
    pub silent: Option<bool>,
}

#[tauri::command]
pub fn apply_operations_cmd(
    state: State<'_, AppState>,
    args: ApplyOperationsArgs,
) -> Result<ApplyResult, CommandError> {
    let mut db = state.db.lock();
    let silent = args.silent.unwrap_or(false);
    let mut bus = state.event_bus.lock();
    let result = crate::application_kernel::apply_change(
        &mut db,
        Some(&mut bus),
        crate::application_kernel::ChangeRequest {
            conversation_id: args.conversation_id.clone(),
            project_id: args.project_id.clone(),
            turn_id: args.turn_id.clone(),
            summary: args
                .summary
                .clone()
                .unwrap_or_else(|| "Agent operations".into()),
            operations: args.operations,
            silent,
            source_type: "user".into(),
            provider: None,
            model: None,
            require_approval: false,
            approval_granted: true,
        },
    )
    .map_err(|e| CommandError::new(e.category(), e.user_message()))?;
    result.apply.ok_or_else(|| {
        CommandError::new(
            "approval_required",
            result
                .proposal_id
                .unwrap_or_else(|| "Change requires approval".into()),
        )
    })
}

#[tauri::command]
pub fn validate_agent_response_v2(payload: AgentResponseV2) -> Result<(), CommandError> {
    payload
        .validate()
        .map_err(|e| CommandError::new("invalid", e))
}

#[tauri::command]
pub fn undo_transaction_cmd(
    state: State<'_, AppState>,
    transaction_id: String,
) -> Result<AppTransactionRecord, CommandError> {
    let mut db = state.db.lock();
    Ok(undo_transaction(&mut db, &transaction_id)?)
}

#[tauri::command]
pub fn list_transactions_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    limit: Option<usize>,
) -> Result<Vec<AppTransactionRecord>, CommandError> {
    let db = state.db.lock();
    Ok(list_transactions(
        &db,
        &conversation_id,
        limit.unwrap_or(50),
    )?)
}

#[tauri::command]
pub fn get_transaction_cmd(
    state: State<'_, AppState>,
    transaction_id: String,
) -> Result<AppTransactionRecord, CommandError> {
    let db = state.db.lock();
    Ok(get_transaction(&db, &transaction_id)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchArgs {
    pub source_conversation_id: String,
    pub source_message_id: String,
    pub branch_name: Option<String>,
}

#[tauri::command]
pub fn branch_conversation_cmd(
    state: State<'_, AppState>,
    args: BranchArgs,
) -> Result<(ChatBranchRecord, Vec<SurfaceRecord>), CommandError> {
    let mut db = state.db.lock();
    Ok(branch_from_message(
        &mut db,
        &args.source_conversation_id,
        &args.source_message_id,
        args.branch_name.as_deref().unwrap_or(""),
        "ws-personal-default",
    )?)
}

#[tauri::command]
pub fn list_branches_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ChatBranchRecord>, CommandError> {
    let db = state.db.lock();
    Ok(list_branches(&db, &conversation_id)?)
}

#[tauri::command]
pub fn create_snapshot_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    project_id: Option<String>,
    description: Option<String>,
) -> Result<SnapshotRecord, CommandError> {
    let mut db = state.db.lock();
    Ok(create_snapshot(
        &mut db,
        &conversation_id,
        project_id.as_deref(),
        description.as_deref().unwrap_or(""),
    )?)
}

#[tauri::command]
pub fn get_snapshot_cmd(
    state: State<'_, AppState>,
    snapshot_id: String,
) -> Result<SnapshotRecord, CommandError> {
    let db = state.db.lock();
    Ok(get_snapshot(&db, &snapshot_id)?)
}

#[tauri::command]
pub fn delete_snapshot_cmd(
    state: State<'_, AppState>,
    snapshot_id: String,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    Ok(delete_snapshot(&mut db, &snapshot_id)?)
}

#[tauri::command]
pub fn enqueue_agent_turn_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    prompt: Value,
    priority: Option<i64>,
) -> Result<QueueItem, CommandError> {
    let mut db = state.db.lock();
    Ok(enqueue(
        &mut db,
        &conversation_id,
        &prompt,
        priority.unwrap_or(100),
    )?)
}

#[tauri::command]
pub fn list_agent_queue_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<QueueItem>, CommandError> {
    let db = state.db.lock();
    Ok(list_queue(&db, &conversation_id)?)
}

#[tauri::command]
pub fn cancel_queue_item_cmd(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<QueueItem, CommandError> {
    let mut db = state.db.lock();
    Ok(cancel_queue_item(&mut db, &item_id)?)
}

#[tauri::command]
pub fn remove_queue_item_cmd(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    Ok(remove_queued(&mut db, &item_id)?)
}

#[tauri::command]
pub fn activate_next_queue_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Option<QueueItem>, CommandError> {
    let mut db = state.db.lock();
    Ok(activate_next(&mut db, &conversation_id)?)
}

#[tauri::command]
pub fn complete_queue_item_cmd(
    state: State<'_, AppState>,
    item_id: String,
    error: Option<String>,
) -> Result<QueueItem, CommandError> {
    let mut db = state.db.lock();
    Ok(complete_queue_item(
        &mut db,
        &item_id,
        error.as_deref(),
    )?)
}

#[tauri::command]
pub fn recover_agent_queue_cmd(state: State<'_, AppState>) -> Result<u64, CommandError> {
    let mut db = state.db.lock();
    Ok(recover_stale_active(&mut db)?)
}

#[tauri::command]
pub fn store_diagnostics_cmd(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    turn_id: Option<String>,
    payload: Value,
) -> Result<String, CommandError> {
    let mut db = state.db.lock();
    let key = state.config.lock().api_key.clone();
    Ok(store_diagnostics(
        &mut db,
        conversation_id.as_deref(),
        turn_id.as_deref(),
        &payload,
        key.as_deref(),
    )?)
}

#[tauri::command]
pub fn list_diagnostics_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    limit: Option<usize>,
) -> Result<Vec<Value>, CommandError> {
    let db = state.db.lock();
    Ok(list_diagnostics(
        &db,
        &conversation_id,
        limit.unwrap_or(20),
    )?)
}

#[tauri::command]
pub fn runtime_v2_limits() -> Result<Value, CommandError> {
    Ok(serde_json::json!({
        "maxOperationsPerTurn": runtime_v2::limits::MAX_OPERATIONS_PER_TURN,
        "maxQueuedTurns": runtime_v2::limits::MAX_QUEUED_TURNS,
        "maxComponentsPerSurface": runtime_v2::limits::MAX_COMPONENTS_PER_SURFACE,
        "maxEventDepth": runtime_v2::limits::MAX_EVENT_DEPTH,
        "schemaVersion": runtime_v2::SCHEMA_VERSION_V2,
    }))
}
