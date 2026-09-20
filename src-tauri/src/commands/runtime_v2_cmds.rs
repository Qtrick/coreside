//! Tauri commands for Generative Interface Runtime V2.

use serde::Deserialize;
use serde_json::Value;
use tauri::{ipc::Channel, State, WebviewWindow};

use super::CommandError;
use crate::runtime_v2::packs::CapabilityPackMeta as PackMeta;
use crate::runtime_v2::{
    self, activate_next, append_ledger_entry, branch_from_message, bundled_packs,
    cancel_queue_item, complete_queue_item, create_inline_surface, create_snapshot, delete_draft,
    delete_snapshot, diff_branch, enqueue, ensure_initial_route, flush_scheduler, get_continuity,
    get_conversation_events, get_draft, get_item, get_provider_profile, get_route_state,
    get_snapshot, get_surface, get_surface_state, get_transaction, list_branches, list_diagnostics,
    list_inline_surfaces, list_ledger_entries, list_queue, list_snapshots, list_transactions,
    list_turn_timeline_events, navigate_route, promote_inline_to_tool, recover_stale_active,
    remove_queued, route_back, route_forward, save_continuity, save_draft, save_surface_state,
    schedule_and_apply, schedule_patches, set_route_state, store_diagnostics, surfaces,
    suspend_surface, undo_transaction, update_surface_definition, AgentResponseV2, AppOperation,
    AppTransactionRecord, ApplyResult, ChatBranchRecord, ContextLedgerEntry, ContinuitySnapshot,
    ConversationEventRecord, NavigateResult, PatchPriority, ProviderConformanceRecord, QueueItem,
    RouteState, ScheduleRequest, ScheduledPatch, SnapshotRecord, SurfaceDraft, SurfaceRecord,
    SuspensionState, TurnTimelineEvent,
};
use crate::state::AppState;
use crate::windows;

pub use crate::state::{QueueChangeKind, QueueChangedEvent, SyncScopedEvent};

/// Notify conversation-scoped Channels that Rust/SQLite queue state mutated.
/// Does not use the process-wide event bus.
pub fn emit_queue_changed(
    state: &AppState,
    kind: QueueChangeKind,
    conversation_id: &str,
    item_id: Option<&str>,
) {
    state.emit_queue_changed_to_subscribers(&QueueChangedEvent {
        kind,
        conversation_id: conversation_id.to_string(),
        item_id: item_id.map(|s| s.to_string()),
    });
}

/// Register a Channel for queue mutations on one conversation (main window only).
#[tauri::command]
pub fn subscribe_conversation_queue(
    state: State<'_, AppState>,
    conversation_id: String,
    on_event: Channel<QueueChangedEvent>,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let conversation_id = conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err(CommandError::new(
            "invalid_argument",
            "conversation_id is required",
        ));
    }
    state.subscribe_queue(conversation_id, on_event);
    Ok(())
}

/// Register a Channel for Sync/Conflict on one conversation (main + tool windows).
/// Keep the Channel alive while the window needs surface reload events.
/// Remounts for the same window label replace the prior Channel (no zombies).
#[tauri::command]
pub fn subscribe_conversation_sync(
    state: State<'_, AppState>,
    window: WebviewWindow,
    conversation_id: String,
    on_event: Channel<SyncScopedEvent>,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let conversation_id = conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err(CommandError::new(
            "invalid_argument",
            "conversation_id is required",
        ));
    }
    state.subscribe_sync(conversation_id, window.label().to_string(), on_event);
    Ok(())
}

#[tauri::command]
pub fn list_capability_packs() -> Result<Vec<PackMeta>, CommandError> {
    Ok(bundled_packs())
}

#[tauri::command]
pub fn list_conversation_surfaces(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<SurfaceRecord>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(list_inline_surfaces(&db, &conversation_id)?)
}

#[tauri::command]
pub fn get_surface_cmd(
    window: WebviewWindow,
    state: State<'_, AppState>,
    surface_id: String,
) -> Result<SurfaceRecord, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let surface = get_surface(&db, &surface_id)?;
    windows::enforce_caller_surface_scope(&window, surface.tool_id.as_deref(), &surface.id)?;
    Ok(surface)
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
    state.require_profile()?;
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
    state.require_profile()?;
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
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(promote_inline_to_tool(
        &mut db,
        &surface_id,
        "ws-personal-default",
    )?)
}

