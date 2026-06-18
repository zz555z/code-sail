use anyhow::{bail, Context, Result};
use chrono::Local;
use serde_json::Value;
use std::{collections::HashSet, env, fs, path::{Path, PathBuf}};
use toml_edit::{value, DocumentMut, Item, Table};

use crate::storage::{
    get_active_model, get_active_provider, get_provider_token, list_stored_providers,
    restrict_file_permissions, SqliteConnection, ToolType,
};

pub(crate) struct ResolvedToken {
    pub(crate) value: String,
}

pub(crate) fn codex_config_path() -> Result<PathBuf> {
    if let Ok(path) = env::var("CODEX_CONFIG") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = dirs::home_dir().context("failed to locate the home directory")?;
    Ok(home.join(".codex").join("config.toml"))
}

pub(crate) fn sync_codex_files(conn: &SqliteConnection, config_path: &Path) -> Result<()> {
    let providers = list_stored_providers(conn, config_path, ToolType::Codex)?;
    let active_provider = get_active_provider(conn, ToolType::Codex)?;
    let active_model = get_active_model(conn, ToolType::Codex)?;
    let mut document = read_or_new_config(config_path)?;

    backup_file_if_exists(config_path)?;
    document.as_table_mut().remove("model_providers");
    document["model_providers"] = Item::Table(Table::new());

    for provider in &providers {
        upsert_provider_in_document(
            &mut document,
            &provider.id,
            &provider.name,
            &provider.base_url,
            &provider.wire_api,
            provider.requires_open_ai_auth,
        )?;
    }

    match active_provider.as_deref() {
        Some(provider_id) if providers.iter().any(|provider| provider.id == provider_id) => {
            document["model_provider"] = value(provider_id);
            let provider_model = providers
                .iter()
                .find(|provider| provider.id == provider_id)
                .and_then(|provider| provider.model.as_deref());
            let model = active_model
                .as_deref()
                .filter(|model| !model.is_empty())
                .or_else(|| provider_model.filter(|model| !model.is_empty()));
            if let Some(model) = model {
                document["model"] = value(model);
            } else {
                document.as_table_mut().remove("model");
            }
        }
        _ => {
            document.as_table_mut().remove("model_provider");
            document.as_table_mut().remove("model");
        }
    }

    write_config(config_path, &document)?;

    if let Some(provider_id) = active_provider {
        if let Some(token) =
            get_provider_token(conn, config_path, &provider_id)?
                .filter(|token| !token.trim().is_empty())
        {
            write_auth_token(config_path, &token)?;
            return Ok(());
        }
    }

    clear_auth_token(config_path)?;
    Ok(())
}

fn read_or_new_config(config_path: &Path) -> Result<DocumentMut> {
    if config_path.exists() {
        read_config(config_path)
    } else {
        Ok(DocumentMut::new())
    }
}

fn read_config(config_path: &Path) -> Result<DocumentMut> {
    let raw = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    raw.parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", config_path.display()))
}

fn write_config(config_path: &Path, document: &DocumentMut) -> Result<()> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(config_path, document.to_string())
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    restrict_file_permissions(config_path)?;
    Ok(())
}

fn upsert_provider_in_document(
    document: &mut DocumentMut,
    id: &str,
    name: &str,
    base_url: &str,
    wire_api: &str,
    requires_open_ai_auth: bool,
) -> Result<()> {
    if !document.as_table().contains_key("model_providers") {
        document["model_providers"] = Item::Table(Table::new());
    }

    let providers = document["model_providers"]
        .as_table_mut()
        .context("model_providers 不是 TOML table")?;

    if providers.get(id).is_none() {
        providers.insert(id, Item::Table(Table::new()));
    }

    let provider = providers
        .get_mut(id)
        .and_then(Item::as_table_mut)
        .context("provider 配置不是 TOML table")?;

    provider.insert("name", value(name));
    provider.insert("base_url", value(base_url));
    provider.insert("wire_api", value(wire_api));
    provider.insert("requires_openai_auth", value(requires_open_ai_auth));

    Ok(())
}

