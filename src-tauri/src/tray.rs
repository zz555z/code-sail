use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::sync::mpsc;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Emitter, Manager,
};

use crate::codex_config::{codex_config_path, import_codex_config_if_needed, sync_codex_files};
use crate::storage::{
    collect_providers_from_database, get_active_model, get_active_provider, get_active_tool,
    open_database, set_active_model, set_active_provider, set_active_tool, ToolType,
};
use crate::tasks::run_background_task;
use crate::terminal::{
    open_claude_terminal_inner, open_codex_terminal_inner, restart_codex_app_inner,
};

const TRAY_ID: &str = "codesail-main-tray";
const MENU_OPEN_APP: &str = "open_app";
const MENU_OPEN_TERMINAL: &str = "open_terminal";
const MENU_RESTART_CODEX: &str = "restart_codex";
const MENU_QUIT: &str = "quit";
const SWITCH_PROVIDER_PREFIX: &str = "switch_provider:";
const SWITCH_TOOL_PREFIX: &str = "switch_tool:";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraySwitchPayload {
    provider_id: String,
    provider_name: String,
    model: Option<String>,
}

pub fn setup_tray(app: &App) -> Result<()> {
    let menu = build_tray_menu(app)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::include_image!("./icons/tray-template.png"))
        .icon_as_template(true)
        .tooltip("CodeSail")
        .menu(&menu)
        .on_menu_event(|app, event| {
            let id = event.id().0.as_str();

            if let Some(provider_id) = id.strip_prefix(SWITCH_PROVIDER_PREFIX) {
                handle_switch_provider(app, provider_id);
                return;
            }

            if let Some(tool_name) = id.strip_prefix(SWITCH_TOOL_PREFIX) {
                handle_switch_tool(app, tool_name);
                return;
            }

            match id {
                MENU_OPEN_APP => show_main_window(app),
                MENU_OPEN_TERMINAL => {
                    std::thread::spawn({
                        let app = app.clone();
                        move || {
                            if let Err(e) = open_active_tool_terminal_inner(&app) {
                                log::error!("tray open terminal failed: {:?}", e);
                            }
                        }
                    });
                }
                MENU_RESTART_CODEX => {
                    std::thread::spawn({
                        let app = app.clone();
                        move || {
                            if let Err(e) = restart_active_tool_inner(&app) {
                                log::error!("tray restart failed: {:?}", e);
                            }
                        }
                    });
                }
                MENU_QUIT => {
                    app.exit(0);
                }
                _ => {
                    log::debug!("tray unhandled menu event: {}", id);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                show_main_window(app);
            }
        })
        .build(app)
        .context("failed to build tray icon")?;

    Ok(())
}

#[tauri::command]
pub async fn refresh_tray_menu(app: AppHandle) -> Result<(), String> {
    run_background_task("codesail-refresh-tray-menu", move || {
        refresh_tray_menu_inner(&app)
    })
    .await
    .map_err(|error| error.to_string())
}

fn refresh_tray_menu_inner(app: &AppHandle) -> Result<()> {
    let app = app.clone();
    let app_for_callback = app.clone();
    let (tx, rx) = mpsc::channel();

    app.run_on_main_thread(move || {
        let result =
            refresh_tray_menu_on_main_thread(&app_for_callback).map_err(|error| error.to_string());
        let _ = tx.send(result);
    })
    .context("failed to schedule tray menu refresh")?;

    match rx
        .recv()
        .context("failed to receive tray menu refresh result")?
    {
        Ok(()) => Ok(()),
        Err(error) => bail!(error),
    }
}

fn refresh_tray_menu_on_main_thread(app: &AppHandle) -> Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let menu = build_tray_menu(app)?;
    tray.set_menu(Some(menu))
        .context("failed to refresh tray menu")?;
    Ok(())
}

fn open_active_tool_terminal_inner(app: &AppHandle) -> Result<()> {
    match active_tool_from_database()? {
        ToolType::Codex => open_codex_terminal_inner(),
        ToolType::Claude => open_claude_terminal_inner(),
    }?;
    refresh_tray_menu_inner(app)?;
    Ok(())
}

fn restart_active_tool_inner(app: &AppHandle) -> Result<()> {
    if active_tool_from_database()? == ToolType::Codex {
        restart_codex_app_inner()?;
    }
    refresh_tray_menu_inner(app)?;
    Ok(())
}

fn active_tool_from_database() -> Result<ToolType> {
    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    import_codex_config_if_needed(&conn, &config_path)?;
    get_active_tool(&conn)
}

