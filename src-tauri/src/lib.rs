//! Coreside Tauri library entrypoint.

mod ai;
mod app_paths;
mod application_kernel;
mod automations;
mod branding;
mod cache_cleaner;
mod commands;
mod config;
mod crawler;
mod credentials;
mod db;
mod exa;
mod exports;
mod firecrawl;
mod linkup;
mod maintenance;
mod maintenance_journal;
mod media;
mod projects;
mod quiescence;
mod research;
mod runtime_v2;
mod search;
mod security;
mod settings;
mod state;
mod wallpapers;
mod windows;

#[cfg(feature = "e2e")]
mod e2e_support;

use std::sync::Arc;

use automations::SchedulerHandle;
use state::AppState;
use tauri::http;
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

    #[allow(unused_mut)]
    let (mut database, bootstrap) = db::open_profile_or_shell();
    if bootstrap.is_ready() {
        let _ = db::ensure_default_workspace(&database);
        if let Err(e) = crate::runtime_v2::mark_interrupted_in_flight(&database) {
            tracing::warn!(error = %e, "failed to mark interrupted in-flight turns");
        }
        if let Err(e) = crate::runtime_v2::compact_turn_journal(&database, 30, 14) {
            tracing::warn!(error = %e, "failed to compact turn journal on startup");
        }
        // Smart cache cleanup: remove stale staging dirs, orphaned temps, oversized crawler data.
        if let Ok(paths) = app_paths::AppPaths::resolve() {
            let report = cache_cleaner::clean_stale_caches(&paths);
            let total_removed = report.stale_restore_staging_removed
                + report.stale_attachment_staging_removed
                + report.orphaned_temp_files_removed;
            if total_removed > 0 || report.crawler_bytes_freed > 0 || !report.errors.is_empty() {
                tracing::info!(
                    restore_staging = report.stale_restore_staging_removed,
                    attachment_staging = report.stale_attachment_staging_removed,
                    orphaned_temps = report.orphaned_temp_files_removed,
                    crawler_freed = %cache_cleaner::format_bytes(report.crawler_bytes_freed),
                    errors = report.errors.len(),
                    "startup cache cleanup completed"
                );
            }
        }
        #[cfg(feature = "e2e")]
        e2e_support::maybe_seed(&mut database);
    } else {
        tracing::warn!(?bootstrap, "starting Coreside in recovery-required mode");
    }

    let app_state = AppState::new_with_bootstrap(config, database, bootstrap);
    let scheduler_handle = Arc::new(SchedulerHandle::default());

    let builder = tauri::Builder::default().plugin(tauri_plugin_shell::init());

    // WebDriver plugins are compile-gated behind Cargo feature `e2e`.
    // Production `tauri build` / default features never register them.
    #[cfg(feature = "e2e")]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    builder
        .manage(app_state)
        .manage(scheduler_handle.clone())
        .register_uri_scheme_protocol("coreside-asset", |ctx, request| {
            let path = request.uri().path().to_string();
            // Scoped read: /attachment/{conversationId}/{attachmentId}
            let parts: Vec<&str> = path
                .trim_start_matches('/')
                .split('/')
                .filter(|s| !s.is_empty())
                .collect();
            let (conversation_id, attachment_id) = match parts.as_slice() {
                ["attachment", conversation_id, attachment_id] => {
                    (*conversation_id, *attachment_id)
                }
                _ => ("", ""),
            };
            let app = ctx.app_handle();
            match commands::attachment_cmds::read_attachment_bytes_for_protocol(
                app,
                conversation_id,
                attachment_id,
            ) {
                Ok((bytes, mime)) => http::Response::builder()
                    .header(http::header::CONTENT_TYPE, &mime)
                    .header(http::header::CACHE_CONTROL, "private, max-age=60")
                    .header(http::header::X_CONTENT_TYPE_OPTIONS, "nosniff")
                    .body(bytes)
                    .unwrap_or_else(|_| http::Response::new(Vec::new())),
                Err(_) => http::Response::builder()
                    .status(http::StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .unwrap_or_else(|_| http::Response::new(Vec::new())),
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_bootstrap_status,
            commands::retry_open_database,
            commands::get_database_health,
            commands::create_profile_backup,
            commands::list_managed_backups,
            commands::preview_restore_backup,
            commands::restore_profile_backup,
            commands::get_storage_summary,
            commands::get_ai_status,
            commands::get_model_catalog,
            commands::get_onboarding_state,
            commands::upsert_tutorial_progress,
            commands::reset_tutorial_progress,
            commands::seed_tutorial_sample,
            commands::cleanup_tutorial_sample,
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
            commands::cancel_chat_attachment,
            commands::get_chat_attachment_src,
            commands::run_attachment_gc,
            commands::discard_tool_change,
            commands::list_interrupted_turns,
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
            commands::set_workspace_appearance,
            commands::list_added_settings,
            commands::upsert_added_setting,
            commands::delete_added_setting,
            commands::validate_change_targets,
            commands::commit_dock_icon_preference,
            commands::get_dock_icon_status,
            commands::apply_persisted_dock_icon,
            commands::clear_conversations,
            commands::clear_tools,
            commands::open_tool_window,
            commands::window_orchestrator_inspect,
            commands::window_orchestrator_expand,
            commands::window_orchestrator_restore,
            commands::window_orchestrator_cancel,
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
            commands::get_surface_state_with_revision_cmd,
            commands::get_draft_cmd,
            commands::save_draft_cmd,
            commands::delete_draft_cmd,
            commands::schedule_patches_cmd,
            commands::flush_patch_scheduler_cmd,
            commands::get_route_state_cmd,
            commands::set_route_state_cmd,
            commands::navigate_route_cmd,
            commands::route_back_cmd,
            commands::route_forward_cmd,
            commands::get_conversation_events_cmd,
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
            commands::diff_branch_cmd,
            commands::create_snapshot_cmd,
            commands::list_snapshots_cmd,
            commands::get_snapshot_cmd,
            commands::delete_snapshot_cmd,
            commands::enqueue_agent_turn_cmd,
            commands::list_agent_queue_cmd,
            commands::subscribe_conversation_queue,
            commands::subscribe_conversation_sync,
            commands::cancel_queue_item_cmd,
            commands::remove_queue_item_cmd,
            commands::activate_next_queue_cmd,
            commands::complete_queue_item_cmd,
            commands::recover_agent_queue_cmd,
            commands::store_diagnostics_cmd,
            commands::list_diagnostics_cmd,
            commands::list_turn_timeline_cmd,
            commands::runtime_v2_limits,
            commands::kernel_capability_catalog,
            commands::kernel_apply_change,
            commands::kernel_get_proposal,
            commands::kernel_list_pending_proposals,
            commands::kernel_decide_proposal,
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
            let profile_ready = app
                .try_state::<AppState>()
                .map(|s| s.profile_ready())
                .unwrap_or(false);
            if profile_ready {
                if let Some(state) = app.try_state::<AppState>() {
                    let handle = app.handle();
                    if let Err(err) = branding::reconcile_dock_on_startup(handle, &state.db) {
                        tracing::warn!(error = %err, "initial dock reconciliation failed at startup");
                    }
                }
                // Interrupt in-flight application jobs after unclean restart (do not resume provider calls)
                if let Some(state) = app.try_state::<AppState>() {
                    let mut db = state.db.lock();
                    let _ = application_kernel::lifecycle::interrupt_active_jobs(&mut db);
                    // Stale `active` queue rows from a crash must not block the conversation forever.
                    match runtime_v2::recover_stale_active(&mut db) {
                        Ok(n) if n > 0 => {
                            tracing::warn!(count = n, "recovered stale active agent queue items")
                        }
                        Err(err) => tracing::warn!(error = %err, "stale queue recovery failed"),
                        _ => {}
                    }
                }
                let app_handle_for_sweep = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(state) = app_handle_for_sweep.try_state::<AppState>() {
                        match commands::reconcile_and_sweep_attachments(&state) {
                            Ok(report) if report.expired > 0 || report.promoted_orphans > 0 || report.missing_durable > 0 => {
                                tracing::info!(
                                    expired = report.expired,
                                    promoted = report.promoted_orphans,
                                    missing = report.missing_durable,
                                    "attachment sweep completed"
                                );
                            }
                            Err(err) => tracing::warn!(error = %err.message, "attachment sweep failed"),
                            _ => {}
                        }
                    }
                });

                let mut skip_ordinary_services = false;
                if let Ok(paths) = app_paths::AppPaths::resolve() {
                    match maintenance_journal::load_journal(&paths) {
                        Ok(Some(journal)) => {
                            let action = maintenance_journal::classify_unfinished_journal(&journal);
                            tracing::warn!(
                                operation_id = %journal.operation_id,
                                stage = %journal.stage,
                                ?action,
                                "unfinished maintenance journal detected at startup"
                            );
                            match maintenance_journal::apply_startup_decision(
                                &paths, &journal, action,
                            ) {
                                Ok(maintenance_journal::JournalStartupOutcome::Cleared) => {
                                    if matches!(
                                        action,
                                        maintenance_journal::JournalStartupAction::RollBack
                                    ) {
                                        if let Some(staged) = journal.staged_profile.as_deref() {
                                            let staged_path = std::path::PathBuf::from(staged);
                                            if staged_path.exists() {
                                                tracing::warn!(
                                                    path = %staged_path.display(),
                                                    "pre-swap rollback left staged tree for Recovery inspection"
                                                );
                                            }
                                        }
                                        tracing::info!(
                                            "pre-swap maintenance journal cleared after safe rollback decision"
                                        );
                                    } else {
                                        tracing::info!(
                                            "stale completed/inactive maintenance journal cleared"
                                        );
                                    }
                                }
                                Ok(maintenance_journal::JournalStartupOutcome::NoOp) => {}
                                Ok(maintenance_journal::JournalStartupOutcome::EnterRecovery {
                                    reason,
                                }) => {
                                    tracing::error!(
                                        %reason,
                                        "maintenance journal requires Recovery — refusing ordinary service start"
                                    );
                                    skip_ordinary_services = true;
                                    if let Some(state) = app.try_state::<AppState>() {
                                        state.quiescence.pause();
                                        let mut db = state.db.lock();
                                        let _ = application_kernel::recovery::enter_safe_startup(
                                            &mut db,
                                            reason,
                                        );
                                        *state.bootstrap.lock() =
                                            crate::db::BootstrapStatus::recovery(
                                                "maintenance_journal",
                                                "Coreside found an unfinished maintenance operation. Use Recovery tools before continuing.",
                                                None,
                                                false,
                                            );
                                    }
                                }
                                Err(err) => {
                                    tracing::error!(
                                        error = %err.message,
                                        "maintenance journal startup action failed — entering Recovery"
                                    );
                                    skip_ordinary_services = true;
                                    if let Some(state) = app.try_state::<AppState>() {
                                        state.quiescence.pause();
                                        let mut db = state.db.lock();
                                        let _ = application_kernel::recovery::enter_safe_startup(
                                            &mut db,
                                            "maintenance journal startup action failed",
                                        );
                                        *state.bootstrap.lock() =
                                            crate::db::BootstrapStatus::recovery(
                                                "maintenance_journal",
                                                "Coreside could not apply a maintenance journal startup decision. Use Recovery tools before continuing.",
                                                None,
                                                false,
                                            );
                                    }
                                }
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            tracing::error!(
                                error = %err.message,
                                "maintenance journal unreadable — entering Recovery"
                            );
                            skip_ordinary_services = true;
                            if let Some(state) = app.try_state::<AppState>() {
                                state.quiescence.pause();
                                let mut db = state.db.lock();
                                let _ = application_kernel::recovery::enter_safe_startup(
                                    &mut db,
                                    "unreadable maintenance journal",
                                );
                                *state.bootstrap.lock() = crate::db::BootstrapStatus::recovery(
                                    "maintenance_journal",
                                    "Coreside could not read the maintenance journal. Use Recovery tools before continuing.",
                                    None,
                                    false,
                                );
                            }
                        }
                    }
                }

                if skip_ordinary_services {
                    tracing::warn!(
                        "skipping automation scheduler — maintenance Recovery required"
                    );
                } else {
                    let handle = scheduler_handle.clone();
                    let app_handle = app.handle().clone();
                    automations::spawn_scheduler(app_handle, handle);
                }
            } else {
                tracing::warn!(
                    "skipping automation scheduler and job interruption — recovery shell active"
                );
            }
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
