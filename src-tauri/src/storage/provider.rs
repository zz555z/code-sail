use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use super::crypto;
use super::{with_transaction, SqliteConnection, SqlValue, ToolType};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) models: Vec<String>,
    pub(crate) wire_api: String,
    pub(crate) requires_openai_auth: bool,
    pub(crate) token_present: bool,
    pub(crate) tool_type: ToolType,
    pub(crate) claude_haiku_model: Option<String>,
    pub(crate) claude_opus_model: Option<String>,
    pub(crate) claude_sonnet_model: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredProvider {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) base_url: String,
    pub(crate) model: Option<String>,
    pub(crate) wire_api: String,
    pub(crate) requires_open_ai_auth: bool,
    pub(crate) token: Option<String>,
    pub(crate) tool_type: ToolType,
    pub(crate) claude_haiku_model: Option<String>,
    pub(crate) claude_opus_model: Option<String>,
    pub(crate) claude_sonnet_model: Option<String>,
}

pub(crate) fn collect_providers_from_database(
    conn: &SqliteConnection,
    tool_type: ToolType,
) -> Result<Vec<ProviderView>> {
    let tool_type_str = tool_type.as_str();
    let provider_models = get_provider_models_for_tool(conn, tool_type)?;
    conn.query_all(
        r#"
        SELECT
            id,
            name,
            base_url,
            model,
            wire_api,
            requires_openai_auth,
            token IS NOT NULL AND length(trim(token)) > 0 AS token_present,
            tool_type,
            claude_haiku_model,
            claude_opus_model,
            claude_sonnet_model
        FROM providers
        WHERE tool_type = ?1
        ORDER BY position, lower(name), lower(id)
        "#,
        &[SqlValue::Text(tool_type_str)],
        |row| {
            let id = row.string(0)?;
            let tool_type_str = row.string(7)?;
            Ok(ProviderView {
                id: id.clone(),
                name: Some(row.string(1)?),
                base_url: Some(row.string(2)?),
                model: row.optional_string(3)?,
                models: provider_models.get(&id).cloned().unwrap_or_default(),
                wire_api: row.string(4)?,
                requires_openai_auth: row.i64(5)? != 0,
                token_present: row.i64(6)? != 0,
                tool_type: ToolType::from_str(&tool_type_str).unwrap_or_default(),
                claude_haiku_model: row.optional_string(8)?,
                claude_opus_model: row.optional_string(9)?,
                claude_sonnet_model: row.optional_string(10)?,
            })
        },
    )
}

pub(crate) fn list_stored_providers(
    conn: &SqliteConnection,
    config_path: &std::path::Path,
    tool_type: ToolType,
) -> Result<Vec<StoredProvider>> {
    let tool_type_str = tool_type.as_str();
    let mut providers = conn.query_all(
        r#"
        SELECT id, name, base_url, model, wire_api, requires_openai_auth, token, tool_type,
               claude_haiku_model, claude_opus_model, claude_sonnet_model
        FROM providers
        WHERE tool_type = ?1
        ORDER BY position, lower(name), lower(id)
        "#,
        &[SqlValue::Text(tool_type_str)],
        stored_provider_from_row,
    )?;
    for provider in &mut providers {
        provider.token = crypto::decrypt_optional_token(config_path, provider.token.as_deref())?;
    }
    Ok(providers)
}

pub(crate) fn get_provider(
    conn: &SqliteConnection,
    config_path: &std::path::Path,
    provider_id: &str,
) -> Result<Option<StoredProvider>> {
    let mut provider = conn.query_one(
        r#"
        SELECT id, name, base_url, model, wire_api, requires_openai_auth, token, tool_type,
               claude_haiku_model, claude_opus_model, claude_sonnet_model
        FROM providers
        WHERE id = ?1
        "#,
        &[SqlValue::Text(provider_id)],
        stored_provider_from_row,
    )?;
    if let Some(provider) = &mut provider {
        provider.token = crypto::decrypt_optional_token(config_path, provider.token.as_deref())?;
    }
    Ok(provider)
}

