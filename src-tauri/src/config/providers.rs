use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::codex_config::{codex_config_path, normalize_base_url, sync_codex_files};
use crate::storage::{
    bool_to_i64, copy_provider_models, delete_setting, encrypt_optional_token, get_provider,
    get_provider_token, get_setting, next_copy_id, next_copy_name, next_provider_id,
    open_database, optional_non_empty, provider_exists, set_setting, with_transaction, SqlValue,
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderResponse {
    pub provider_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyProviderResponse {
    pub provider_id: String,
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
pub async fn copy_provider(provider_id: String) -> Result<CopyProviderResponse, String> {
    run_background_task("codex-copy-provider", move || copy_provider_inner(&provider_id))
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
pub async fn set_current_model(input: SetCurrentModelRequest) -> Result<(), String> {
    run_background_task("codex-set-current-model", move || set_current_model_inner(input))
        .await
        .map_err(|error| error.to_string())
}

fn save_provider_inner(input: ProviderInput) -> Result<SaveProviderResponse> {
    let name = input.name.trim();
    let base_url = input.base_url.trim();
    let model = input.model.trim();

    if base_url.is_empty() {
        bail!("Base URL 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let original_id = input.original_id.as_deref().map(str::trim).unwrap_or_default();
    let provider_id = if !original_id.is_empty() && provider_exists(&conn, original_id)? {
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
        if !original_id.is_empty() {
            get_provider_token(&conn, &config_path, original_id)?
        } else {
            get_provider_token(&conn, &config_path, &provider_id)?
        }
    } else {
        None
    };
    let token = explicit_token.or(existing_token);
    let stored_token = encrypt_optional_token(&config_path, token.as_deref())?;
    let normalized_base_url = normalize_base_url(base_url);
    let provider_name = if name.is_empty() { provider_id.as_str() } else { name };

    with_transaction(&conn, |conn| {
        if !original_id.is_empty() && original_id != provider_id {
            conn.execute("DELETE FROM providers WHERE id = ?1", &[SqlValue::Text(original_id)])?;
            if get_setting(conn, "active_provider")?.as_deref() == Some(original_id) {
                set_setting(conn, "active_provider", &provider_id)?;
            }
        }

        conn.execute(
            r#"
            INSERT INTO providers (id, name, base_url, model, token)
            VALUES (?1, ?2, ?3, ?4, ?5)
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
            ],
        )?;
        if input.update_config {
            set_setting(conn, "active_provider", &provider_id)?;
            if let Some(model) = optional_non_empty(model) {
                set_setting(conn, "active_model", model)?;
            }
        }
        Ok(())
    })?;

    if input.update_config {
        sync_codex_files(&conn, &config_path)?;
    }
    Ok(SaveProviderResponse {
        provider_id,
    })
}

fn delete_provider_inner(provider_id: &str) -> Result<()> {
    let id = provider_id.trim();
    if id.is_empty() {
        bail!("Provider ID 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    with_transaction(&conn, |conn| {
        conn.execute(
            "DELETE FROM provider_models WHERE provider_id = ?1",
            &[SqlValue::Text(id)],
        )?;
        conn.execute("DELETE FROM providers WHERE id = ?1", &[SqlValue::Text(id)])?;
        if get_setting(conn, "active_provider")?.as_deref() == Some(id) {
            delete_setting(conn, "active_provider")?;
            delete_setting(conn, "active_model")?;
        }
        Ok(())
    })?;

    sync_codex_files(&conn, &config_path)?;
    Ok(())
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

    conn.execute(
        r#"
        INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        &[
            SqlValue::Text(&copy_id),
            SqlValue::Text(&copy_name),
            SqlValue::Text(&source.base_url),
            SqlValue::OptionalText(source.model.as_deref()),
            SqlValue::Text(&source.wire_api),
            SqlValue::I64(bool_to_i64(source.requires_open_ai_auth)),
            SqlValue::OptionalText(copied_token.as_deref()),
        ],
    )?;
    copy_provider_models(&conn, source_id, &copy_id)?;

    sync_codex_files(&conn, &config_path)?;
    Ok(CopyProviderResponse {
        provider_id: copy_id,
    })
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
        set_setting(conn, "active_provider", provider_id)?;
        set_setting(conn, "active_model", model)?;
        Ok(())
    })?;

    if input.update_config {
        sync_codex_files(&conn, &config_path)?;
    }
    Ok(())
}
