//! Tauri commands for the Application Kernel.

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::application_kernel::compiler::{compile, ChangeIntent, CompiledChange};
use crate::application_kernel::context::{
    application_summary, data_model_summary, recent_transactions_summary,
};
use crate::application_kernel::data::{upsert_model, DataModelDefinition};
use crate::application_kernel::lifecycle::{
    clear_build_failures, create_job, garbage_collect, get_job, interrupt_active_jobs,
    list_build_failures, list_versions, record_build_failure, set_application_enabled,
    ApplicationVersion, BuildFailure, JobRecord,
};
use crate::application_kernel::manifest::{
    ensure_manifest_for_tool, get_manifest, list_manifests, mark_last_known_good,
    restore_last_known_good, ManifestRecord,
};
use crate::application_kernel::packages::{
    export_package, import_package, package_to_bytes, preview_package, validate_package_bytes,
    AppPackage, PackagePreview,
};
use crate::application_kernel::permissions::{
    grant_permission, list_permissions, revoke_permission, PermissionGrant,
};
use crate::application_kernel::policy::{clear_policy_override, set_policy_override};
use crate::application_kernel::recovery::{
    clear_recovery, enter_safe_startup, get_recovery_state, set_flags, set_recovery_mode,
    RecoveryState,
};
use crate::application_kernel::registered_actions::approvals::{
    self, ApprovalDecisionResult, ApprovalRequest, RememberChoice,
};
use crate::application_kernel::registered_actions::audit::{self, AuditEvent};
use crate::application_kernel::registered_actions::grants::{self, GrantDuration, GrantScope};
use crate::application_kernel::registered_actions::{
    self, execute_registered_action, ActionOutcome, ActionRunContext, ClientActionRequest,
    RuntimeGrant, Venue,
};
use crate::application_kernel::testing::{
    run_test, upsert_test, visual_checks_summary, DeclarativeTest,
};
use crate::application_kernel::{apply_change, capability_catalog, ChangeRequest, ChangeResult};
use crate::commands::CommandError;
use crate::state::AppState;
use crate::windows;

fn map_kernel(err: crate::application_kernel::errors::KernelError) -> CommandError {
    CommandError::new(err.category(), err.user_message())
}

