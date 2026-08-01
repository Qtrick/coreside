//! Coreside Tauri library entrypoint.

mod ai;
mod application_kernel;
mod automations;
mod branding;
mod commands;
mod config;
mod crawler;
mod credentials;
mod db;
mod exa;
mod exports;
mod media;
mod projects;
mod research;
mod runtime_v2;
mod search;
mod security;
mod settings;
mod state;
mod wallpapers;
mod windows;

use std::sync::Arc;

use automations::SchedulerHandle;
use state::AppState;
use tauri::Manager;
use tracing_subscriber::{fmt, EnvFilter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = config::load_config();
    init_tracing(&config.log_level);

    tracing::info!(
        provider = %config.provider,
        model = %config.model,
        has_key = config.has_api_key(),
        "Coreside starting"
    );

    let database = db::Database::open_default().unwrap_or_else(|e| {
        tracing::error!(error = %e, "Failed to open database");
        panic!("Failed to open database: {e}");
    });

    {
        let _ = db::ensure_default_workspace(&database);
    }

    let app_state = AppState::new(config, database);
    let scheduler_handle = Arc::new(SchedulerHandle::default());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .manage(scheduler_handle.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_ai_status,
            commands::get_model_catalog,
            commands::test_ai_connection,
            commands::list_provider_connections,
            commands::upsert_provider_connection,
            commands::delete_provider_connection,
            commands::set_active_provider_connection,
            commands::test_provider_connection,
            commands::provider_key_hints,
            commands::store_hosted_auth_session,
            commands::clear_hosted_auth_session,
            commands::get_hosted_auth_status,
            commands::list_conversations,
            commands::create_conversation,
            commands::delete_conversation,
            commands::get_messages,
            commands::delete_messages_from,
            commands::send_message,
            commands::cancel_request,
            commands::stage_chat_attachment,
            commands::get_chat_attachment_src,
            commands::discard_tool_change,
            commands::discard_kernel_proposal,
            commands::set_kernel_proposal_status,
            commands::apply_tool_change,
            commands::list_tools,
            commands::get_tool,
            commands::get_tool_versions,
            commands::undo_tool_change,
            commands::save_tool_state,
            commands::get_tool_state,
            commands::get_settings,
            commands::set_setting,
            commands::list_added_settings,
            commands::upsert_added_setting,
            commands::delete_added_setting,
            commands::validate_change_targets,
            commands::set_dock_icon,
            commands::set_dock_icon_for_os_appearance,
            commands::clear_conversations,
            commands::clear_tools,
            commands::open_tool_window,
            commands::open_external_url,
            commands::list_automations,
            commands::upsert_automation,
            commands::set_automation_enabled,
            commands::delete_automation,
            commands::list_automation_runs,
            commands::run_automation_now,
            commands::list_workspace_backgrounds,
            commands::upsert_workspace_background,
            commands::export_tool,
            commands::list_export_formats,
            commands::list_projects_cmd,
            commands::get_project_cmd,
            commands::create_project_cmd,
            commands::update_project_cmd,
            commands::archive_project_cmd,
            commands::restore_project_cmd,
            commands::delete_project_cmd,
            commands::create_conversation_in_project,
            commands::assign_chats_to_project,
            commands::assign_conversation_to_project_cmd,
            commands::remove_chat_from_project,
            commands::list_project_conversations_cmd,
            commands::list_unassigned_conversations_cmd,
            commands::search_project_context_cmd,
            commands::refresh_project_summary,
            commands::rebuild_project_index_cmd,
            commands::rename_conversation_cmd,
            commands::duplicate_conversation_cmd,
            commands::export_project,
            commands::set_project_wallpaper_cmd,
            commands::touch_project_opened,
            commands::get_search_connection,
            commands::configure_search_connection,
            commands::delete_search_connection,
            commands::test_search_connection,
            commands::get_exa_connection,
            commands::configure_exa_connection,
            commands::delete_exa_connection,
            commands::test_exa_connection,
            commands::get_exa_usage,
            commands::list_exa_usage,
            commands::get_exa_budget,
            commands::set_exa_budget,
            commands::get_search_profile,
            commands::set_search_profile,
            commands::web_search_cmd,
            commands::image_search_cmd,
            commands::video_search_cmd,
            commands::fetch_web_page_cmd,
            commands::list_search_sessions_cmd,
            commands::get_search_session_cmd,
            commands::clear_search_history_cmd,
            commands::get_crawler_status,
            commands::get_crawler_installation,
            commands::cleanup_crawler_cache,
            commands::get_crawler_cache_stats,
            commands::set_web_research_resource_profile,
            commands::list_media_assets_cmd,
            commands::get_media_asset_cmd,
            commands::delete_media_asset_cmd,
            commands::import_media_asset_cmd,
            commands::touch_media_asset_cmd,
            commands::get_media_asset_src_cmd,
            commands::get_media_asset_thumb_src_cmd,
            commands::media_asset_usage_cmd,
            commands::list_capability_packs,
            commands::list_conversation_surfaces,
            commands::get_surface_cmd,
            commands::create_inline_surface_cmd,
            commands::update_surface_cmd,
            commands::promote_surface_cmd,
            commands::save_surface_state_cmd,
            commands::get_surface_state_cmd,
            commands::get_draft_cmd,
            commands::save_draft_cmd,
            commands::schedule_patches_cmd,
            commands::flush_patch_scheduler_cmd,
            commands::get_route_state_cmd,
            commands::set_route_state_cmd,
            commands::navigate_route_cmd,
            commands::append_context_ledger_cmd,
            commands::list_context_ledger_cmd,
            commands::get_provider_profile_cmd,
            commands::get_continuity_cmd,
            commands::save_continuity_cmd,
            commands::suspend_surface_cmd,
            commands::apply_operations_cmd,
            commands::validate_agent_response_v2,
            commands::undo_transaction_cmd,
            commands::list_transactions_cmd,
            commands::get_transaction_cmd,
            commands::branch_conversation_cmd,
            commands::list_branches_cmd,
            commands::create_snapshot_cmd,
            commands::get_snapshot_cmd,
            commands::delete_snapshot_cmd,
            commands::enqueue_agent_turn_cmd,
            commands::list_agent_queue_cmd,
            commands::cancel_queue_item_cmd,
            commands::remove_queue_item_cmd,
            commands::activate_next_queue_cmd,
            commands::complete_queue_item_cmd,
            commands::recover_agent_queue_cmd,
            commands::store_diagnostics_cmd,
            commands::list_diagnostics_cmd,
            commands::runtime_v2_limits,
            commands::kernel_capability_catalog,
            commands::kernel_apply_change,
            commands::kernel_compile_intent,
            commands::kernel_list_manifests,
            commands::kernel_get_manifest,
            commands::kernel_ensure_tool_manifest,
            commands::kernel_restore_last_known_good,
            commands::kernel_mark_last_known_good,
            commands::kernel_upsert_data_model,
            commands::kernel_grant_permission,
            commands::kernel_revoke_permission,
            commands::kernel_list_permissions,
            commands::kernel_get_recovery_state,
            commands::kernel_set_recovery_mode,
            commands::kernel_enter_safe_startup,
            commands::kernel_clear_recovery,
            commands::kernel_set_recovery_flags,
            commands::kernel_export_package,
            commands::kernel_export_package_bytes,
            commands::kernel_preview_package,
            commands::kernel_import_package,
            commands::kernel_application_summary,
            commands::kernel_data_model_summary,
            commands::kernel_recent_transactions,
            commands::kernel_upsert_test,
            commands::kernel_run_test,
            commands::kernel_visual_checks,
            commands::kernel_garbage_collect,
            commands::kernel_create_job,
            commands::kernel_get_job,
            commands::kernel_interrupt_jobs,
            commands::kernel_set_policy_override,
            commands::kernel_clear_policy_override,
            commands::kernel_unified_search,
            commands::kernel_list_registered_actions,
            commands::kernel_invoke_registered_action,
            commands::kernel_list_pending_approvals,
            commands::kernel_decide_approval,
            commands::kernel_list_runtime_grants,
            commands::kernel_revoke_runtime_grant,
            commands::kernel_list_audit_events,
            commands::kernel_clear_audit_events,
            commands::kernel_set_application_lifecycle,
            commands::kernel_record_build_failure,
            commands::kernel_clear_build_failure,
            commands::kernel_list_build_failures,
            commands::kernel_list_application_versions,
        ])
        .setup(move |app| {
            branding::apply_display_name();
            // Interrupt in-flight application jobs after unclean restart (do not resume provider calls)
            if let Some(state) = app.try_state::<AppState>() {
                let mut db = state.db.lock();
                let _ = application_kernel::lifecycle::interrupt_active_jobs(&mut db);
            }
            let handle = scheduler_handle.clone();
            let app_handle = app.handle().clone();
            automations::spawn_scheduler(app_handle, handle);
            tracing::info!("Coreside setup complete");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Coreside")
        .run(|app, event| match event {
            tauri::RunEvent::Ready => {
                branding::apply_display_name();
            }
            tauri::RunEvent::Exit => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.cancel_all();
                    let crawler = state.crawler.clone();
                    tauri::async_runtime::block_on(async move {
                        let _ = crawler.shutdown().await;
                    });
                }
            }
            _ => {}
        });
}

fn init_tracing(default_level: &str) {
    let filter = EnvFilter::try_from_env("CORESIDE_LOG_LEVEL")
        .or_else(|_| EnvFilter::try_new(default_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = fmt().with_env_filter(filter).with_target(true).try_init();
}