#[tauri::command]
pub fn save_surface_state_cmd(
    window: WebviewWindow,
    state: State<'_, AppState>,
    surface_id: String,
    state_json: Value,
    expected_state_revision: Option<i64>,
) -> Result<i64, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    let surface = get_surface(&db, &surface_id)?;
    windows::enforce_caller_surface_scope(&window, surface.tool_id.as_deref(), &surface.id)?;
    let (_merged, new_rev) = surfaces::save_surface_state_user_cas(
        &mut db,
        &surface_id,
        expected_state_revision,
        &state_json,
    )?;
    Ok(new_rev)
}

#[tauri::command]
pub fn get_surface_state_cmd(
    window: WebviewWindow,
    state: State<'_, AppState>,
    surface_id: String,
) -> Result<Value, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let surface = get_surface(&db, &surface_id)?;
    windows::enforce_caller_surface_scope(&window, surface.tool_id.as_deref(), &surface.id)?;
    Ok(get_surface_state(&db, &surface_id)?)
}

/// Returns the current state value AND the current state revision together.
/// Use this instead of getSurface() + getSurfaceState() to avoid a TOCTOU race
/// where the definition revision and state revision might diverge between two calls.
/// The returned `stateRevision` is the correct value to pass to saveSurfaceState().
#[tauri::command]
pub fn get_surface_state_with_revision_cmd(
    window: WebviewWindow,
    state: State<'_, AppState>,
    surface_id: String,
) -> Result<serde_json::Value, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let surface = get_surface(&db, &surface_id)?;
    windows::enforce_caller_surface_scope(&window, surface.tool_id.as_deref(), &surface.id)?;
    let (state_val, state_revision) = surfaces::get_surface_state_with_revision(&db, &surface_id)?;
    Ok(serde_json::json!({
        "state": state_val,
        "stateRevision": state_revision,
    }))
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
    state.require_profile()?;
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
    state.require_profile()?;
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

#[tauri::command]
pub fn delete_draft_cmd(
    state: State<'_, AppState>,
    surface_id: String,
    component_id: String,
    window_id: Option<String>,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(delete_draft(
        &mut db,
        &surface_id,
        &component_id,
        window_id.as_deref().unwrap_or("main"),
    )?)
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
    pub model: Option<String>,
    pub provider: Option<String>,
}

#[tauri::command]
pub fn schedule_patches_cmd(
    state: State<'_, AppState>,
    args: SchedulePatchesArgs,
) -> Result<Vec<ScheduledPatch>, CommandError> {
    state.require_profile()?;
    state
        .quiescence
        .require_active(crate::quiescence::QuiescedSubsystem::PatchScheduler)?;
    let mut db = state.db.lock();
    let priority = PatchPriority::parse(&args.priority)
        .ok_or_else(|| CommandError::new("invalid", "unknown patch priority"))?;

    // P0 security boundary: frontend IPC cannot forge agent authority or bypass approval gates.
    let trusted_source = if args.source_type == "direct_manipulation" {
        "direct_manipulation"
    } else {
        "user"
    };

    let req = ScheduleRequest {
        conversation_id: args.conversation_id,
        turn_id: args.turn_id,
        surface_id: args.surface_id,
        priority,
        operations: args.operations,
        source_type: trusted_source.into(),
        from_agent: false,
        model: None,
        provider: None,
    };
    if args.apply_immediately.unwrap_or(false) {
        let mut bus = state.event_bus.lock();
        let mut bus_opt = Some(&mut *bus);
        let result = schedule_and_apply(
            &mut db,
            &mut bus_opt,
            req,
            true, // User direct manipulation is self-authorized
        )?;
        return Ok(result.scheduled);
    }
    Ok(schedule_patches(&mut db, &req)?)
}

