//! Tauri commands for the Application Kernel.

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::application_kernel::compiler::{compile, ChangeIntent, CompiledChange};
use crate::application_kernel::context::{
    application_summary, data_model_summary, recent_transactions_summary,
};
use crate::application_kernel::data::{
    create_record, query_records, upsert_model, DataModelDefinition,
};
use crate::application_kernel::lifecycle::{
    create_job, garbage_collect, get_job, interrupt_active_jobs, JobRecord,
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
use crate::application_kernel::testing::{run_test, upsert_test, visual_checks_summary, DeclarativeTest};
use crate::application_kernel::{
    apply_change, capability_catalog, ChangeRequest, ChangeResult,
};
use crate::commands::CommandError;
use crate::state::AppState;

fn map_kernel(err: crate::application_kernel::errors::KernelError) -> CommandError {
    CommandError::new(err.category(), err.user_message())
}

#[tauri::command]
pub fn kernel_capability_catalog() -> Value {
    capability_catalog()
}

#[tauri::command]
pub fn kernel_apply_change(
    state: State<'_, AppState>,
    mut request: ChangeRequest,
) -> Result<ChangeResult, CommandError> {
    // IPC from the UI is always user-initiated; never allow agent self-approval.
    request.source_type = "user".into();
    if request.operations.is_empty() {
        return Err(CommandError::new("invalid", "operations required"));
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
    let db = state.db.lock();
    list_manifests(&db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_get_manifest(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<ManifestRecord, CommandError> {
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
    let mut db = state.db.lock();
    ensure_manifest_for_tool(&mut db, &tool_id, &tool_name, &surface_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_restore_last_known_good(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<ManifestRecord, CommandError> {
    let mut db = state.db.lock();
    restore_last_known_good(&mut db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_mark_last_known_good(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<(), CommandError> {
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
    let mut db = state.db.lock();
    crate::application_kernel::permissions::assert_can_write_data(&db, &application_id)
        .map_err(CommandError::from)?;
    upsert_model(&mut db, &application_id, model).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_create_record(
    state: State<'_, AppState>,
    application_id: String,
    model_id: String,
    data: Value,
) -> Result<String, CommandError> {
    let mut db = state.db.lock();
    create_record(&mut db, &application_id, &model_id, data).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_query_records(
    state: State<'_, AppState>,
    application_id: String,
    model_id: String,
    limit: Option<usize>,
) -> Result<Vec<Value>, CommandError> {
    let db = state.db.lock();
    query_records(&db, &application_id, &model_id, limit.unwrap_or(100)).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_grant_permission(
    state: State<'_, AppState>,
    application_id: String,
    permission: String,
    scope: Option<Value>,
) -> Result<PermissionGrant, CommandError> {
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
    state: State<'_, AppState>,
    application_id: String,
    permission: String,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    revoke_permission(&mut db, &application_id, &permission).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_list_permissions(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<Vec<PermissionGrant>, CommandError> {
    let db = state.db.lock();
    list_permissions(&db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_get_recovery_state(
    state: State<'_, AppState>,
) -> Result<RecoveryState, CommandError> {
    let db = state.db.lock();
    get_recovery_state(&db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_set_recovery_mode(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<RecoveryState, CommandError> {
    let mut db = state.db.lock();
    set_recovery_mode(&mut db, enabled).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_enter_safe_startup(
    state: State<'_, AppState>,
    reason: String,
) -> Result<RecoveryState, CommandError> {
    let mut db = state.db.lock();
    enter_safe_startup(&mut db, &reason).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_clear_recovery(
    state: State<'_, AppState>,
) -> Result<RecoveryState, CommandError> {
    let mut db = state.db.lock();
    clear_recovery(&mut db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_set_recovery_flags(
    state: State<'_, AppState>,
    disable_user_surfaces: Option<bool>,
    disable_custom_layouts: Option<bool>,
    disable_capability_packs: Option<bool>,
) -> Result<RecoveryState, CommandError> {
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
    state: State<'_, AppState>,
    application_id: String,
) -> Result<AppPackage, CommandError> {
    let db = state.db.lock();
    export_package(&db, &application_id).map_err(map_kernel)
}

#[tauri::command]
pub fn kernel_export_package_bytes(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<Vec<u8>, CommandError> {
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
    state: State<'_, AppState>,
    bytes: Vec<u8>,
    approve: bool,
    remint_ids: Option<bool>,
) -> Result<crate::application_kernel::manifest::ApplicationManifest, CommandError> {
    let mut db = state.db.lock();
    import_package(&mut db, &bytes, approve, remint_ids.unwrap_or(true)).map_err(map_kernel)
}

#[tauri::command]
pub fn kernel_application_summary(
    state: State<'_, AppState>,
    application_id: String,
) -> Result<Value, CommandError> {
    let db = state.db.lock();
    application_summary(&db, &application_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_data_model_summary(
    state: State<'_, AppState>,
    application_id: String,
    model_id: String,
) -> Result<Value, CommandError> {
    let db = state.db.lock();
    data_model_summary(&db, &application_id, &model_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_recent_transactions(
    state: State<'_, AppState>,
    application_id: String,
    limit: Option<usize>,
) -> Result<Value, CommandError> {
    let db = state.db.lock();
    recent_transactions_summary(&db, &application_id, limit.unwrap_or(10)).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_upsert_test(
    state: State<'_, AppState>,
    application_id: String,
    test: DeclarativeTest,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    upsert_test(&mut db, &application_id, test).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_run_test(
    state: State<'_, AppState>,
    application_id: String,
    test_id: String,
) -> Result<Value, CommandError> {
    let mut db = state.db.lock();
    run_test(&mut db, &application_id, &test_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_visual_checks(width: u32) -> Value {
    visual_checks_summary(width)
}

#[tauri::command]
pub fn kernel_garbage_collect(state: State<'_, AppState>) -> Result<u64, CommandError> {
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
pub fn kernel_get_job(
    state: State<'_, AppState>,
    id: String,
) -> Result<JobRecord, CommandError> {
    let db = state.db.lock();
    get_job(&db, &id).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_interrupt_jobs(state: State<'_, AppState>) -> Result<u64, CommandError> {
    let mut db = state.db.lock();
    interrupt_active_jobs(&mut db).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_set_policy_override(
    state: State<'_, AppState>,
    key: String,
    decision: String,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    set_policy_override(&mut db, &key, &decision).map_err(CommandError::from)
}

#[tauri::command]
pub fn kernel_clear_policy_override(
    state: State<'_, AppState>,
    key: String,
) -> Result<(), CommandError> {
    let mut db = state.db.lock();
    clear_policy_override(&mut db, &key).map_err(CommandError::from)
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
