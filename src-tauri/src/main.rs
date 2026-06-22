mod claude_config;
mod codex_config;
mod config;
mod history;
mod storage;
mod tasks;
mod terminal;
mod tools;
mod tray;

use config::{
    check_app_update, check_provider_health, copy_provider, delete_provider, fetch_models,
    get_active_tool_command, get_app_state, get_provider_detail, get_tool_statuses,
    import_codex_providers_to_claude, open_app_update, open_claude_terminal,
    open_codex_terminal, open_tool_install, reorder_providers, restart_codex_app,
    save_provider, set_active_tool_command, set_current_model,
};
use history::{
    delete_history_provider, delete_history_session,
    list_tool_history_sessions, read_history_session, resume_history_session,
};

fn main() {
    env_logger::init();
    tauri::Builder::default()
        .setup(|app| {
            if let Err(e) = tray::setup_tray(app) {
                log::error!("failed to setup system tray: {:?}", e);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            get_tool_statuses,
            check_app_update,
            open_app_update,
            save_provider,
            get_provider_detail,
            copy_provider,
            import_codex_providers_to_claude,
            delete_provider,
            reorder_providers,
            fetch_models,
            check_provider_health,
            set_current_model,
            get_active_tool_command,
            set_active_tool_command,
            restart_codex_app,
            open_codex_terminal,
            open_claude_terminal,
            open_tool_install,
            list_tool_history_sessions,
            read_history_session,
            resume_history_session,
            delete_history_session,
            delete_history_provider
        ])
        .run(tauri::generate_context!())
        .expect("failed to run app");
}