#[tauri::command]
pub fn flush_patch_scheduler_cmd(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    _source_type: Option<String>,
    _approval_granted: Option<bool>,
) -> Result<Vec<crate::application_kernel::ChangeResult>, CommandError> {
    state.require_profile()?;
    state
        .quiescence
        .require_active(crate::quiescence::QuiescedSubsystem::PatchScheduler)?;
    let mut db = state.db.lock();
    let mut bus = state.event_bus.lock();
    let mut bus_opt = Some(&mut *bus);
    // IPC flush is always user-initiated; agent changes in queue still require explicit approval
    Ok(flush_scheduler(
        &mut db,
        &mut bus_opt,
        conversation_id.as_deref(),
        "user",
        true,
    )?)
}

#[tauri::command]
pub fn get_route_state_cmd(
    window: WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
    _window_id: Option<String>,
) -> Result<RouteState, CommandError> {
    state.require_profile()?;
    let effective_window = window.label();
    let db = state.db.lock();
    Ok(get_route_state(
        &db,
        &application_id,
        effective_window,
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
    window: WebviewWindow,
    state: State<'_, AppState>,
    args: SetRouteStateArgs,
) -> Result<RouteState, CommandError> {
    state.require_profile()?;
    let effective_window = window.label();
    let mut db = state.db.lock();
    Ok(set_route_state(
        &mut db,
        &args.application_id,
        effective_window,
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
    window: WebviewWindow,
    state: State<'_, AppState>,
    args: NavigateRouteArgs,
) -> Result<NavigateResult, CommandError> {
    state.require_profile()?;
    let effective_window = window.label();
    let mut db = state.db.lock();
    Ok(navigate_route(
        &mut db,
        &args.application_id,
        effective_window,
        &args.route_id,
        &args.route_params,
        args.push_history.unwrap_or(true),
    )?)
}

#[tauri::command]
pub fn route_back_cmd(
    window: WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
) -> Result<NavigateResult, CommandError> {
    state.require_profile()?;
    let effective_window = window.label();
    let mut db = state.db.lock();
    Ok(route_back(&mut db, &application_id, effective_window)?)
}

#[tauri::command]
pub fn route_forward_cmd(
    window: WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
) -> Result<NavigateResult, CommandError> {
    state.require_profile()?;
    let effective_window = window.label();
    let mut db = state.db.lock();
    Ok(route_forward(&mut db, &application_id, effective_window)?)
}

#[tauri::command]
pub fn ensure_initial_route_cmd(
    window: WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
) -> Result<RouteState, CommandError> {
    state.require_profile()?;
    let effective_window = window.label();
    let mut db = state.db.lock();
    Ok(ensure_initial_route(&mut db, &application_id, effective_window)?)
}

#[tauri::command]
pub fn get_conversation_events_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    after_sequence: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<ConversationEventRecord>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(get_conversation_events(
        &db,
        &conversation_id,
        after_sequence,
        limit,
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
    state.require_profile()?;
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
    state.require_profile()?;
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
    state.require_profile()?;
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
    state.require_profile()?;
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
    state.require_profile()?;
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
    state.require_profile()?;
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
    state.require_profile()?;
    let mut db = state.db.lock();
    let silent = args.silent.unwrap_or(false);
    let mut bus = state.event_bus.lock();
    let result = crate::application_kernel::apply_change(
        &mut db,
        Some(&mut bus),
        crate::application_kernel::ChangeRequest {
            proposal_id: None,
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
    if result.is_committed() {
        return result.apply.ok_or_else(|| {
            CommandError::new("apply_failed", "Committed outcome missing apply payload")
        });
    }
    if result.proposal_id.is_some() {
        return Err(CommandError::new(
            "approval_required",
            result
                .proposal_id
                .unwrap_or_else(|| "Change requires approval".into()),
        ));
    }
    if !result.conflicts.is_empty() {
        return Err(CommandError::new("conflict", result.conflicts.join("; ")));
    }
    Err(CommandError::new(
        "apply_failed",
        format!("Change was not committed ({:?})", result.outcome),
    ))
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
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(undo_transaction(&mut db, &transaction_id)?)
}

#[tauri::command]
pub fn list_transactions_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    limit: Option<usize>,
) -> Result<Vec<AppTransactionRecord>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let capped = limit
        .unwrap_or(50)
        .min(crate::runtime_v2::limits::MAX_REPLAY_OPS_LOADED);
    Ok(list_transactions(&db, &conversation_id, capped)?)
}

#[tauri::command]
pub fn get_transaction_cmd(
    state: State<'_, AppState>,
    transaction_id: String,
) -> Result<AppTransactionRecord, CommandError> {
    state.require_profile()?;
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
    state.require_profile()?;
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
    state.require_profile()?;
    let db = state.db.lock();
    Ok(list_branches(&db, &conversation_id)?)
}

#[tauri::command]
pub fn diff_branch_cmd(
    state: State<'_, AppState>,
    branch_id: String,
) -> Result<crate::runtime_v2::BranchDiffRecord, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(diff_branch(&db, &branch_id)?)
}

#[tauri::command]
pub fn create_snapshot_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    project_id: Option<String>,
    description: Option<String>,
) -> Result<SnapshotRecord, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    Ok(create_snapshot(
        &mut db,
        &conversation_id,
        project_id.as_deref(),
        description.as_deref().unwrap_or(""),
    )?)
}

#[tauri::command]
pub fn list_snapshots_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<SnapshotRecord>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(list_snapshots(&db, &conversation_id)?)
}

#[tauri::command]
pub fn get_snapshot_cmd(
    state: State<'_, AppState>,
    snapshot_id: String,
) -> Result<SnapshotRecord, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(get_snapshot(&db, &snapshot_id)?)
}