fn backup_file_if_exists(path: &Path) -> Result<Option<PathBuf>> {
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

pub(crate) fn normalize_base_url(input: &str) -> String {
    let url = input.trim().trim_end_matches('/');
    if url.is_empty() {
        return String::new();
    }
    if url.ends_with("/v1") {
        url.to_string()
    } else if url.ends_with("/v1/models") {
        url.trim_end_matches("/models").to_string()
    } else {
        format!("{url}/v1")
    }
}

pub(crate) fn parse_model_ids(body: &Value) -> Vec<String> {
    let mut models = Vec::new();
    let mut seen = HashSet::new();
    collect_model_ids(body, &mut models, &mut seen);
    models
}

fn collect_model_ids(value: &Value, models: &mut Vec<String>, seen: &mut HashSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(id) = item.as_str() {
                    push_model_id(id, models, seen);
                } else {
                    collect_model_ids(item, models, seen);
                }
            }
        }
        Value::Object(map) => {
            if let Some(id) = map.get("id").and_then(Value::as_str) {
                push_model_id(id, models, seen);
            }

            for item in map.values() {
                if !item.is_string() {
                    collect_model_ids(item, models, seen);
                }
            }
        }
        _ => {}
    }
}

fn push_model_id(id: &str, models: &mut Vec<String>, seen: &mut HashSet<String>) {
    let id = id.trim();
    if !id.is_empty() && seen.insert(id.to_string()) {
        models.push(id.to_string());
    }
}

pub(crate) fn resolve_token_for_request(
    conn: &SqliteConnection,
    config_path: &Path,
    provider_id: &str,
    token_override: Option<&str>,
) -> Result<ResolvedToken> {
    if let Some(token) = token_override.map(str::trim).filter(|token| !token.is_empty()) {
        return Ok(ResolvedToken {
            value: token.to_string(),
        });
    }

    if !provider_id.is_empty() {
        if let Some(token) =
            get_provider_token(conn, config_path, provider_id)?
                .filter(|token| !token.trim().is_empty())
        {
            return Ok(ResolvedToken { value: token });
        }
    }

    bail!("没有可用 token，请先在 Token 输入框填入 token 并保存")
}

fn auth_path(config_path: &Path) -> Result<PathBuf> {
    let config_dir = config_path
        .parent()
        .context("failed to locate config directory")?;
    Ok(config_dir.join("auth.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_base_urls_for_codex_model_provider_config() {
        assert_eq!(normalize_base_url("https://api.example.com"), "https://api.example.com/v1");
        assert_eq!(normalize_base_url("https://api.example.com/v1"), "https://api.example.com/v1");
        assert_eq!(
            normalize_base_url("https://api.example.com/v1/models"),
            "https://api.example.com/v1"
        );
        assert_eq!(normalize_base_url("  https://api.example.com/  "), "https://api.example.com/v1");
        assert_eq!(normalize_base_url(""), "");
    }

    #[test]
    fn parses_unique_model_ids_from_nested_responses() {
        let body = json!({
            "data": [
                { "id": "gpt-5" },
                { "id": "gpt-5" },
                { "id": "gpt-5-mini" },
                { "nested": [{ "id": "o4-mini" }] }
            ],
            "metadata": { "id": "ignored-but-valid" }
        });

        assert_eq!(
            parse_model_ids(&body),
            vec![
                "gpt-5".to_string(),
                "gpt-5-mini".to_string(),
                "o4-mini".to_string(),
                "ignored-but-valid".to_string()
            ]
        );
    }
}

fn write_auth_token(config_path: &Path, token: &str) -> Result<()> {
    let auth_path = auth_path(config_path)?;
    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    backup_file_if_exists(&auth_path)?;

    let mut root = if auth_path.exists() {
        let raw = fs::read_to_string(&auth_path)
            .with_context(|| format!("failed to read {}", auth_path.display()))?;
        serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    root["OPENAI_API_KEY"] = Value::String(token.to_string());
    let encoded = serde_json::to_string_pretty(&root).context("failed to encode auth.json")?;
    fs::write(&auth_path, format!("{encoded}\n"))
        .with_context(|| format!("failed to write {}", auth_path.display()))?;
    restrict_file_permissions(&auth_path)?;
    Ok(())
}

fn clear_auth_token(config_path: &Path) -> Result<()> {
    let auth_path = auth_path(config_path)?;
    if !auth_path.exists() {
        return Ok(());
    }

    backup_file_if_exists(&auth_path)?;
    let raw = fs::read_to_string(&auth_path)
        .with_context(|| format!("failed to read {}", auth_path.display()))?;
    let mut root = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = root.as_object_mut() {
        object.remove("OPENAI_API_KEY");
    }

    let encoded = serde_json::to_string_pretty(&root).context("failed to encode auth.json")?;
    fs::write(&auth_path, format!("{encoded}\n"))
        .with_context(|| format!("failed to write {}", auth_path.display()))?;
    restrict_file_permissions(&auth_path)?;
    Ok(())
}
