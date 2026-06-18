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

#[cfg(target_os = "macos")]
pub(crate) fn open_claude_command_in_terminal(args: &[&str]) -> Result<()> {
    let script_path = env::temp_dir().join(format!("codesail-claude-{}.command", timestamp_millis()));
    let command_line = std::iter::once("claude")
        .chain(args.iter().copied())
        .map(shell_single_quote)
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        r#"#!/bin/zsh -l
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/.bun/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

if [ -f "$HOME/.zprofile" ]; then
  . "$HOME/.zprofile" >/dev/null 2>&1 || true
fi

if [ -f "$HOME/.zshrc" ]; then
  . "$HOME/.zshrc" >/dev/null 2>&1 || true
fi

{command_line}
status=$?
echo
if [ $status -ne 0 ]; then
  echo "claude failed with exit code $status"
fi
echo "Press any key to close this window..."
read -k 1
"#
    );

    fs::write(&script_path, script)
        .with_context(|| format!("无法创建 Claude 终端脚本: {}", script_path.display()))?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("无法设置 Claude 终端脚本权限: {}", script_path.display()))?;

    Command::new("/usr/bin/open")
        .arg(&script_path)
        .spawn()
        .context("无法打开终端启动 Claude Code")?;

    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn open_claude_command_in_terminal(args: &[&str]) -> Result<()> {
    let mut command_args = vec!["/C", "start", "", "cmd", "/K", "claude"];
    command_args.extend(args.iter().copied());

    Command::new("cmd")
        .args(command_args)
        .spawn()
        .context("无法打开终端启动 Claude Code")?;

    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub(crate) fn open_claude_command_in_terminal(args: &[&str]) -> Result<()> {
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
        command.args(terminal_args).arg("claude").args(args);
        if command.spawn().is_ok() {
            return Ok(());
        }
    }

    Command::new("claude")
        .args(args)
        .spawn()
        .context("无法打开终端启动 Claude Code，请确认 claude 命令可用")?;

    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn open_codex_command_in_terminal(args: &[&str]) -> Result<()> {
    let script_path = env::temp_dir().join(format!("codesail-codex-{}.command", timestamp_millis()));
    let command_line = std::iter::once("codex")
        .chain(args.iter().copied())
        .map(shell_single_quote)
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        r#"#!/bin/zsh -l
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/.bun/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

if [ -f "$HOME/.zprofile" ]; then
  . "$HOME/.zprofile" >/dev/null 2>&1 || true
fi

if [ -f "$HOME/.zshrc" ]; then
  . "$HOME/.zshrc" >/dev/null 2>&1 || true
fi

if ! command -v codex >/dev/null 2>&1 && [ -s "$HOME/.nvm/nvm.sh" ]; then
  . "$HOME/.nvm/nvm.sh" >/dev/null 2>&1 || true
  nvm use --silent default >/dev/null 2>&1 || true
fi

{command_line}
status=$?
echo
if [ $status -ne 0 ]; then
  echo "codex failed with exit code $status"
fi
echo "Press any key to close this window..."
read -k 1
"#
    );

    fs::write(&script_path, script)
        .with_context(|| format!("无法创建 Codex 终端脚本: {}", script_path.display()))?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("无法设置 Codex 终端脚本权限: {}", script_path.display()))?;

    Command::new("/usr/bin/open")
        .arg(&script_path)
        .spawn()
        .context("无法打开终端启动 Codex")?;

    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn open_codex_command_in_terminal(args: &[&str]) -> Result<()> {
    let mut command_args = vec!["/C", "start", "", "cmd", "/K", "codex"];
    command_args.extend(args.iter().copied());

    Command::new("cmd")
        .args(command_args)
        .spawn()
        .context("无法打开终端启动 Codex")?;

    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub(crate) fn open_codex_command_in_terminal(args: &[&str]) -> Result<()> {
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
        command.args(terminal_args).arg("codex").args(args);
        if command.spawn().is_ok() {
            return Ok(());
        }
    }

    Command::new("codex")
        .args(args)
        .spawn()
        .context("无法打开终端启动 Codex，请确认 codex 命令可用")?;

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
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

#[cfg(target_os = "macos")]
fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
