mod codex_config;
mod config;
mod history;
mod storage;
mod tasks;
mod terminal;
mod tools;

use config::{
    check_app_update, copy_provider, delete_provider, fetch_models, get_app_state,
    get_tool_statuses, open_app_update, open_codex_terminal, open_tool_install,
    restart_codex_app, save_provider, set_current_model,
};
use history::{
    delete_history_provider, delete_history_session, list_history_sessions, read_history_session,
    resume_history_session,
};

fn main() {
    env_logger::init();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            get_tool_statuses,
            check_app_update,
            open_app_update,
            save_provider,
            copy_provider,
            delete_provider,
            fetch_models,
            set_current_model,
            restart_codex_app,
            open_codex_terminal,
            open_tool_install,
            list_history_sessions,
            read_history_session,
            resume_history_session,
            delete_history_session,
            delete_history_provider
        ])
        .run(tauri::generate_context!())
        .expect("failed to run app");
}