#[tauri::command]
pub fn delete_snapshot_cmd(
    state: State<'_, AppState>,
    snapshot_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
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
    state.require_profile()?;
    let mut db = state.db.lock();
    let item = enqueue(&mut db, &conversation_id, &prompt, priority.unwrap_or(100))?;
    drop(db);
    emit_queue_changed(
        state.inner(),
        QueueChangeKind::ItemAdded,
        &conversation_id,
        Some(&item.id),
    );
    Ok(item)
}

#[tauri::command]
pub fn list_agent_queue_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<QueueItem>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(list_queue(&db, &conversation_id)?)
}

#[tauri::command]
pub fn cancel_queue_item_cmd(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<QueueItem, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    let item = get_item(&db, &item_id).map_err(CommandError::from)?;
    let conversation_id = item.conversation_id.clone();
    let attachment_ids: Vec<String> = item
        .prompt
        .get("attachmentIds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    // Cancel the queue row first (queued-only). Only then release staged IDs so a
    // concurrent drain cannot activate the item after attachments were deleted.
    let cancelled = cancel_queue_item(&mut db, &item_id)?;
    drop(db);
    crate::commands::attachment_cmds::release_staged_attachment_ids_best_effort(
        state.inner(),
        &attachment_ids,
    );
    emit_queue_changed(
        state.inner(),
        QueueChangeKind::ItemCancelled,
        &conversation_id,
        Some(&cancelled.id),
    );
    Ok(cancelled)
}

#[tauri::command]
pub fn remove_queue_item_cmd(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    let item = get_item(&db, &item_id).map_err(CommandError::from)?;
    let conversation_id = item.conversation_id.clone();
    let attachment_ids: Vec<String> = item
        .prompt
        .get("attachmentIds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    // Remove queued row first, then release staged IDs (same ordering as cancel).
    remove_queued(&mut db, &item_id)?;
    drop(db);
    crate::commands::attachment_cmds::release_staged_attachment_ids_best_effort(
        state.inner(),
        &attachment_ids,
    );
    emit_queue_changed(
        state.inner(),
        QueueChangeKind::ItemCancelled,
        &conversation_id,
        Some(&item_id),
    );
    Ok(())
}

#[tauri::command]
pub fn activate_next_queue_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Option<QueueItem>, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    let item = activate_next(&mut db, &conversation_id)?;
    drop(db);
    if let Some(ref activated) = item {
        emit_queue_changed(
            state.inner(),
            QueueChangeKind::ItemActivated,
            &conversation_id,
            Some(&activated.id),
        );
    }
    Ok(item)
}

#[tauri::command]
pub fn complete_queue_item_cmd(
    state: State<'_, AppState>,
    item_id: String,
    error: Option<String>,
) -> Result<QueueItem, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    let completed = complete_queue_item(&mut db, &item_id, error.as_deref())?;
    drop(db);
    emit_queue_changed(
        state.inner(),
        QueueChangeKind::ItemCompleted,
        &completed.conversation_id,
        Some(&completed.id),
    );
    Ok(completed)
}

