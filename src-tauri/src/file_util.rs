use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Write a file through a same-directory temporary file, then rename it into place.
pub fn atomic_write_restricted(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let temp_path = temporary_write_path(path);
    let write_result = (|| -> Result<()> {
        let mut file = fs::File::create(&temp_path)
            .with_context(|| format!("failed to create {}", temp_path.display()))?;
        restrict_file_permissions(&temp_path)?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("failed to write {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", temp_path.display()))?;
        fs::rename(&temp_path, path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
        restrict_file_permissions(path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn temporary_write_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("codesail-write");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_name = format!(
        ".{}.tmp.{}.{}",
        file_name,
        std::process::id(),
        timestamp
    );
    path.with_file_name(temp_name)
}

#[cfg(unix)]
fn restrict_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn atomic_write_replaces_file_contents() -> Result<()> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = env::temp_dir().join(format!("codesail-atomic-write-{unique}"));
        let path = dir.join("config.toml");

        atomic_write_restricted(&path, "first\n")?;
        atomic_write_restricted(&path, "second\n")?;

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        assert_eq!(contents, "second\n");

        let _ = fs::remove_dir_all(dir);
        Ok(())
    }
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
