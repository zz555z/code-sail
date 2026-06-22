use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

/// Create a backup of a file if it exists.
/// Returns the backup path if a backup was created, None if the file didn't exist.
/// The backup filename includes a timestamp to avoid overwriting previous backups.
pub fn backup_file_if_exists(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }

    let backup = PathBuf::from(format!(
        "{}.bak.{}",
        path.display(),
        Local::now().format("%Y%m%d_%H%M%S")
    ));
    if !backup.exists() {
        fs::copy(path, &backup).with_context(|| {
            format!(
                "failed to create backup {} from {}",
                backup.display(),
                path.display()
            )
        })?;
    }
    Ok(Some(backup))
}

/// Common shell PATH setup and profile sourcing for macOS zsh scripts.
/// This ensures consistent environment setup across terminal and tool detection.
#[cfg(target_os = "macos")]
pub const MACOS_ZSH_PATH_SETUP: &str = r#"export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/.bun/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

if [ -f "$HOME/.zprofile" ]; then
  . "$HOME/.zprofile" >/dev/null 2>&1 || true
fi

if [ -f "$HOME/.zshrc" ]; then
  . "$HOME/.zshrc" >/dev/null 2>&1 || true
fi
"#;