pub(crate) fn get_provider_token(
    conn: &SqliteConnection,
    config_path: &std::path::Path,
    provider_id: &str,
) -> Result<Option<String>> {
    conn.query_one(
        "SELECT token FROM providers WHERE id = ?1",
        &[SqlValue::Text(provider_id)],
        |row| row.optional_string(0),
    )
    .map(Option::flatten)
    .and_then(|token| crypto::decrypt_optional_token(config_path, token.as_deref()))
}

pub(crate) fn get_provider_models(conn: &SqliteConnection, provider_id: &str) -> Result<Vec<String>> {
    conn.query_all(
        r#"
        SELECT model
        FROM provider_models
        WHERE provider_id = ?1
        ORDER BY position, model
        "#,
        &[SqlValue::Text(provider_id)],
        |row| row.string(0),
    )
}

fn get_provider_models_for_tool(
    conn: &SqliteConnection,
    tool_type: ToolType,
) -> Result<HashMap<String, Vec<String>>> {
    let rows = conn.query_all(
        r#"
        SELECT pm.provider_id, pm.model
        FROM provider_models pm
        INNER JOIN providers p ON p.id = pm.provider_id
        WHERE p.tool_type = ?1
        ORDER BY pm.provider_id, pm.position, pm.model
        "#,
        &[SqlValue::Text(tool_type.as_str())],
        |row| Ok((row.string(0)?, row.string(1)?)),
    )?;
    let mut models = HashMap::<String, Vec<String>>::new();
    for (provider_id, model) in rows {
        models.entry(provider_id).or_default().push(model);
    }
    Ok(models)
}

pub(crate) fn replace_provider_models(
    conn: &SqliteConnection,
    provider_id: &str,
    models: &[String],
) -> Result<()> {
    conn.execute(
        "DELETE FROM provider_models WHERE provider_id = ?1",
        &[SqlValue::Text(provider_id)],
    )?;

    for (index, model) in models.iter().enumerate() {
        conn.execute(
            r#"
            INSERT INTO provider_models (provider_id, model, position)
            VALUES (?1, ?2, ?3)
            "#,
            &[
                SqlValue::Text(provider_id),
                SqlValue::Text(model),
                SqlValue::I64(index as i64),
            ],
        )?;
    }

    Ok(())
}

pub(crate) fn copy_provider_models(conn: &SqliteConnection, source_id: &str, target_id: &str) -> Result<()> {
    let models = get_provider_models(conn, source_id)?;
    replace_provider_models(conn, target_id, &models)
}

pub(crate) fn next_provider_position(conn: &SqliteConnection, tool_type: ToolType) -> Result<i64> {
    conn.query_one(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM providers WHERE tool_type = ?1",
        &[SqlValue::Text(tool_type.as_str())],
        |row| row.i64(0),
    )
    .map(|value| value.unwrap_or(0))
}

pub(crate) fn reorder_provider_positions(
    conn: &SqliteConnection,
    tool_type: ToolType,
    provider_ids: &[String],
) -> Result<()> {
    let tool_type_str = tool_type.as_str();
    let current_ids = provider_ids_for_tool(conn, tool_type_str)?;
    let current_set = current_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let requested_set = provider_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    if current_ids.len() != provider_ids.len() || current_set != requested_set {
        bail!("配置列表已变化，请刷新后再排序");
    }

    with_transaction(conn, |conn| {
        for (index, provider_id) in provider_ids.iter().enumerate() {
            conn.execute(
                "UPDATE providers SET position = ?1 WHERE id = ?2 AND tool_type = ?3",
                &[
                    SqlValue::I64(index as i64),
                    SqlValue::Text(provider_id),
                    SqlValue::Text(tool_type_str),
                ],
            )?;
        }
        Ok(())
    })
}

pub(crate) fn normalize_provider_positions(conn: &SqliteConnection) -> Result<()> {
    let tool_types = conn.query_all(
        "SELECT DISTINCT tool_type FROM providers ORDER BY tool_type",
        &[],
        |row| row.string(0),
    )?;

    for tool_type in tool_types {
        let provider_ids = provider_ids_for_tool(conn, &tool_type)?;
        for (index, provider_id) in provider_ids.iter().enumerate() {
            conn.execute(
                "UPDATE providers SET position = ?1 WHERE id = ?2 AND tool_type = ?3",
                &[
                    SqlValue::I64(index as i64),
                    SqlValue::Text(provider_id),
                    SqlValue::Text(&tool_type),
                ],
            )?;
        }
    }

    Ok(())
}

