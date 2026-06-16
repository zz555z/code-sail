use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{thread, time::Duration};

use crate::codex_config::{
    codex_config_path, normalize_base_url, parse_model_ids, resolve_token_for_request,
    sync_codex_files,
};
use crate::storage::{
    bool_to_i64, collect_providers_from_database, copy_provider_models, delete_setting,
    encrypt_optional_token, get_provider, get_provider_token, get_setting, next_copy_id,
    next_copy_name, next_provider_id, open_database, optional_non_empty, provider_exists,
    replace_provider_models, set_setting, with_transaction, ProviderView, SqlValue,
};
use crate::terminal::{open_codex_terminal_inner, restart_codex_app_inner};
use crate::tasks::run_background_task;
use crate::tools::{
    load_tool_statuses, open_tool_install_inner, OpenToolInstallRequest, ToolStatus,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    config_path: String,
    config_exists: bool,
    active_provider: Option<String>,
    active_model: Option<String>,
    providers: Vec<ProviderView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    original_id: Option<String>,
    name: String,
    base_url: String,
    model: String,
    token: Option<String>,
    update_config: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchModelsRequest {
    original_id: Option<String>,
    name: String,
    base_url: String,
    model: String,
    token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchModelsResponse {
    provider_id: String,
    models: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderResponse {
    provider_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetCurrentModelRequest {
    provider_id: String,
    model: String,
    token: Option<String>,
    update_config: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyProviderResponse {
    provider_id: String,
}

#[tauri::command]
pub async fn get_app_state() -> Result<AppState, String> {
    run_background_task("codex-load-app-state", load_app_state)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_tool_statuses() -> Result<Vec<ToolStatus>, String> {
    run_background_task("codex-load-tool-statuses", load_tool_statuses)
        .await
        .map_err(|error| error.to_string())
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
pub async fn fetch_models(input: FetchModelsRequest) -> Result<FetchModelsResponse, String> {
    fetch_models_on_network_thread(input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_current_model(input: SetCurrentModelRequest) -> Result<(), String> {
    run_background_task("codex-set-current-model", move || set_current_model_inner(input))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn restart_codex_app() -> Result<(), String> {
    run_background_task("codex-restart-app", restart_codex_app_inner)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn open_codex_terminal() -> Result<(), String> {
    run_background_task("codex-open-terminal", open_codex_terminal_inner)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn open_tool_install(input: OpenToolInstallRequest) -> Result<(), String> {
    run_background_task("codex-open-tool-install", move || {
        open_tool_install_inner(&input.command)
    })
    .await
    .map_err(|error| error.to_string())
}

fn load_app_state() -> Result<AppState> {
    let config_path = codex_config_path()?;
    let config_exists = config_path.exists();
    let conn = open_database(&config_path)?;

    Ok(AppState {
        config_path: config_path.display().to_string(),
        config_exists,
        active_provider: get_setting(&conn, "active_provider")?,
        active_model: get_setting(&conn, "active_model")?,
        providers: collect_providers_from_database(&conn, &config_path)?,
    })
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
            INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                base_url = excluded.base_url,
                model = excluded.model,
                wire_api = excluded.wire_api,
                requires_openai_auth = excluded.requires_openai_auth,
                token = excluded.token
            "#,
            &[
                SqlValue::Text(&provider_id),
                SqlValue::Text(provider_name),
                SqlValue::Text(&normalized_base_url),
                SqlValue::OptionalText(optional_non_empty(model)),
                SqlValue::Text("responses"),
                SqlValue::I64(1),
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

async fn fetch_models_on_network_thread(input: FetchModelsRequest) -> Result<FetchModelsResponse> {
    let (sender, receiver) = tokio::sync::oneshot::channel();

    thread::Builder::new()
        .name("codex-fetch-models".to_string())
        .spawn(move || {
            let result = (|| -> Result<FetchModelsResponse> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .context("failed to build fetch_models Tokio runtime")?;
                runtime.block_on(fetch_models_inner(input))
            })();
            let _ = sender.send(result);
        })
        .context("failed to spawn fetch_models network thread")?;

    receiver
        .await
        .context("fetch_models network thread exited without result")?
}

async fn fetch_models_inner(input: FetchModelsRequest) -> Result<FetchModelsResponse> {
    let base_url = normalize_base_url(input.base_url.trim());
    if base_url.is_empty() {
        bail!("Base URL 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let original_id = input.original_id.as_deref().map(str::trim).unwrap_or_default();
    let token = resolve_token_for_request(&conn, &config_path, original_id, input.token.as_deref())?;
    let models_url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")?;

    eprintln!(
        "[codex-config] fetch_models request\n  method=GET\n  models_url={}\n  token_present=true",
        models_url,
    );

    let response = match client
        .get(&models_url)
        .header("Authorization", format!("Bearer {}", token.value))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            eprintln!(
                "[codex-config] fetch_models request error\n  models_url={}\n  error={:#?}",
                models_url, error,
            );
            return Err(anyhow!(error).context("请求模型列表失败"));
        }
    };

    let status = response.status();
    let response_text = response.text().await.context("读取模型列表响应失败")?;
    eprintln!("[codex-config] fetch_models response status={}", status);

    if !status.is_success() {
        bail!("模型列表请求失败: HTTP {}", status.as_u16());
    }

    let body = serde_json::from_str::<Value>(&response_text).context("模型列表不是有效 JSON")?;
    let models = parse_model_ids(&body);
    if models.is_empty() {
        bail!("响应里没有找到模型 id");
    }

    let provider_id = if !original_id.is_empty() && provider_exists(&conn, original_id)? {
        original_id.to_string()
    } else {
        next_provider_id(&conn, input.name.trim(), input.base_url.trim(), None)?
    };
    let provider_name = if input.name.trim().is_empty() {
        provider_id.as_str()
    } else {
        input.name.trim()
    };
    let selected_model = input
        .model
        .trim()
        .is_empty()
        .then(|| models.first().map(String::as_str).unwrap_or_default())
        .unwrap_or_else(|| input.model.trim());
    let stored_input_token =
        encrypt_optional_token(&config_path, input.token.as_deref().and_then(optional_non_empty))?;

    with_transaction(&conn, |conn| {
        conn.execute(
            r#"
            INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                base_url = excluded.base_url,
                model = excluded.model,
                wire_api = excluded.wire_api,
                requires_openai_auth = excluded.requires_openai_auth,
                token = COALESCE(excluded.token, providers.token)
            "#,
            &[
                SqlValue::Text(&provider_id),
                SqlValue::Text(provider_name),
                SqlValue::Text(&base_url),
                SqlValue::OptionalText(optional_non_empty(selected_model)),
                SqlValue::Text("responses"),
                SqlValue::I64(1),
                SqlValue::OptionalText(stored_input_token.as_deref()),
            ],
        )?;
        replace_provider_models(conn, &provider_id, &models)?;
        Ok(())
    })?;

    Ok(FetchModelsResponse {
        provider_id,
        models,
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
