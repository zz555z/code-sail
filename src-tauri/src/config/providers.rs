use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::claude_config::sync_claude_settings;
use crate::codex_config::{codex_config_path, normalize_base_url, sync_codex_files};
use crate::storage::{
    bool_to_i64, copy_provider_models, delete_active_settings, encrypt_optional_token,
    get_active_provider, get_active_tool, get_provider, get_provider_models, get_provider_token,
    list_stored_providers, next_copy_id, next_copy_name, next_provider_id,
    next_provider_position, open_database, optional_non_empty, provider_exists,
    provider_belongs_to_tool, reorder_provider_positions, set_active_model, set_active_provider,
    with_transaction, SqliteConnection, SqlValue, ToolType,
};
use crate::tasks::run_background_task;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    pub original_id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub token: Option<String>,
    pub update_config: bool,
    pub tool_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderResponse {
    pub provider_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDetail {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: Option<String>,
    pub models: Vec<String>,
    pub token: Option<String>,
    pub token_present: bool,
    pub tool_type: ToolType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyProviderResponse {
    pub provider_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProvidersResponse {
    pub imported_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetCurrentModelRequest {
    pub provider_id: String,
    pub model: String,
    pub token: Option<String>,
    pub update_config: bool,
}

#[tauri::command]
pub async fn save_provider(input: ProviderInput) -> Result<SaveProviderResponse, String> {
    run_background_task("codex-save-provider", move || save_provider_inner(input))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_provider_detail(provider_id: String) -> Result<ProviderDetail, String> {
    run_background_task("codex-get-provider-detail", move || {
        get_provider_detail_inner(&provider_id)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn copy_provider(provider_id: String) -> Result<CopyProviderResponse, String> {
    run_background_task("codex-copy-provider", move || copy_provider_inner(&provider_id))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn import_codex_providers_to_claude() -> Result<ImportProvidersResponse, String> {
    run_background_task("claude-import-codex-providers", import_codex_providers_to_claude_inner)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn delete_provider(provider_id: String) -> Result<(), String> {
    run_background_task("codex-delete-provider", move || delete_provider_inner(&provider_id))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn reorder_providers(provider_ids: Vec<String>) -> Result<(), String> {
    run_background_task("codex-reorder-providers", move || reorder_providers_inner(provider_ids))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_current_model(input: SetCurrentModelRequest) -> Result<(), String> {
    run_background_task("codex-set-current-model", move || set_current_model_inner(input))
        .await
        .map_err(|error| error.to_string())
}

fn save_provider_inner(input: ProviderInput) -> Result<SaveProviderResponse> {
    let name = input.name.trim();
    let base_url = input.base_url.trim();
    let model = input.model.trim();
    let tool_type = match input.tool_type.as_deref() {
        Some(s) => ToolType::from_str(s)
            .with_context(|| format!("无效的 tool type: {s}"))?,
        None => ToolType::default(),
    };

    if base_url.is_empty() {
        bail!("Base URL 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let original_id = input.original_id.as_deref().map(str::trim).unwrap_or_default();
    let updates_existing_provider =
        !original_id.is_empty() && provider_belongs_to_tool(&conn, original_id, tool_type)?;
    let provider_id = if updates_existing_provider {
        original_id.to_string()
    } else {
        next_provider_id(&conn, name, base_url, None)?
    };
    let explicit_token = input
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string);
    let existing_token = if explicit_token.is_none() {
        if updates_existing_provider {
            get_provider_token(&conn, &config_path, original_id)?
        } else {
            get_provider_token(&conn, &config_path, &provider_id)?
        }
    } else {
        None
    };
    let token = explicit_token.or(existing_token);
    let stored_token = encrypt_optional_token(&config_path, token.as_deref())?;
    let normalized_base_url = match tool_type {
        ToolType::Claude => base_url.to_string(),
        ToolType::Codex => normalize_base_url(base_url),
    };
    let provider_name = if name.is_empty() { provider_id.as_str() } else { name };
    let tool_type_str = tool_type.as_str();
    let position = if updates_existing_provider {
        0
    } else {
        next_provider_position(&conn, tool_type)?
    };

    with_transaction(&conn, |conn| {
        if !original_id.is_empty() && original_id != provider_id {
            conn.execute("DELETE FROM providers WHERE id = ?1", &[SqlValue::Text(original_id)])?;
            if get_active_provider(conn, tool_type)?.as_deref() == Some(original_id) {
                set_active_provider(conn, tool_type, &provider_id)?;
            }
        }

        conn.execute(
            r#"
            INSERT INTO providers (id, name, base_url, model, token, tool_type, position)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                base_url = excluded.base_url,
                model = excluded.model,
                token = excluded.token
            "#,
            &[
                SqlValue::Text(&provider_id),
                SqlValue::Text(provider_name),
                SqlValue::Text(&normalized_base_url),
                SqlValue::OptionalText(optional_non_empty(model)),
                SqlValue::OptionalText(stored_token.as_deref()),
                SqlValue::Text(tool_type_str),
                SqlValue::I64(position),
            ],
        )?;
        if input.update_config {
            set_active_provider(conn, tool_type, &provider_id)?;
            if let Some(model) = optional_non_empty(model) {
                set_active_model(conn, tool_type, model)?;
            }
        }
        Ok(())
    })?;

    if input.update_config {
        sync_tool_config(&conn, &config_path, tool_type)?;
    }
    Ok(SaveProviderResponse {
        provider_id,
    })
}

fn get_provider_detail_inner(provider_id: &str) -> Result<ProviderDetail> {
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        bail!("Provider ID 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let provider = get_provider(&conn, &config_path, provider_id)?
        .with_context(|| format!("未找到 provider: {provider_id}"))?;
    let models = get_provider_models(&conn, provider_id)?;
    let token_present = provider
        .token
        .as_deref()
        .is_some_and(|token| !token.trim().is_empty());

    Ok(ProviderDetail {
        id: provider.id,
        name: provider.name,
        base_url: provider.base_url,
        model: provider.model,
        models,
        token: provider.token,
        token_present,
        tool_type: provider.tool_type,
    })
}

fn delete_provider_inner(provider_id: &str) -> Result<()> {
    let id = provider_id.trim();
    if id.is_empty() {
        bail!("Provider ID 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;

    // Look up tool_type before deleting
    let tool_type = get_provider(&conn, &config_path, id)?
        .map(|p| p.tool_type)
        .unwrap_or_default();

    with_transaction(&conn, |conn| {
        conn.execute(
            "DELETE FROM provider_models WHERE provider_id = ?1",
            &[SqlValue::Text(id)],
        )?;
        conn.execute("DELETE FROM providers WHERE id = ?1", &[SqlValue::Text(id)])?;
        if get_active_provider(conn, tool_type)?.as_deref() == Some(id) {
            delete_active_settings(conn, tool_type)?;
        }
        Ok(())
    })?;

    sync_tool_config(&conn, &config_path, tool_type)?;
    Ok(())
}

fn reorder_providers_inner(provider_ids: Vec<String>) -> Result<()> {
    let provider_ids = provider_ids
        .into_iter()
        .map(|id| id.trim().to_string())
        .collect::<Vec<_>>();

    if provider_ids.iter().any(|id| id.is_empty()) {
        bail!("配置列表包含空 provider ID");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let active_tool = get_active_tool(&conn)?;
    reorder_provider_positions(&conn, active_tool, &provider_ids)
}

fn copy_provider_inner(provider_id: &str) -> Result<CopyProviderResponse> {
    let source_id = provider_id.trim();
    if source_id.is_empty() {
        bail!("Provider ID 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let source = get_provider(&conn, &config_path, source_id)?
        .with_context(|| format!("未找到 provider: {source_id}"))?;
    let copy_id = next_copy_id(&conn, source_id)?;
    let copy_name = next_copy_name(&conn, &source.name)?;
    let copied_token = encrypt_optional_token(&config_path, source.token.as_deref())?;
    let tool_type_str = source.tool_type.as_str();

    with_transaction(&conn, |conn| {
        let position = next_provider_position(conn, source.tool_type)?;

        conn.execute(
            r#"
            INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token, tool_type, position)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            &[
                SqlValue::Text(&copy_id),
                SqlValue::Text(&copy_name),
                SqlValue::Text(&source.base_url),
                SqlValue::OptionalText(source.model.as_deref()),
                SqlValue::Text(&source.wire_api),
                SqlValue::I64(bool_to_i64(source.requires_open_ai_auth)),
                SqlValue::OptionalText(copied_token.as_deref()),
                SqlValue::Text(tool_type_str),
                SqlValue::I64(position),
            ],
        )?;
        copy_provider_models(conn, source_id, &copy_id)?;
        Ok(())
    })?;

    sync_tool_config(&conn, &config_path, source.tool_type)?;
    Ok(CopyProviderResponse {
        provider_id: copy_id,
    })
}

fn import_codex_providers_to_claude_inner() -> Result<ImportProvidersResponse> {
    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let existing_claude = list_stored_providers(&conn, &config_path, ToolType::Claude)?;
    if !existing_claude.is_empty() {
        return Ok(ImportProvidersResponse { imported_count: 0 });
    }

    let codex_providers = list_stored_providers(&conn, &config_path, ToolType::Codex)?;
    if codex_providers.is_empty() {
        return Ok(ImportProvidersResponse { imported_count: 0 });
    }

    let mut copied_pairs = Vec::with_capacity(codex_providers.len());
    let mut next_position = next_provider_position(&conn, ToolType::Claude)?;

    with_transaction(&conn, |conn| {
        for source in &codex_providers {
            let target_id = next_imported_claude_provider_id(conn, &source.id)?;
            let copied_token = encrypt_optional_token(&config_path, source.token.as_deref())?;

            conn.execute(
                r#"
                INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token, tool_type, position)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                &[
                    SqlValue::Text(&target_id),
                    SqlValue::Text(&source.name),
                    SqlValue::Text(&source.base_url),
                    SqlValue::OptionalText(source.model.as_deref()),
                    SqlValue::Text(&source.wire_api),
                    SqlValue::I64(bool_to_i64(source.requires_open_ai_auth)),
                    SqlValue::OptionalText(copied_token.as_deref()),
                    SqlValue::Text(ToolType::Claude.as_str()),
                    SqlValue::I64(next_position),
                ],
            )?;
            next_position += 1;
            copied_pairs.push((source.id.clone(), target_id));
            if let Some((source_id, target_id)) = copied_pairs.last() {
                copy_provider_models(conn, source_id, target_id)?;
            }
        }

        if let Some((_, first_target_id)) = copied_pairs.first() {
            set_active_provider(conn, ToolType::Claude, first_target_id)?;
            if let Some(model) = codex_providers
                .first()
                .and_then(|provider| provider.model.as_deref())
                .filter(|model| !model.trim().is_empty())
            {
                set_active_model(conn, ToolType::Claude, model)?;
            }
        }

        Ok(())
    })?;

    Ok(ImportProvidersResponse {
        imported_count: copied_pairs.len(),
    })
}

fn next_imported_claude_provider_id(conn: &SqliteConnection, source_id: &str) -> Result<String> {
    let base = format!("{}-claude", source_id.trim().trim_end_matches("-claude"));
    let mut candidate = base.clone();
    let mut index = 2;

    while provider_exists(conn, &candidate)? {
        candidate = format!("{base}-{index}");
        index += 1;
    }

    Ok(candidate)
}

fn set_current_model_inner(input: SetCurrentModelRequest) -> Result<()> {
    let provider_id = input.provider_id.trim();
    let model = input.model.trim();
    if provider_id.is_empty() {
        bail!("Provider ID 不能为空");
    }
    if model.is_empty() {
        bail!("Model 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let provider = get_provider(&conn, &config_path, provider_id)?
        .with_context(|| format!("未找到 provider: {provider_id}"))?;
    let tool_type = provider.tool_type;
    let token = input
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
        .or(provider.token);
    let stored_token = encrypt_optional_token(&config_path, token.as_deref())?;

    with_transaction(&conn, |conn| {
        if let Some(token) = stored_token.as_deref() {
            conn.execute(
                "UPDATE providers SET token = ?1, model = ?2 WHERE id = ?3",
                &[
                    SqlValue::Text(token),
                    SqlValue::Text(model),
                    SqlValue::Text(provider_id),
                ],
            )?;
        } else {
            conn.execute(
                "UPDATE providers SET model = ?1 WHERE id = ?2",
                &[SqlValue::Text(model), SqlValue::Text(provider_id)],
            )?;
        }
        set_active_provider(conn, tool_type, provider_id)?;
        set_active_model(conn, tool_type, model)?;
        Ok(())
    })?;

    if input.update_config {
        sync_tool_config(&conn, &config_path, tool_type)?;
    }
    Ok(())
}

/// Dispatches config sync to the correct tool based on tool_type.
fn sync_tool_config(
    conn: &SqliteConnection,
    config_path: &Path,
    tool_type: ToolType,
) -> Result<()> {
    match tool_type {
        ToolType::Codex => sync_codex_files(conn, config_path),
        ToolType::Claude => sync_claude_settings(conn, config_path),
    }
}
