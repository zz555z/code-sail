mod models;
mod providers;
mod update;

pub use models::fetch_models;
pub use providers::{copy_provider, delete_provider, save_provider, set_current_model};
pub use update::{check_app_update, open_app_update};

use anyhow::Result;
use serde::Serialize;

use crate::codex_config::codex_config_path;
use crate::storage::{collect_providers_from_database, get_setting, open_database, ProviderView};
use crate::tasks::run_background_task;
use crate::terminal::{open_codex_terminal_inner, restart_codex_app_inner};
use crate::tools::{
    load_tool_statuses, open_tool_install_inner, OpenToolInstallRequest, ToolStatus,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    config_path: String,
    config_exists: bool,
    active_provider: Option<String>,
    active_model: Option<String>,
    providers: Vec<ProviderView>,
}

#[tauri::command]
pub async fn get_app_state() -> Result<AppState, String> {
    run_background_task("codex-load-app-state", load_app_state)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_tool_statuses() -> Result<Vec<ToolStatus>, String> {
    run_background_task("codex-load-tool-statuses", load_tool_statuses)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn restart_codex_app() -> Result<(), String> {
    run_background_task("codex-restart-app", restart_codex_app_inner)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn open_codex_terminal() -> Result<(), String> {
    run_background_task("codex-open-terminal", open_codex_terminal_inner)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn open_tool_install(input: OpenToolInstallRequest) -> Result<(), String> {
    run_background_task("codex-open-tool-install", move || {
        open_tool_install_inner(&input.command)
    })
    .await
    .map_err(|error| error.to_string())
}

fn load_app_state() -> Result<AppState> {
    let config_path = codex_config_path()?;
    let config_exists = config_path.exists();
    let conn = open_database(&config_path)?;

    Ok(AppState {
        config_path: config_path.display().to_string(),
        config_exists,
        active_provider: get_setting(&conn, "active_provider")?,
        active_model: get_setting(&conn, "active_model")?,
        providers: collect_providers_from_database(&conn, &config_path)?,
    })
}
