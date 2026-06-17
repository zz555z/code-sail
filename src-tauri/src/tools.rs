use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::process::{Command, Output};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    name: String,
    command: String,
    available: bool,
    version: Option<String>,
    detail: Option<String>,
    install_label: String,
    install_hint: String,
    install_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenToolInstallRequest {
    pub(crate) command: String,
}

struct ToolInstallInfo {
    label: &'static str,
    hint: &'static str,
    url: &'static str,
}

pub(crate) fn load_tool_statuses() -> Result<Vec<ToolStatus>> {
    Ok(vec![
        load_tool_status("Codex", "codex", &["--version"]),
        load_tool_status("Node.js", "node", &["--version"]),
        load_tool_status("npm", "npm", &["--version"]),
    ])
}

fn load_tool_status(name: &str, command: &str, args: &[&str]) -> ToolStatus {
    let install_info = tool_install_info(command);
    match version_command_output(command, args) {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let version = if stdout.is_empty() { stderr } else { stdout };

            ToolStatus {
                name: name.to_string(),
                command: command.to_string(),
                available: true,
                version: optional_non_empty(&version).map(ToString::to_string),
                detail: None,
                install_label: install_info.label.to_string(),
                install_hint: install_info.hint.to_string(),
                install_url: install_info.url.to_string(),
            }
        }
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();

            ToolStatus {
                name: name.to_string(),
                command: command.to_string(),
                available: false,
                version: None,
                detail: optional_non_empty(&detail).map(ToString::to_string),
                install_label: install_info.label.to_string(),
                install_hint: install_info.hint.to_string(),
                install_url: install_info.url.to_string(),
            }
        }
        Err(error) => ToolStatus {
            name: name.to_string(),
            command: command.to_string(),
            available: false,
            version: None,
            detail: Some(error.to_string()),
            install_label: install_info.label.to_string(),
            install_hint: install_info.hint.to_string(),
            install_url: install_info.url.to_string(),
        },
    }
}

fn tool_install_info(command: &str) -> ToolInstallInfo {
    match command {
        "codex" => ToolInstallInfo {
            label: "安装",
            hint: codex_install_hint(),
            url: "https://developers.openai.com/codex/",
        },
        "node" => ToolInstallInfo {
            label: "下载",
            hint: node_install_hint(),
            url: node_install_url(),
        },
        "npm" => ToolInstallInfo {
            label: "安装",
            hint: npm_install_hint(),
            url: node_install_url(),
        },
        _ => ToolInstallInfo {
            label: "查看",
            hint: "打开安装说明",
            url: "https://developers.openai.com/codex/",
        },
    }
}

#[cfg(target_os = "macos")]
fn codex_install_hint() -> &'static str {
    "打开 Codex 安装说明；macOS 可在终端使用 npm 安装 Codex CLI"
}

#[cfg(target_os = "windows")]
fn codex_install_hint() -> &'static str {
    "打开 Codex 安装说明；Windows 请先确认 Node.js 和 npm 可用"
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn codex_install_hint() -> &'static str {
    "打开 Codex 安装说明；Linux 请先确认 Node.js 和 npm 可用"
}

#[cfg(target_os = "macos")]
fn node_install_hint() -> &'static str {
    "打开 Node.js macOS 下载页"
}

#[cfg(target_os = "windows")]
fn node_install_hint() -> &'static str {
    "打开 Node.js Windows 下载页"
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn node_install_hint() -> &'static str {
    "打开 Node.js Linux 包管理器安装说明"
}

#[cfg(target_os = "macos")]
fn npm_install_hint() -> &'static str {
    "npm 通常随 Node.js 一起安装，打开 macOS 下载页"
}

#[cfg(target_os = "windows")]
fn npm_install_hint() -> &'static str {
    "npm 通常随 Node.js 一起安装，打开 Windows 下载页"
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn npm_install_hint() -> &'static str {
    "npm 通常随 Node.js 一起安装，打开 Linux 安装说明"
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn node_install_url() -> &'static str {
    "https://nodejs.org/en/download"
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn node_install_url() -> &'static str {
    "https://nodejs.org/en/download/package-manager"
}

#[cfg(target_os = "macos")]
fn version_command_output(command: &str, args: &[&str]) -> std::io::Result<Output> {
    let command_line = std::iter::once(command)
        .chain(args.iter().copied())
        .map(shell_single_quote)
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        r#"
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/.bun/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

if [ -f "$HOME/.zprofile" ]; then
  . "$HOME/.zprofile" >/dev/null 2>&1 || true
fi

if [ -f "$HOME/.zshrc" ]; then
  . "$HOME/.zshrc" >/dev/null 2>&1 || true
fi

if ! command -v {command} >/dev/null 2>&1 && [ -s "$HOME/.nvm/nvm.sh" ]; then
  . "$HOME/.nvm/nvm.sh" >/dev/null 2>&1 || true
  nvm use --silent default >/dev/null 2>&1 || true
fi

{command_line}
"#,
        command = shell_single_quote(command),
        command_line = command_line
    );

    Command::new("/bin/zsh")
        .args(["-lc", &script])
        .output()
}

#[cfg(target_os = "macos")]
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn version_command_output(command: &str, args: &[&str]) -> std::io::Result<Output> {
    let command_line = std::iter::once(command)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");

    Command::new("/bin/sh").args(["-c", &command_line]).output()
}

#[cfg(not(unix))]
fn version_command_output(command: &str, args: &[&str]) -> std::io::Result<Output> {
    Command::new(command).args(args).output()
}

pub(crate) fn open_tool_install_inner(command: &str) -> Result<()> {
    let command = command.trim();
    if !matches!(command, "codex" | "node" | "npm") {
        bail!("不支持的依赖项: {command}");
    }

    open_external_url(tool_install_info(command).url)
}

#[cfg(target_os = "macos")]
pub(crate) fn open_external_url(url: &str) -> Result<()> {
    Command::new("/usr/bin/open")
        .arg(url)
        .spawn()
        .with_context(|| format!("无法打开下载页面: {url}"))?;

    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn open_external_url(url: &str) -> Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .with_context(|| format!("无法打开下载页面: {url}"))?;

    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub(crate) fn open_external_url(url: &str) -> Result<()> {
    Command::new("xdg-open")
        .arg(url)
        .spawn()
        .with_context(|| format!("无法打开下载页面: {url}"))?;

    Ok(())
}

fn optional_non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