/// Defense-in-depth: sensitive kernel mutations require the main window label.
/// Tool windows are already capability-denied; this also blocks non-main labels.
/// Not a substitute for CSP / markdown hardening — main-webview XSS remains residual risk.
fn require_main_for_sensitive_kernel(window: &tauri::WebviewWindow) -> Result<(), CommandError> {
    if windows::caller_bound_tool_id(window).is_some() || window.label() != "main" {
        return Err(CommandError::new(
            "forbidden",
            "Only the main Coreside window can perform this kernel operation.",
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn kernel_capability_catalog() -> Value {
    capability_catalog()
}

#[tauri::command]
pub fn kernel_apply_change(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    mut request: ChangeRequest,
) -> Result<ChangeResult, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    // IPC from the UI is always user-initiated; never allow agent self-approval.
    request.source_type = "user".into();
    if request.proposal_id.is_none() && request.operations.is_empty() {
        return Err(CommandError::new("invalid", "operations or proposalId required"));
    }
    let mut db = state.db.lock();
    let mut bus = state.event_bus.lock();
    let result = apply_change(&mut db, Some(&mut bus), request).map_err(map_kernel)?;
    Ok(result)
}

#[tauri::command]
pub fn kernel_compile_intent(intent: ChangeIntent) -> Result<CompiledChange, CommandError> {
    compile(intent).map_err(map_kernel)
}

#[tauri::command]
pub fn kernel_list_manifests(
    state: State<'_, AppState>,
) -> Result<Vec<ManifestRecord>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    list_manifests(&db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_get_manifest(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<ManifestRecord, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    get_manifest(&db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_ensure_tool_manifest(
    state: State<'_, AppState>,
    tool_id: String,
    tool_name: String,
    surface_id: String,
) -> Result<ManifestRecord, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    ensure_manifest_for_tool(&mut db, &tool_id, &tool_name, &surface_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_restore_last_known_good(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
) -> Result<ManifestRecord, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    restore_last_known_good(&mut db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_mark_last_known_good(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    // Trusted UI / verification path only — not agent-callable as self-grant via ops
    mark_last_known_good(&mut db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_upsert_data_model(
    state: State<'_, AppState>,
    application_id: String,
    model: DataModelDefinition,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    crate::application_kernel::permissions::assert_can_write_data(&db, &application_id)
        .map_err(CommandError::from)?;
    upsert_model(&mut db, &application_id, model).map_err(CommandError::from)
}

// Generated-application record reads and writes have exactly one public IPC
// path: `kernel_invoke_registered_action` (`local_data.query` / `local_data.write`
// / `local_data.delete`). The former `kernel_create_record` and
// `kernel_query_records` commands were retired because they skipped the
// gateway's audit ledger, circuit breakers, and output bounds.

#[tauri::command]
pub fn kernel_grant_permission(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
    permission: String,
    scope: Option<Value>,
) -> Result<PermissionGrant, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    grant_permission(
        &mut db,
        &application_id,
        &permission,
        scope.unwrap_or_else(|| serde_json::json!({ "scope": "application" })),
        "user",
    )
    .map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_revoke_permission(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
    permission: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    revoke_permission(&mut db, &application_id, &permission).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_list_permissions(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<Vec<PermissionGrant>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    list_permissions(&db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_get_recovery_state(
    state: State<'_, AppState>,
) -> Result<RecoveryState, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    get_recovery_state(&db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_set_recovery_mode(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<RecoveryState, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    set_recovery_mode(&mut db, enabled).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_enter_safe_startup(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    reason: String,
) -> Result<RecoveryState, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    enter_safe_startup(&mut db, &reason).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_clear_recovery(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<RecoveryState, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    clear_recovery(&mut db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_set_recovery_flags(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    disable_user_surfaces: Option<bool>,
    disable_custom_layouts: Option<bool>,
    disable_capability_packs: Option<bool>,
) -> Result<RecoveryState, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    set_flags(
        &mut db,
        disable_user_surfaces,
        disable_custom_layouts,
        disable_capability_packs,
    )
    .map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_export_package(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
) -> Result<AppPackage, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let db = state.db.lock();
    export_package(&db, &application_id).map_err(map_kernel)
}

#[tauri::command]
pub fn kernel_export_package_bytes(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
) -> Result<Vec<u8>, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let db = state.db.lock();
    let pkg = export_package(&db, &application_id).map_err(map_kernel)?;
    package_to_bytes(&pkg).map_err(map_kernel)
}

#[tauri::command]
pub fn kernel_preview_package(bytes: Vec<u8>) -> Result<PackagePreview, CommandError> {
    let pkg = validate_package_bytes(&bytes).map_err(map_kernel)?;
    Ok(preview_package(&pkg))
}

#[tauri::command]
pub fn kernel_import_package(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    bytes: Vec<u8>,
    approve: bool,
    remint_ids: Option<bool>,
) -> Result<crate::application_kernel::manifest::ApplicationManifest, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    import_package(&mut db, &bytes, approve, remint_ids.unwrap_or(true)).map_err(map_kernel)
}

#[tauri::command]
pub fn kernel_application_summary(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<Value, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    application_summary(&db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_data_model_summary(
    state: State<'_, AppState>,
    application_id: String,
    model_id: String,
) -> Result<Value, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    data_model_summary(&db, &application_id, &model_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_recent_transactions(
    state: State<'_, AppState>,
    application_id: String,
    limit: Option<usize>,
) -> Result<Value, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    recent_transactions_summary(&db, &application_id, limit.unwrap_or(10))
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_upsert_test(
    state: State<'_, AppState>,
    application_id: String,
    test: DeclarativeTest,
) -> Result<(), CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    upsert_test(&mut db, &application_id, test).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_run_test(
    state: State<'_, AppState>,
    application_id: String,
    test_id: String,
) -> Result<Value, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    run_test(&mut db, &application_id, &test_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_visual_checks(width: u32) -> Value {
    visual_checks_summary(width)
}

#[tauri::command]
pub fn kernel_garbage_collect(state: State<'_, AppState>) -> Result<u64, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    garbage_collect(&mut db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_create_job(
    state: State<'_, AppState>,
    application_id: Option<String>,
    turn_id: Option<String>,
    job_type: String,
) -> Result<JobRecord, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    create_job(
        &mut db,
        application_id.as_deref(),
        turn_id.as_deref(),
        &job_type,
    )
    .map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_get_job(state: State<'_, AppState>, id: String) -> Result<JobRecord, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    get_job(&db, &id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_interrupt_jobs(state: State<'_, AppState>) -> Result<u64, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    interrupt_active_jobs(&mut db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_set_policy_override(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    key: String,
    decision: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    set_policy_override(&mut db, &key, &decision).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_clear_policy_override(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    key: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    clear_policy_override(&mut db, &key).map_err(CommandError::from)
}

// ---------------------------------------------------------------------------
// Registered action runtime
// ---------------------------------------------------------------------------

/// Approval and grant state is shared by every window. Emitting on change lets
/// each window refresh on demand instead of polling on a timer.
pub const APPROVALS_CHANGED_EVENT: &str = "runtime-approvals-changed";

fn notify_approvals_changed(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let _ = app.emit(APPROVALS_CHANGED_EVENT, ());
}

#[tauri::command]
pub fn kernel_list_registered_actions() -> Value {
    registered_actions::catalog_json()
}

/// Invoke a registered action. The trusted context is built here: the client
/// cannot choose the actor, the venue's authority, the presence, or the session.
/// `presence` is always `present` over IPC — away runs only exist inside the
/// automation executor.
///
/// Tool windows may only invoke actions for their bound tool (`application_id`).
#[tauri::command]
pub fn kernel_invoke_registered_action(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mut request: ClientActionRequest,
) -> Result<ActionOutcome, CommandError> {
    state.require_profile()?;
    if let Some(bound) = crate::windows::caller_bound_tool_id(&window) {
        if bound.is_empty() {
            return Err(CommandError::new(
                "forbidden",
                "This tool window cannot invoke registered actions.",
            ));
        }
        // Normalize blank client ids to "missing" before binding.
        if request
            .application_id
            .as_deref()
            .is_some_and(|id| id.trim().is_empty())
        {
            request.application_id = None;
        }
        match request.application_id.as_deref() {
            Some(id) if id == bound => {}
            Some(_) => {
                return Err(CommandError::new(
                    "forbidden",
                    "This tool window cannot invoke actions for another application.",
                ));
            }
            None => {
                request.application_id = Some(bound.clone());
            }
        }
        if let Some(ref surface_id) = request.surface_id {
            let db = state.db.lock();
            let surface =
                crate::runtime_v2::get_surface(&db, surface_id).map_err(CommandError::from)?;
            crate::windows::enforce_caller_surface_scope(
                &window,
                surface.tool_id.as_deref(),
                &surface.id,
            )?;
        }
    }
    let venue = if request.application_id.is_some() {
        Venue::Application
    } else {
        Venue::Chat
    };
    let ctx = ActionRunContext::from_client(&request, venue);
    let outcome = {
        let mut db = state.db.lock();
        execute_registered_action(
            &mut db,
            &ctx,
            &request.action_name,
            &request.input,
            request.approval_id.as_deref(),
        )
    };
    if matches!(outcome, ActionOutcome::PendingApproval { .. }) {
        notify_approvals_changed(&app);
    }
    Ok(outcome)
}

#[tauri::command]
pub fn kernel_list_pending_approvals(
    state: State<'_, AppState>,
) -> Result<Vec<ApprovalRequest>, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    approvals::list_pending(&mut db).map_err(CommandError::from)
}

/// Approve or deny. Only reachable from the user interface, so the actor is
/// always `user`; the agent has no command that decides approvals.
#[tauri::command]
pub fn kernel_decide_approval(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    approval_id: String,
    approve: bool,
    remember_scope: Option<String>,
    remember_duration: Option<String>,
) -> Result<ApprovalDecisionResult, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let remember = match (remember_scope, remember_duration) {
        (Some(scope), Some(duration)) => {
            let scope = GrantScope::parse(&scope)
                .ok_or_else(|| CommandError::new("invalid", "unknown grant scope"))?;
            let duration = GrantDuration::parse(&duration)
                .ok_or_else(|| CommandError::new("invalid", "unknown grant duration"))?;
            Some(RememberChoice { scope, duration })
        }
        _ => None,
    };
    let mut db = state.db.lock();
    let mut result = approvals::decide(&mut db, &approval_id, approve, remember, "user")
        .map_err(CommandError::from)?;
    // Exactly-once execution: after a user approval, re-run the frozen call through
    // the gateway so the action is not left hanging until a second click.
    if approve {
        let input = approvals::frozen_input(&db, &approval_id).map_err(CommandError::from)?;
        let venue = if result.approval.application_id.is_some() {
            Venue::Application
        } else {
            Venue::Chat
        };
        let req = ClientActionRequest {
            action_name: result.approval.action_name.clone(),
            input,
            application_id: result.approval.application_id.clone(),
            surface_id: result.approval.surface_id.clone(),
            component_id: result.approval.component_id.clone(),
            conversation_id: None,
            project_id: None,
            approval_id: Some(approval_id),
        };
        let ctx = ActionRunContext::from_client(&req, venue);
        result.outcome = Some(execute_registered_action(
            &mut db,
            &ctx,
            &req.action_name,
            &req.input,
            req.approval_id.as_deref(),
        ));
        if result.approval.presence == "away" {
            if let Some(app_id) = result.approval.application_id.as_deref() {
                let _ = crate::db::clear_automation_waiting_for_application(&mut db, app_id);
            }
        }
    }
    drop(db);
    notify_approvals_changed(&app);
    Ok(result)
}

#[tauri::command]
pub fn kernel_list_runtime_grants(
    state: State<'_, AppState>,
    application_id: Option<String>,
) -> Result<Vec<RuntimeGrant>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    grants::list_grants(&db, application_id.as_deref()).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_revoke_runtime_grant(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    grant_id: String,
) -> Result<(), CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    {
        let mut db = state.db.lock();
        grants::revoke_grant(&mut db, &grant_id).map_err(CommandError::from)?;
    }
    // A revoked grant stops authorizing calls in every open window immediately.
    notify_approvals_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn kernel_list_audit_events(
    state: State<'_, AppState>,
    application_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<AuditEvent>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    audit::list_events(&db, application_id.as_deref(), limit.unwrap_or(100))
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_clear_audit_events(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<u64, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    audit::clear_events(&mut db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_set_application_lifecycle(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    application_id: String,
    enabled: bool,
) -> Result<ManifestRecord, CommandError> {
    state.require_profile()?;
    require_main_for_sensitive_kernel(&window)?;
    let mut db = state.db.lock();
    set_application_enabled(&mut db, &application_id, enabled).map_err(CommandError::from)?;
    get_manifest(&db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_record_build_failure(
    state: State<'_, AppState>,
    application_id: String,
    message: String,
    retryable: Option<bool>,
    request_ref: Option<String>,
) -> Result<BuildFailure, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    record_build_failure(
        &mut db,
        &application_id,
        &message,
        retryable.unwrap_or(true),
        request_ref.as_deref(),
    )
    .map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_clear_build_failure(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<u64, CommandError> {
    state.require_profile()?;
    let mut db = state.db.lock();
    clear_build_failures(&mut db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_list_build_failures(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<Vec<BuildFailure>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    list_build_failures(&db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_list_application_versions(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<Vec<ApplicationVersion>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    list_versions(&db, &application_id).map_err(CommandError::from)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedSearchHit {
    pub resource_type: String,
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub updated_at: Option<String>,
}

#[tauri::command]
pub fn kernel_unified_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<UnifiedSearchHit>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let q = format!("%{}%", query.trim());
    let lim = limit.unwrap_or(20) as i64;
    let mut hits = Vec::new();

    // Applications
    if let Ok(mut stmt) = db.conn().prepare(
        "SELECT application_id, json_extract(manifest_json, '$.name'), project_id, updated_at
         FROM application_manifests
         WHERE application_id LIKE ?1 OR manifest_json LIKE ?1
         ORDER BY updated_at DESC LIMIT ?2",
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![q, lim], |row| {
            Ok(UnifiedSearchHit {
                resource_type: "application".into(),
                id: row.get(0)?,
                title: row
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| "Application".into()),
                project_id: row.get(2)?,
                updated_at: row.get(3)?,
            })
        }) {
            hits.extend(rows.flatten());
        }
    }

    // Tools
    if let Ok(mut stmt) = db.conn().prepare(
        "SELECT id, name, updated_at FROM tools
         WHERE name LIKE ?1 OR id LIKE ?1 ORDER BY updated_at DESC LIMIT ?2",
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![q, lim], |row| {
            Ok(UnifiedSearchHit {
                resource_type: "tool".into(),
                id: row.get(0)?,
                title: row.get(1)?,
                project_id: None,
                updated_at: row.get(2)?,
            })
        }) {
            hits.extend(rows.flatten());
        }
    }

    // Conversations
    if let Ok(mut stmt) = db.conn().prepare(
        "SELECT id, COALESCE(title, 'Chat'), project_id, updated_at FROM conversations
         WHERE title LIKE ?1 OR id LIKE ?1 ORDER BY updated_at DESC LIMIT ?2",
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![q, lim], |row| {
            Ok(UnifiedSearchHit {
                resource_type: "chat".into(),
                id: row.get(0)?,
                title: row.get(1)?,
                project_id: row.get(2)?,
                updated_at: row.get(3)?,
            })
        }) {
            hits.extend(rows.flatten());
        }
    }

    hits.truncate(lim as usize);
    Ok(hits)
}
