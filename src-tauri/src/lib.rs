//! Coreside Tauri library entrypoint.

mod ai;
mod commands;
mod config;
mod db;
mod security;
mod state;
mod windows;

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

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_ai_status,
            commands::test_ai_connection,
            commands::list_conversations,
            commands::create_conversation,
            commands::delete_conversation,
            commands::get_messages,
            commands::send_message,
            commands::cancel_request,
            commands::discard_tool_change,
            commands::apply_tool_change,
            commands::list_tools,
            commands::get_tool,
            commands::get_tool_versions,
            commands::undo_tool_change,
            commands::save_tool_state,
            commands::get_tool_state,
            commands::get_settings,
            commands::set_setting,
            commands::clear_conversations,
            commands::clear_tools,
            commands::open_tool_window,
        ])
        .setup(|_app| {
            tracing::info!("Coreside setup complete");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Coreside")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<AppState>() {
                    state.cancel_all();
                }
            }
        });
}

fn init_tracing(default_level: &str) {
    let filter = EnvFilter::try_from_env("CORESIDE_LOG_LEVEL")
        .or_else(|_| EnvFilter::try_new(default_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
}
