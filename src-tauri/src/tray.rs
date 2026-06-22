use anyhow::{Context, Result};
use serde::Serialize;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Emitter, Manager,
};

use crate::codex_config::{codex_config_path, sync_codex_files};
use crate::storage::{
    collect_providers_from_database, get_active_model, get_active_provider, get_active_tool,
    open_database, set_active_model, set_active_provider, ToolType,
};
use crate::terminal::{open_codex_terminal_inner, restart_codex_app_inner};

const MENU_OPEN_APP: &str = "open_app";
const MENU_OPEN_TERMINAL: &str = "open_terminal";
const MENU_RESTART_CODEX: &str = "restart_codex";
const MENU_QUIT: &str = "quit";
const SWITCH_PROVIDER_PREFIX: &str = "switch_provider:";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraySwitchPayload {
    provider_id: String,
    provider_name: String,
    model: Option<String>,
}

pub fn setup_tray(app: &App) -> Result<()> {
    let menu = build_tray_menu(app)?;

    let icon = app
        .default_window_icon()
        .cloned()
        .context("no default window icon")?;

    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("CodeSail")
        .menu(&menu)
        .on_menu_event(|app, event| {
            let id = event.id().0.as_str();

            if let Some(provider_id) = id.strip_prefix(SWITCH_PROVIDER_PREFIX) {
                handle_switch_provider(app, provider_id);
                return;
            }

            match id {
                MENU_OPEN_APP => show_main_window(app),
                MENU_OPEN_TERMINAL => {
                    std::thread::spawn(|| {
                        if let Err(e) = open_codex_terminal_inner() {
                            log::error!("tray open terminal failed: {:?}", e);
                        }
                    });
                }
                MENU_RESTART_CODEX => {
                    std::thread::spawn(|| {
                        if let Err(e) = restart_codex_app_inner() {
                            log::error!("tray restart codex failed: {:?}", e);
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

fn build_tray_menu(app: &App) -> Result<tauri::menu::Menu<tauri::Wry>> {
    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let active_tool = get_active_tool(&conn).unwrap_or_default();
    let active_provider = get_active_provider(&conn, active_tool)?.unwrap_or_default();
    let active_model = get_active_model(&conn, active_tool)?.unwrap_or_default();
    let providers = collect_providers_from_database(&conn, active_tool)?;

    let tool_label = match active_tool {
        ToolType::Codex => "Codex",
        ToolType::Claude => "Claude",
    };

    let active_label = if active_provider.is_empty() {
        format!("[{tool_label}] 未选择模型")
    } else {
        let provider_name = providers
            .iter()
            .find(|p| p.id == active_provider)
            .and_then(|p| p.name.as_deref())
            .unwrap_or(&active_provider);
        if active_model.is_empty() {
            format!("[{tool_label}] {provider_name}")
        } else {
            format!("[{tool_label}] {provider_name} / {active_model}")
        }
    };

    let status_item = MenuItemBuilder::new(active_label)
        .id("status")
        .enabled(false)
        .build(app)?;

    let mut switch_submenu_builder = SubmenuBuilder::new(app, "切换模型");
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
        switch_submenu_builder = switch_submenu_builder.item(&item);
    }
    if providers.is_empty() {
        let empty_item = MenuItemBuilder::new("暂无配置")
            .id("no_providers")
            .enabled(false)
            .build(app)?;
        switch_submenu_builder = switch_submenu_builder.item(&empty_item);
    }
    let switch_submenu = switch_submenu_builder.build()?;

    let terminal_label = format!("打开 {} 终端", tool_label);
    let restart_label = format!("重启 {}", tool_label);
    let open_terminal = MenuItemBuilder::new(terminal_label)
        .id(MENU_OPEN_TERMINAL)
        .build(app)?;
    let restart_codex = MenuItemBuilder::new(restart_label)
        .id(MENU_RESTART_CODEX)
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

    let menu = MenuBuilder::new(app)
        .item(&status_item)
        .item(&separator1)
        .item(&switch_submenu)
        .item(&separator2)
        .item(&open_terminal)
        .item(&restart_codex)
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

fn switch_provider_inner(app: &tauri::AppHandle, provider_id: &str) -> Result<()> {
    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
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

    let _ = app.emit("tray-switch-provider", &payload);

    log::info!(
        "tray switched provider: id={} name={}",
        provider.id,
        provider_name
    );

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
