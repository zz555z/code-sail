use anyhow::{Context, Result};
use chrono::Local;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::storage::{
    get_active_model, get_active_provider, get_provider, restrict_file_permissions,
    SqliteConnection, ToolType,
};

const CLAUDE_SETTINGS_SCHEMA: &str = "https://json.schemastore.org/claude-code-settings.json";
pub(crate) const ANTHROPIC_VERSION: &str = "2023-06-01";

const ANTHROPIC_API_KEY_ENV: &str = "ANTHROPIC_API_KEY";
const ANTHROPIC_AUTH_TOKEN_ENV: &str = "ANTHROPIC_AUTH_TOKEN";
const ANTHROPIC_BASE_URL_ENV: &str = "ANTHROPIC_BASE_URL";
const ANTHROPIC_DEFAULT_HAIKU_MODEL_ENV: &str = "ANTHROPIC_DEFAULT_HAIKU_MODEL";
const ANTHROPIC_DEFAULT_OPUS_MODEL_ENV: &str = "ANTHROPIC_DEFAULT_OPUS_MODEL";
const ANTHROPIC_DEFAULT_SONNET_MODEL_ENV: &str = "ANTHROPIC_DEFAULT_SONNET_MODEL";

pub(crate) fn claude_settings_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to locate the home directory")?;
    Ok(home.join(".claude").join("settings.json"))
}

/// Syncs the active Claude provider's credentials into ~/.claude/settings.json.
///
/// Only updates Claude Code API env keys;
/// all other keys in settings.json are preserved.
pub(crate) fn sync_claude_settings(conn: &SqliteConnection, config_path: &Path) -> Result<()> {
    let settings_path = claude_settings_path()?;

    // Determine active provider for Claude tool type
    let active_provider = get_active_provider(conn, ToolType::Claude)?;
    let active_model = get_active_model(conn, ToolType::Claude)?;

    let mut root = read_or_new_json(&settings_path)?;
    ensure_claude_settings_shape(&mut root);

    match active_provider.as_deref() {
        Some(provider_id) if !provider_id.is_empty() => {
            let provider = get_provider(conn, config_path, provider_id)?;

            if let Some(ref provider) = provider {
                let base_url = provider.base_url.trim();

                {
                    let env = root["env"]
                        .as_object_mut()
                        .context("env is not an object")?;
                    clear_claude_managed_env_vars(env);

                    if let Some(ref token) = provider.token {
                        write_claude_env_string(env, ANTHROPIC_AUTH_TOKEN_ENV, token);
                    }

                    write_claude_env_string(env, ANTHROPIC_BASE_URL_ENV, base_url);

                    let model = active_model
                        .as_deref()
                        .filter(|model| !model.trim().is_empty())
                        .or_else(|| provider.model.as_deref().filter(|model| !model.trim().is_empty()));
                    if let Some(model) = model {
                        write_claude_default_model_env(env, model);
                    }
                }

                root.as_object_mut()
                    .context("settings root is not an object")?
                    .remove("model");
            } else {
                // Provider not found, clear Claude model and env vars
                clear_claude_settings(&mut root);
            }
        }
        _ => {
            clear_claude_settings(&mut root);
        }
    }

    write_json(&settings_path, &root)?;
    Ok(())
}

fn clear_claude_env_vars(root: &mut Value) {
    if let Some(env) = root.get_mut("env").and_then(|v| v.as_object_mut()) {
        clear_claude_managed_env_vars(env);
    }
}

fn clear_claude_managed_env_vars(env: &mut serde_json::Map<String, Value>) {
    env.remove(ANTHROPIC_API_KEY_ENV);
    env.remove(ANTHROPIC_AUTH_TOKEN_ENV);
    env.remove(ANTHROPIC_BASE_URL_ENV);
    env.remove(ANTHROPIC_DEFAULT_HAIKU_MODEL_ENV);
    env.remove(ANTHROPIC_DEFAULT_OPUS_MODEL_ENV);
    env.remove(ANTHROPIC_DEFAULT_SONNET_MODEL_ENV);
}

fn write_claude_env_string(
    env: &mut serde_json::Map<String, Value>,
    key: &str,
    value: &str,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }

    env.insert(key.to_string(), Value::String(value.to_string()));
}

fn write_claude_default_model_env(
    env: &mut serde_json::Map<String, Value>,
    model: &str,
) {
    write_claude_env_string(env, ANTHROPIC_DEFAULT_HAIKU_MODEL_ENV, model);
    write_claude_env_string(env, ANTHROPIC_DEFAULT_OPUS_MODEL_ENV, model);
    write_claude_env_string(env, ANTHROPIC_DEFAULT_SONNET_MODEL_ENV, model);
}

fn clear_claude_settings(root: &mut Value) {
    clear_claude_env_vars(root);
    if let Some(map) = root.as_object_mut() {
        map.remove("model");
    }
}

fn read_or_new_json(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(default_claude_settings());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str::<Value>(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn ensure_claude_settings_shape(root: &mut Value) {
    if !root.is_object() {
        *root = default_claude_settings();
        return;
    }

    let map = root.as_object_mut().expect("checked object");
    map.entry("$schema".to_string())
        .or_insert_with(|| Value::String(CLAUDE_SETTINGS_SCHEMA.to_string()));
    if !map.get("env").is_some_and(Value::is_object) {
        map.insert("env".to_string(), Value::Object(serde_json::Map::new()));
    }
}

fn default_claude_settings() -> Value {
    let mut root = serde_json::Map::new();
    root.insert(
        "$schema".to_string(),
        Value::String(CLAUDE_SETTINGS_SCHEMA.to_string()),
    );
    root.insert("env".to_string(), Value::Object(serde_json::Map::new()));
    Value::Object(root)
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    backup_file_if_exists(path)?;

    let encoded = serde_json::to_string_pretty(value).context("failed to encode settings.json")?;
    fs::write(path, format!("{encoded}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    restrict_file_permissions(path)?;
    Ok(())
}

fn backup_file_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
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
    Ok(())
}

pub(crate) fn normalize_claude_base_url(input: &str) -> String {
    let mut url = input.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        return String::new();
    }

    for suffix in [
        "/v1/messages/count_tokens",
        "/v1/messages",
        "/v1/models",
        "/v1",
    ] {
        if url.ends_with(suffix) {
            let next_len = url.len() - suffix.len();
            url.truncate(next_len);
            return url.trim_end_matches('/').to_string();
        }
    }

    url
}

pub(crate) fn is_first_party_anthropic_base_url(base_url: &str) -> bool {
    let base_url = normalize_claude_base_url(base_url);
    if base_url.is_empty() {
        return true;
    }

    let without_scheme = base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = without_scheme
        .split('/')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(':');

    host == "api.anthropic.com"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_claude_base_urls_without_adding_v1() {
        assert_eq!(
            normalize_claude_base_url("https://api.anthropic.com"),
            "https://api.anthropic.com"
        );
        assert_eq!(
            normalize_claude_base_url("https://api.anthropic.com/v1"),
            "https://api.anthropic.com"
        );
        assert_eq!(
            normalize_claude_base_url("https://gateway.example.com/anthropic/v1/messages"),
            "https://gateway.example.com/anthropic"
        );
        assert_eq!(
            normalize_claude_base_url(" https://litellm.example.com:4000/ "),
            "https://litellm.example.com:4000"
        );
    }

    #[test]
    fn identifies_first_party_anthropic_base_url() {
        assert!(is_first_party_anthropic_base_url("https://api.anthropic.com/v1"));
        assert!(!is_first_party_anthropic_base_url("https://gateway.example.com"));
    }
}