fn build_tray_menu<M: Manager<tauri::Wry>>(app: &M) -> Result<tauri::menu::Menu<tauri::Wry>> {
    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    import_codex_config_if_needed(&conn, &config_path)?;
    let active_tool = get_active_tool(&conn).unwrap_or_default();
    let active_provider = get_active_provider(&conn, active_tool)?.unwrap_or_default();
    let active_model = get_active_model(&conn, active_tool)?.unwrap_or_default();
    let providers = collect_providers_from_database(&conn, active_tool)?;

    let tool_label = match active_tool {
        ToolType::Codex => "Codex",
        ToolType::Claude => "Claude",
    };

    // Status line - 只显示模型名称
    let active_label = if active_model.is_empty() {
        "未选择模型".to_string()
    } else {
        active_model.clone()
    };

    let status_item = MenuItemBuilder::new(active_label)
        .id("status")
        .enabled(false)
        .build(app)?;

    // 切换配置 submenu (Codex / Claude)
    let codex_item = MenuItemBuilder::new("Codex")
        .id(format!("{}codex", SWITCH_TOOL_PREFIX))
        .enabled(active_tool != ToolType::Codex)
        .build(app)?;
    let claude_item = MenuItemBuilder::new("Claude")
        .id(format!("{}claude", SWITCH_TOOL_PREFIX))
        .enabled(active_tool != ToolType::Claude)
        .build(app)?;
    let switch_config_submenu = SubmenuBuilder::new(app, "切换配置")
        .item(&codex_item)
        .item(&claude_item)
        .build()?;

    // 切换模型 submenu (providers for active tool)
    let mut switch_model_submenu_builder = SubmenuBuilder::new(app, "切换模型");
    for provider in &providers {
        let is_active = provider.id == active_provider;
        let display_name = provider.name.as_deref().unwrap_or(&provider.id);
        let label = if is_active {
            format!("● {display_name}")
        } else {
            display_name.to_string()
        };
        let item = MenuItemBuilder::new(label)
            .id(format!("{}{}", SWITCH_PROVIDER_PREFIX, provider.id))
            .enabled(!is_active)
            .build(app)?;
        switch_model_submenu_builder = switch_model_submenu_builder.item(&item);
    }
    if providers.is_empty() {
        let empty_item = MenuItemBuilder::new("暂无配置")
            .id("no_providers")
            .enabled(false)
            .build(app)?;
        switch_model_submenu_builder = switch_model_submenu_builder.item(&empty_item);
    }
    let switch_model_submenu = switch_model_submenu_builder.build()?;

    // Tool-specific actions
    let terminal_label = format!("打开 {} CLI", tool_label);
    let open_terminal = MenuItemBuilder::new(terminal_label)
        .id(MENU_OPEN_TERMINAL)
        .build(app)?;

    let open_app = MenuItemBuilder::new("打开 CodeSail")
        .id(MENU_OPEN_APP)
        .build(app)?;
    let quit = MenuItemBuilder::new("退出")
        .id(MENU_QUIT)
        .build(app)?;

    let separator1 = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let separator3 = PredefinedMenuItem::separator(app)?;

    let mut menu_builder = MenuBuilder::new(app)
        .item(&status_item)
        .item(&separator1)
        .item(&switch_config_submenu)
        .item(&switch_model_submenu)
        .item(&separator2)
        .item(&open_terminal);

    // Add restart option only for Codex
    if active_tool == ToolType::Codex {
        let restart_codex = MenuItemBuilder::new("重启 Codex")
            .id(MENU_RESTART_CODEX)
            .build(app)?;
        menu_builder = menu_builder.item(&restart_codex);
    }

    let menu = menu_builder
        .item(&separator3)
        .item(&open_app)
        .item(&quit)
        .build()?;

    Ok(menu)
}

fn handle_switch_provider(app: &tauri::AppHandle, provider_id: &str) {
    let provider_id = provider_id.to_string();
    let app_handle = app.clone();

    std::thread::spawn(move || {
        if let Err(e) = switch_provider_inner(&app_handle, &provider_id) {
            log::error!("tray switch provider failed: {:?}", e);
        }
    });
}

fn handle_switch_tool(app: &tauri::AppHandle, tool_name: &str) {
    let tool_type = match tool_name {
        "codex" => ToolType::Codex,
        "claude" => ToolType::Claude,
        _ => {
            log::error!("tray switch tool: unknown tool {}", tool_name);
            return;
        }
    };
    let app_handle = app.clone();

    std::thread::spawn(move || {
        if let Err(e) = switch_tool_inner(&app_handle, tool_type) {
            log::error!("tray switch tool failed: {:?}", e);
        }
    });
}

fn switch_provider_inner(app: &tauri::AppHandle, provider_id: &str) -> Result<()> {
    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    import_codex_config_if_needed(&conn, &config_path)?;
    let active_tool = get_active_tool(&conn).unwrap_or_default();

    let providers = collect_providers_from_database(&conn, active_tool)?;
    let provider = providers
        .iter()
        .find(|p| p.id == provider_id)
        .context("provider not found")?;

    set_active_provider(&conn, active_tool, provider_id)?;
    if let Some(ref model) = provider.model {
        set_active_model(&conn, active_tool, model)?;
    }

    // Sync config for the appropriate tool
    match active_tool {
        ToolType::Codex => sync_codex_files(&conn, &config_path)?,
        ToolType::Claude => {
            crate::claude_config::sync_claude_settings(&conn, &config_path)?;
        }
    }

    let provider_name = provider.name.clone().unwrap_or_else(|| provider.id.clone());
    let payload = TraySwitchPayload {
        provider_id: provider.id.clone(),
        provider_name: provider_name.clone(),
        model: provider.model.clone(),
    };

    refresh_tray_menu_inner(app)?;
    let _ = app.emit("tray-switch-provider", &payload);

    log::info!(
        "tray switched provider: id={} name={}",
        provider.id,
        provider_name
    );

    Ok(())
}

fn switch_tool_inner(app: &tauri::AppHandle, tool_type: ToolType) -> Result<()> {
    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    import_codex_config_if_needed(&conn, &config_path)?;

    set_active_tool(&conn, tool_type)?;

    // Sync config for the newly active tool
    match tool_type {
        ToolType::Codex => sync_codex_files(&conn, &config_path)?,
        ToolType::Claude => {
            crate::claude_config::sync_claude_settings(&conn, &config_path)?;
        }
    }

    refresh_tray_menu_inner(app)?;

    let tool_name = match tool_type {
        ToolType::Codex => "codex",
        ToolType::Claude => "claude",
    };
    let _ = app.emit("tray-switch-tool", tool_name);

    log::info!("tray switched tool: {}", tool_name);

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
