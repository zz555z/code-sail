use anyhow::{Context, Result};
use std::{process::Command, thread, time::Duration};

#[cfg(target_os = "macos")]
use std::{
    env, fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;

pub(crate) fn open_codex_terminal_inner() -> Result<()> {
    open_codex_command_in_terminal(&[])
}

pub(crate) fn open_claude_terminal_inner() -> Result<()> {
    open_claude_command_in_terminal(&[])
}

struct ToolTerminalConfig {
    command: &'static str,
    context_label: &'static str,
    #[cfg(target_os = "macos")]
    extra_script: &'static str,
}

fn claude_config() -> ToolTerminalConfig {
    ToolTerminalConfig {
        command: "claude",
        context_label: "Claude Code",
        #[cfg(target_os = "macos")]
        extra_script: "",
    }
}

fn codex_config() -> ToolTerminalConfig {
    ToolTerminalConfig {
        command: "codex",
        context_label: "Codex",
        #[cfg(target_os = "macos")]
        extra_script: r#"if ! command -v codex >/dev/null 2>&1 && [ -s "$HOME/.nvm/nvm.sh" ]; then
  . "$HOME/.nvm/nvm.sh" >/dev/null 2>&1 || true
  nvm use --silent default >/dev/null 2>&1 || true
fi

"#,
    }
}

pub(crate) fn open_claude_command_in_terminal(args: &[&str]) -> Result<()> {
    open_tool_in_terminal(&claude_config(), args)
}

pub(crate) fn open_codex_command_in_terminal(args: &[&str]) -> Result<()> {
    open_tool_in_terminal(&codex_config(), args)
}

#[cfg(target_os = "macos")]
fn open_tool_in_terminal(config: &ToolTerminalConfig, args: &[&str]) -> Result<()> {
    let script_path = env::temp_dir().join(format!(
        "codesail-{}-{}.command",
        config.command,
        timestamp_millis()
    ));
    let command_line = std::iter::once(config.command)
        .chain(args.iter().copied())
        .map(shell_single_quote)
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        r#"#!/bin/zsh -l
{path_setup}

{extra_script}{command_line}
status=$?
echo
if [ $status -ne 0 ]; then
  echo "{command} failed with exit code $status"
fi
echo "Press any key to close this window..."
read -k 1
"#,
        path_setup = crate::file_util::MACOS_ZSH_PATH_SETUP,
        extra_script = config.extra_script,
        command_line = command_line,
        command = config.command,
    );

    fs::write(&script_path, script).with_context(|| {
        format!(
            "无法创建 {} 终端脚本: {}",
            config.context_label,
            script_path.display()
        )
    })?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o700))
        .with_context(|| {
            format!(
                "无法设置 {} 终端脚本权限: {}",
                config.context_label,
                script_path.display()
            )
        })?;

    Command::new("/usr/bin/open")
        .arg(&script_path)
        .spawn()
        .with_context(|| format!("无法打开终端启动 {}", config.context_label))?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn open_tool_in_terminal(config: &ToolTerminalConfig, args: &[&str]) -> Result<()> {
    let mut command_args = vec!["/C", "start", "", "cmd", "/K", config.command];
    command_args.extend(args.iter().copied());

    Command::new("cmd")
        .args(command_args)
        .spawn()
        .with_context(|| format!("无法打开终端启动 {}", config.context_label))?;

    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn open_tool_in_terminal(config: &ToolTerminalConfig, args: &[&str]) -> Result<()> {
    let candidates: [(&str, &[&str]); 7] = [
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("kgx", &["--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["-e"]),
        ("mate-terminal", &["-e"]),
        ("lxterminal", &["-e"]),
    ];

    for (terminal, terminal_args) in candidates {
        let mut command = Command::new(terminal);
        command.args(terminal_args).arg(config.command).args(args);
        if command.spawn().is_ok() {
            return Ok(());
        }
    }

    Command::new(config.command)
        .args(args)
        .spawn()
        .with_context(|| {
            format!(
                "无法打开终端启动 {}，请确认 {} 命令可用",
                config.context_label, config.command
            )
        })?;

    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn restart_codex_app_inner() -> Result<()> {
    let _ = Command::new("/usr/bin/pkill").args(["-x", "Codex"]).status();
    thread::sleep(Duration::from_millis(800));

    let app_status = Command::new("/usr/bin/open")
        .args(["-a", "Codex"])
        .status();

    if matches!(app_status, Ok(status) if status.success()) {
        return Ok(());
    }

    open_codex_command_in_terminal(&[])
        .context("无法重新打开 Codex App，也无法在终端启动 Codex")
}

#[cfg(target_os = "windows")]
pub(crate) fn restart_codex_app_inner() -> Result<()> {
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "Codex.exe"])
        .status();
    thread::sleep(Duration::from_millis(800));

    let app_status = Command::new("cmd")
        .args(["/C", "start", "", "Codex"])
        .status();

    if matches!(app_status, Ok(status) if status.success()) {
        return Ok(());
    }

    open_codex_command_in_terminal(&[])
        .context("无法重新打开 Codex App，也无法在终端启动 Codex")
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub(crate) fn restart_codex_app_inner() -> Result<()> {
    let _ = Command::new("pkill").args(["-x", "codex"]).status();
    thread::sleep(Duration::from_millis(800));

    open_codex_command_in_terminal(&[])
        .context("无法重新打开 Codex App，也无法在终端启动 Codex")?;

    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

#[cfg(target_os = "macos")]
pub(crate) fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