#[tauri::command]
pub fn recover_agent_queue_cmd(state: State<'_, AppState>) -> Result<u64, CommandError> {
    state.require_profile()?;
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
    state.require_profile()?;
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
    state.require_profile()?;
    let db = state.db.lock();
    Ok(list_diagnostics(
        &db,
        &conversation_id,
        limit.unwrap_or(20),
    )?)
}

/// List redacted turn timeline events for read-only replay (never re-applies).
#[tauri::command]
pub fn list_turn_timeline_cmd(
    state: State<'_, AppState>,
    conversation_id: String,
    turn_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<TurnTimelineEvent>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    Ok(list_turn_timeline_events(
        &db,
        &conversation_id,
        turn_id.as_deref(),
        limit.unwrap_or(200),
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

#[cfg(test)]
mod queue_event_tests {
    use super::{emit_queue_changed, QueueChangeKind, QueueChangedEvent};
    use crate::db::Database;
    use crate::state::AppState;
    use serde::Deserialize;
    use std::sync::Arc;
    use tauri::ipc::{Channel, InvokeResponseBody};

    #[test]
    fn queue_changed_event_serializes_camel_case_kinds() {
        let event = QueueChangedEvent {
            kind: QueueChangeKind::ItemAdded,
            conversation_id: "conv-1".into(),
            item_id: Some("q-1".into()),
        };
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["kind"], "itemAdded");
        assert_eq!(value["conversationId"], "conv-1");
        assert_eq!(value["itemId"], "q-1");

        for (kind, expected) in [
            (QueueChangeKind::ItemActivated, "itemActivated"),
            (QueueChangeKind::ItemCancelled, "itemCancelled"),
            (QueueChangeKind::ItemCompleted, "itemCompleted"),
            (QueueChangeKind::QueueSnapshot, "queueSnapshot"),
        ] {
            let v = serde_json::to_value(&QueueChangedEvent {
                kind,
                conversation_id: "c".into(),
                item_id: None,
            })
            .unwrap();
            assert_eq!(v["kind"], expected);
        }
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QueueEventWire {
        conversation_id: String,
    }

    fn test_channel(sink: Arc<parking_lot::Mutex<Vec<String>>>) -> Channel<QueueChangedEvent> {
        Channel::new(move |body| {
            let InvokeResponseBody::Json(json) = body else {
                return Ok(());
            };
            if let Ok(parsed) = serde_json::from_str::<QueueEventWire>(&json) {
                sink.lock().push(parsed.conversation_id);
            }
            Ok(())
        })
    }

    #[test]
    fn emit_queue_changed_only_notifies_matching_conversation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::open_path(&dir.path().join("queue-scope.db")).expect("db");
        let state = AppState::new_for_test(db);
        let hit_a = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let hit_b = Arc::new(parking_lot::Mutex::new(Vec::new()));
        state.subscribe_queue("conv-a".into(), test_channel(hit_a.clone()));
        state.subscribe_queue("conv-b".into(), test_channel(hit_b.clone()));

        emit_queue_changed(&state, QueueChangeKind::ItemAdded, "conv-a", Some("q-1"));

        assert_eq!(hit_a.lock().as_slice(), &["conv-a".to_string()]);
        assert!(
            hit_b.lock().is_empty(),
            "wrong conversation must get nothing"
        );
    }

    #[test]
    fn subscribe_queue_replaces_prior_channel_for_conversation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::open_path(&dir.path().join("queue-replace.db")).expect("db");
        let state = AppState::new_for_test(db);
        let first = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let second = Arc::new(parking_lot::Mutex::new(Vec::new()));
        state.subscribe_queue("conv-a".into(), test_channel(first.clone()));
        state.subscribe_queue("conv-a".into(), test_channel(second.clone()));

        emit_queue_changed(&state, QueueChangeKind::ItemAdded, "conv-a", Some("q-1"));

        assert!(
            first.lock().is_empty(),
            "replaced Channel must not receive events"
        );
        assert_eq!(second.lock().as_slice(), &["conv-a".to_string()]);
    }
}

#[cfg(test)]
mod sync_event_tests {
    use super::SyncScopedEvent;
    use crate::db::Database;
    use crate::state::AppState;
    use serde::Deserialize;
    use std::sync::Arc;
    use tauri::ipc::{Channel, InvokeResponseBody};

    #[derive(Debug, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    enum SyncWire {
        #[serde(rename_all = "camelCase")]
        Sync { conversation_id: Option<String> },
        #[serde(rename_all = "camelCase")]
        Conflict { conversation_id: Option<String> },
    }

    fn sync_test_channel(sink: Arc<parking_lot::Mutex<Vec<String>>>) -> Channel<SyncScopedEvent> {
        Channel::new(move |body| {
            let InvokeResponseBody::Json(json) = body else {
                return Ok(());
            };
            if let Ok(parsed) = serde_json::from_str::<SyncWire>(&json) {
                let cid = match parsed {
                    SyncWire::Sync {
                        conversation_id, ..
                    }
                    | SyncWire::Conflict {
                        conversation_id, ..
                    } => conversation_id.unwrap_or_default(),
                };
                sink.lock().push(cid);
            }
            Ok(())
        })
    }

    #[test]
    fn emit_sync_only_notifies_matching_conversation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::open_path(&dir.path().join("sync-scope.db")).expect("db");
        let state = AppState::new_for_test(db);
        let hit_a = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let hit_b = Arc::new(parking_lot::Mutex::new(Vec::new()));
        state.subscribe_sync("conv-a".into(), "main", sync_test_channel(hit_a.clone()));
        state.subscribe_sync("conv-b".into(), "main", sync_test_channel(hit_b.clone()));

        let n = state.emit_sync_to_subscribers(
            "conv-a",
            &SyncScopedEvent::Sync {
                conversation_id: Some("conv-a".into()),
                surface_ids: vec!["surf-1".into()],
                tool_ids: vec![],
                application_id: None,
                revision: Some(1),
                sync_kind: "transaction_applied".into(),
            },
        );

        assert_eq!(n, 1);
        assert_eq!(hit_a.lock().as_slice(), &["conv-a".to_string()]);
        assert!(
            hit_b.lock().is_empty(),
            "wrong conversation must get nothing"
        );
    }

    #[test]
    fn subscribe_sync_replaces_same_window_and_fans_out_distinct_windows() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::open_path(&dir.path().join("sync-multi.db")).expect("db");
        let state = AppState::new_for_test(db);
        let zombie = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let main = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let tool = Arc::new(parking_lot::Mutex::new(Vec::new()));
        // Remount leaves a zombie if we only append — replace per window label.
        state.subscribe_sync("conv-a".into(), "main", sync_test_channel(zombie.clone()));
        state.subscribe_sync("conv-a".into(), "main", sync_test_channel(main.clone()));
        state.subscribe_sync(
            "conv-a".into(),
            "tool-sample",
            sync_test_channel(tool.clone()),
        );

        let n = state.emit_sync_to_subscribers(
            "conv-a",
            &SyncScopedEvent::Conflict {
                conversation_id: Some("conv-a".into()),
                message: "conflict".into(),
                conflicts: vec!["rev".into()],
            },
        );

        assert_eq!(n, 2, "main + tool window both receive");
        assert!(
            zombie.lock().is_empty(),
            "replaced main Channel must not absorb Sync"
        );
        assert_eq!(main.lock().as_slice(), &["conv-a".to_string()]);
        assert_eq!(tool.lock().as_slice(), &["conv-a".to_string()]);
        assert_eq!(state.sync_subscriber_count("conv-a"), 2);
    }
}
