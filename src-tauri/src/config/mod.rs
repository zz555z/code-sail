mod health;
mod models;
mod providers;
mod update;

pub use health::check_provider_health;
pub use models::fetch_models;
pub use providers::{
    copy_provider, delete_provider, get_provider_detail, import_codex_providers_to_claude,
    reorder_providers, save_provider, set_current_model,
};
pub use update::{check_app_update, open_app_update};

use anyhow::Result;
use serde::Serialize;

use crate::codex_config::{codex_config_path, import_codex_config_if_needed};
use crate::storage::{
    collect_providers_from_database, get_active_model, get_active_provider, get_active_tool,
    open_database,
    set_active_tool as set_active_tool_db, ProviderView, ToolType,
};
use crate::tasks::run_background_task;
use crate::terminal::{
    open_claude_terminal_inner, open_codex_terminal_inner, restart_codex_app_inner,
};
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
    active_tool: ToolType,
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
pub async fn open_claude_terminal() -> Result<(), String> {
    run_background_task("claude-open-terminal", open_claude_terminal_inner)
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

#[tauri::command]
pub async fn get_active_tool_command() -> Result<String, String> {
    run_background_task("codex-get-active-tool", || {
        let config_path = codex_config_path()?;
        let conn = open_database(&config_path)?;
        let tool = get_active_tool(&conn)?;
        Ok(tool.as_str().to_string())
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_active_tool_command(tool_type: String) -> Result<(), String> {
    run_background_task("codex-set-active-tool", move || {
        let tool = ToolType::from_str(&tool_type)?;
        let config_path = codex_config_path()?;
        let conn = open_database(&config_path)?;
        set_active_tool_db(&conn, tool)?;
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())
}

fn load_app_state() -> Result<AppState> {
    let config_path = codex_config_path()?;
    let config_exists = config_path.exists();
    let conn = open_database(&config_path)?;
    import_codex_config_if_needed(&conn, &config_path)?;
    let active_tool = get_active_tool(&conn)?;

    Ok(AppState {
        config_path: config_path.display().to_string(),
        config_exists,
        active_provider: get_active_provider(&conn, active_tool)?,
        active_model: get_active_model(&conn, active_tool)?,
        providers: collect_providers_from_database(&conn, active_tool)?,
        active_tool,
    })
}
