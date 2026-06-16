use anyhow::{bail, Context, Result};
use std::{env, fs, process::Command, thread, time::Duration};

#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;

#[cfg(target_os = "macos")]
pub(crate) fn open_codex_terminal_inner() -> Result<()> {
    let script_path = env::temp_dir().join("codex-open-terminal.command");
    let script = r#"#!/bin/zsh -l
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
codex
status=$?
echo
if [ $status -ne 0 ]; then
  echo "codex failed with exit code $status"
fi
echo "Press any key to close this window..."
read -k 1
"#;

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
pub(crate) fn open_codex_terminal_inner() -> Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "cmd", "/K", "codex"])
        .spawn()
        .context("无法打开终端启动 Codex")?;

    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub(crate) fn open_codex_terminal_inner() -> Result<()> {
    let candidates = ["x-terminal-emulator", "gnome-terminal", "konsole", "xfce4-terminal"];
    for terminal in candidates {
        if Command::new(terminal).arg("-e").arg("codex").spawn().is_ok() {
            return Ok(());
        }
    }

    Command::new("codex")
        .spawn()
        .context("无法打开终端启动 Codex，请确认 codex 命令可用")?;

    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn restart_codex_app_inner() -> Result<()> {
    let _ = Command::new("/usr/bin/pkill").args(["-x", "Codex"]).status();
    thread::sleep(Duration::from_millis(800));

    let status = Command::new("/usr/bin/open")
        .args(["-a", "Codex"])
        .status()
        .context("无法重新打开 Codex App")?;

    if !status.success() {
        bail!("无法重新打开 Codex App，请确认已安装名为 Codex 的应用");
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn restart_codex_app_inner() -> Result<()> {
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "Codex.exe"])
        .status();
    thread::sleep(Duration::from_millis(800));

    let status = Command::new("cmd")
        .args(["/C", "start", "", "Codex"])
        .status()
        .context("无法重新打开 Codex App")?;

    if !status.success() {
        bail!("无法重新打开 Codex App，请确认 Codex 已安装并可从系统启动");
    }

    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub(crate) fn restart_codex_app_inner() -> Result<()> {
    let _ = Command::new("pkill").args(["-x", "codex"]).status();
    thread::sleep(Duration::from_millis(800));

    Command::new("codex")
        .spawn()
        .context("无法重新打开 Codex App，请确认 codex 命令可用")?;

    Ok(())
}