fn provider_ids_for_tool(conn: &SqliteConnection, tool_type: &str) -> Result<Vec<String>> {
    conn.query_all(
        r#"
        SELECT id
        FROM providers
        WHERE tool_type = ?1
        ORDER BY position, lower(name), lower(id)
        "#,
        &[SqlValue::Text(tool_type)],
        |row| row.string(0),
    )
}

pub(crate) fn next_provider_id(
    conn: &SqliteConnection,
    name: &str,
    base_url: &str,
    skip_id: Option<&str>,
) -> Result<String> {
    let base = slugify_provider_id(name)
        .or_else(|| infer_provider_id_from_url(base_url))
        .unwrap_or_else(|| "provider".to_string());
    let mut candidate = base.clone();
    let mut index = 2;
    loop {
        if Some(candidate.as_str()) == skip_id || !provider_exists(conn, &candidate)? {
            return Ok(candidate);
        }
        candidate = format!("{base}-{index}");
        index += 1;
    }
}

fn slugify_provider_id(input: &str) -> Option<String> {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for character in input.trim().chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() || character == '_' {
            slug.push(character);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

fn infer_provider_id_from_url(base_url: &str) -> Option<String> {
    let without_scheme = base_url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = without_scheme
        .split('/')
        .next()
        .unwrap_or_default()
        .trim_start_matches("www.");
    let parts = host
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let candidate = if parts.len() > 1 {
        parts.get(parts.len() - 2).copied()
    } else {
        parts.first().copied()
    };
    candidate.and_then(slugify_provider_id)
}

pub(crate) fn optional_non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn stored_provider_from_row(row: &super::SqliteRow<'_>) -> Result<StoredProvider> {
    let tool_type_str = row.string(7)?;
    Ok(StoredProvider {
        id: row.string(0)?,
        name: row.string(1)?,
        base_url: row.string(2)?,
        model: row.optional_string(3)?,
        wire_api: row.string(4)?,
        requires_open_ai_auth: row.i64(5)? != 0,
        token: row.optional_string(6)?,
        tool_type: ToolType::from_str(&tool_type_str).unwrap_or_default(),
        claude_haiku_model: row.optional_string(8)?,
        claude_opus_model: row.optional_string(9)?,
        claude_sonnet_model: row.optional_string(10)?,
    })
}

pub(crate) fn next_copy_id(conn: &SqliteConnection, source_id: &str) -> Result<String> {
    let base = format!("{source_id}-copy");
    let mut candidate = base.clone();
    let mut index = 2;

    while provider_exists(conn, &candidate)? {
        candidate = format!("{base}-{index}");
        index += 1;
    }

    Ok(candidate)
}

pub(crate) fn next_copy_name(conn: &SqliteConnection, source_name: &str) -> Result<String> {
    let base = strip_copy_suffix(source_name.trim()).trim();
    let base = if base.is_empty() { "provider" } else { base };
    let mut index = 1;

    loop {
        let candidate = format!("{base} copy{index}");
        if !provider_name_exists(conn, &candidate)? {
            return Ok(candidate);
        }
        index += 1;
    }
}

fn strip_copy_suffix(name: &str) -> &str {
    let Some((prefix, suffix)) = name.rsplit_once(" copy") else {
        return name;
    };

    if !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit()) {
        prefix
    } else {
        name
    }
}

fn provider_name_exists(conn: &SqliteConnection, provider_name: &str) -> Result<bool> {
    let count = conn
        .query_one(
            "SELECT COUNT(*) FROM providers WHERE lower(name) = lower(?1)",
            &[SqlValue::Text(provider_name)],
            |row| row.i64(0),
        )?
        .unwrap_or(0);
    Ok(count > 0)
}

pub(crate) fn provider_exists(conn: &SqliteConnection, provider_id: &str) -> Result<bool> {
    let count = conn
        .query_one(
            "SELECT COUNT(*) FROM providers WHERE id = ?1",
            &[SqlValue::Text(provider_id)],
            |row| row.i64(0),
        )?
        .unwrap_or(0);
    Ok(count > 0)
}
