use anyhow::{bail, Context, Result};
use reqwest::Url;
use serde_json::Value;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};
use toml_edit::{value, DocumentMut, Item, Table};

use crate::file_util;
use crate::storage::{
    get_active_model, get_active_provider, get_provider_token, get_setting,
    list_stored_providers, optional_non_empty, set_active_model,
    set_active_provider, set_setting, with_transaction, SqliteConnection, SqlValue, ToolType,
};

const CODEX_CONFIG_IMPORTED_SETTING: &str = "codex_config_imported_v1";

#[derive(Debug)]
struct ImportedCodexProvider {
    id: String,
    name: String,
    base_url: String,
    wire_api: String,
    requires_openai_auth: bool,
    model: Option<String>,
}

#[derive(Debug)]
struct ImportedCodexConfig {
    active_provider: Option<String>,
    active_model: Option<String>,
    providers: Vec<ImportedCodexProvider>,
}

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

pub(crate) fn import_codex_config_if_needed(
    conn: &SqliteConnection,
    config_path: &Path,
) -> Result<()> {
    if get_setting(conn, CODEX_CONFIG_IMPORTED_SETTING)?.as_deref() == Some("1") {
        return Ok(());
    }

    if !list_stored_providers(conn, config_path, ToolType::Codex)?.is_empty() {
        set_setting(conn, CODEX_CONFIG_IMPORTED_SETTING, "1")?;
        return Ok(());
    }

    if !config_path.exists() {
        return Ok(());
    }

    let imported = read_codex_config_for_import(config_path)?;
    with_transaction(conn, |conn| {
        for (position, provider) in imported.providers.iter().enumerate() {
            conn.execute(
                r#"
                INSERT INTO providers (
                    id, name, base_url, model, wire_api, requires_openai_auth, tool_type, position
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(id) DO NOTHING
                "#,
                &[
                    SqlValue::Text(&provider.id),
                    SqlValue::Text(&provider.name),
                    SqlValue::Text(&provider.base_url),
                    SqlValue::OptionalText(provider.model.as_deref()),
                    SqlValue::Text(&provider.wire_api),
                    SqlValue::I64(if provider.requires_openai_auth { 1 } else { 0 }),
                    SqlValue::Text(ToolType::Codex.as_str()),
                    SqlValue::I64(position as i64),
                ],
            )?;
        }

        if let Some(active_provider) = imported
            .active_provider
            .as_deref()
            .filter(|provider_id| {
                imported
                    .providers
                    .iter()
                    .any(|provider| provider.id == *provider_id)
            })
        {
            set_active_provider(conn, ToolType::Codex, active_provider)?;
        }
        if let Some(active_model) = imported.active_model.as_deref() {
            set_active_model(conn, ToolType::Codex, active_model)?;
        }

        set_setting(conn, CODEX_CONFIG_IMPORTED_SETTING, "1")?;
        Ok(())
    })?;

    if !imported.providers.is_empty() {
        log::info!(
            "imported {} providers from {}",
            imported.providers.len(),
            config_path.display()
        );
    }
    Ok(())
}

pub(crate) fn sync_codex_files(conn: &SqliteConnection, config_path: &Path) -> Result<()> {
    let providers = list_stored_providers(conn, config_path, ToolType::Codex)?;
    let active_provider = get_active_provider(conn, ToolType::Codex)?;
    let active_model = get_active_model(conn, ToolType::Codex)?;
    let mut document = read_or_new_config(config_path)?;

    file_util::backup_file_if_exists(config_path)?;
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

fn read_codex_config_for_import(config_path: &Path) -> Result<ImportedCodexConfig> {
    let document = read_config(config_path)?;
    let active_provider = string_item(document.get("model_provider"));
    let active_model = string_item(document.get("model"));
    let providers = document
        .get("model_providers")
        .and_then(Item::as_table)
        .map(|table| {
            table
                .iter()
                .filter_map(|(id, item)| {
                    imported_provider_from_item(
                        id,
                        item,
                        active_provider.as_deref(),
                        active_model.as_deref(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(ImportedCodexConfig {
        active_provider,
        active_model,
        providers,
    })
}

fn imported_provider_from_item(
    id: &str,
    item: &Item,
    active_provider: Option<&str>,
    active_model: Option<&str>,
) -> Option<ImportedCodexProvider> {
    let table = item.as_table()?;
    let base_url = string_item(table.get("base_url"))?;
    if optional_non_empty(&base_url).is_none() {
        return None;
    }

    let name = string_item(table.get("name"))
        .and_then(|value| optional_non_empty(&value).map(ToString::to_string))
        .unwrap_or_else(|| id.to_string());
    let wire_api = string_item(table.get("wire_api"))
        .and_then(|value| optional_non_empty(&value).map(ToString::to_string))
        .unwrap_or_else(|| "responses".to_string());
    let requires_openai_auth = table
        .get("requires_openai_auth")
        .and_then(Item::as_value)
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let model = active_provider
        .filter(|provider_id| *provider_id == id)
        .and(active_model)
        .and_then(|value| optional_non_empty(value).map(ToString::to_string));

    Some(ImportedCodexProvider {
        id: id.to_string(),
        name,
        base_url,
        wire_api,
        requires_openai_auth,
        model,
    })
}

fn string_item(item: Option<&Item>) -> Option<String> {
    item.and_then(Item::as_value)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
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
    file_util::atomic_write_restricted(config_path, &document.to_string())?;
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

pub(crate) fn normalize_model_list_base_url(input: &str) -> String {
    let mut url = input.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        return String::new();
    }

    if let Some(origin) = url_origin(&url) {
        return format!("{origin}/v1");
    }

    for suffix in [
        "/v1/messages/count_tokens",
        "/v1/messages",
        "/v1/models",
        "/models",
    ] {
        if url.ends_with(suffix) {
            let next_len = url.len() - suffix.len();
            url.truncate(next_len);
            break;
        }
    }

    normalize_base_url(&url)
}

fn url_origin(input: &str) -> Option<String> {
    let parsed = Url::parse(input).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }

    let host = parsed.host_str()?;
    let port = parsed.port().map(|port| format!(":{port}")).unwrap_or_default();
    Some(format!("{}://{}{}", parsed.scheme(), host, port))
}

pub(crate) fn parse_model_ids(body: &Value) -> Vec<String> {
    let mut models = Vec::new();
    let mut seen = HashSet::new();
    collect_standard_model_ids(body, &mut models, &mut seen);
    if !models.is_empty() {
        return models;
    }

    collect_model_ids(body, &mut models, &mut seen);
    models
}

fn collect_standard_model_ids(
    value: &Value,
    models: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    let Some(data) = value.get("data").and_then(Value::as_array) else {
        return;
    };

    for item in data {
        if let Some(id) = item
            .as_str()
            .or_else(|| item.get("id").and_then(Value::as_str))
        {
            push_model_id(id, models, seen);
        }
    }
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
    use crate::storage::{
        collect_providers_from_database, get_active_model, get_active_provider, get_setting,
        open_database,
    };
    use serde_json::json;
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_config_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir()
            .join(format!("codesail-codex-config-{name}-{unique}"))
            .join("config.toml")
    }

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
    fn normalizes_model_list_base_urls_for_shared_fetch() {
        assert_eq!(
            normalize_model_list_base_url("https://api.example.com"),
            "https://api.example.com/v1"
        );
        assert_eq!(
            normalize_model_list_base_url("https://api.example.com/v1"),
            "https://api.example.com/v1"
        );
        assert_eq!(
            normalize_model_list_base_url("https://api.example.com/v1/models"),
            "https://api.example.com/v1"
        );
        assert_eq!(
            normalize_model_list_base_url("https://api.example.com/v1/messages"),
            "https://api.example.com/v1"
        );
        assert_eq!(
            normalize_model_list_base_url("https://api.example.com/v1/messages/count_tokens"),
            "https://api.example.com/v1"
        );
        assert_eq!(
            normalize_model_list_base_url("https://api.example.com/custom/path?token=abc"),
            "https://api.example.com/v1"
        );
        assert_eq!(
            normalize_model_list_base_url("https://api.example.com:8443/anything/v1/models"),
            "https://api.example.com:8443/v1"
        );
    }

    #[test]
    fn parses_standard_model_ids_before_fallback() {
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
                "gpt-5-mini".to_string()
            ]
        );
    }

    #[test]
    fn falls_back_to_nested_model_ids_when_data_is_missing() {
        let body = json!({
            "models": [
                { "id": "custom-1" },
                { "nested": [{ "id": "custom-2" }] }
            ]
        });

        assert_eq!(
            parse_model_ids(&body),
            vec!["custom-1".to_string(), "custom-2".to_string()]
        );
    }

    #[test]
    fn imports_existing_codex_model_providers_once() -> Result<()> {
        let config_path = temp_config_path("import-existing");
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &config_path,
            r#"
model_provider = "openai"
model = "gpt-5"

[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
wire_api = "responses"
requires_openai_auth = true

[model_providers.local]
name = "Local"
base_url = "http://localhost:11434/v1"
requires_openai_auth = false
"#,
        )?;

        let conn = open_database(&config_path)?;
        import_codex_config_if_needed(&conn, &config_path)?;
        import_codex_config_if_needed(&conn, &config_path)?;

        let providers = collect_providers_from_database(&conn, ToolType::Codex)?;
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].id, "openai");
        assert_eq!(providers[0].model.as_deref(), Some("gpt-5"));
        assert_eq!(providers[1].id, "local");
        assert_eq!(
            get_active_provider(&conn, ToolType::Codex)?.as_deref(),
            Some("openai")
        );
        assert_eq!(get_active_model(&conn, ToolType::Codex)?.as_deref(), Some("gpt-5"));
        assert_eq!(
            get_setting(&conn, CODEX_CONFIG_IMPORTED_SETTING)?.as_deref(),
            Some("1")
        );

        if let Some(parent) = config_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
        Ok(())
    }
}

fn write_auth_token(config_path: &Path, token: &str) -> Result<()> {
    let auth_path = auth_path(config_path)?;
    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    file_util::backup_file_if_exists(&auth_path)?;

    let mut root = if auth_path.exists() {
        let raw = fs::read_to_string(&auth_path)
            .with_context(|| format!("failed to read {}", auth_path.display()))?;
        serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    root["OPENAI_API_KEY"] = Value::String(token.to_string());
    let encoded = serde_json::to_string_pretty(&root).context("failed to encode auth.json")?;
    file_util::atomic_write_restricted(&auth_path, &format!("{encoded}\n"))?;
    Ok(())
}

fn clear_auth_token(config_path: &Path) -> Result<()> {
    let auth_path = auth_path(config_path)?;
    if !auth_path.exists() {
        return Ok(());
    }

    file_util::backup_file_if_exists(&auth_path)?;
    let raw = fs::read_to_string(&auth_path)
        .with_context(|| format!("failed to read {}", auth_path.display()))?;
    let mut root = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = root.as_object_mut() {
        object.remove("OPENAI_API_KEY");
    }

    let encoded = serde_json::to_string_pretty(&root).context("failed to encode auth.json")?;
    file_util::atomic_write_restricted(&auth_path, &format!("{encoded}\n"))?;
    Ok(())
}
