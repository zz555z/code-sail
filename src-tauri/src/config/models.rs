use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{thread, time::Duration};

use crate::claude_config::{
    is_first_party_anthropic_base_url, normalize_claude_base_url, ANTHROPIC_VERSION,
};
use crate::codex_config::{
    codex_config_path, normalize_base_url, parse_model_ids, resolve_token_for_request,
};
use crate::storage::{
    encrypt_optional_token, next_provider_id, next_provider_position, open_database,
    optional_non_empty, provider_exists, replace_provider_models, with_transaction, SqlValue,
    ToolType,
};

const CLAUDE_PRESET_MODELS: &[&str] = &[
    "claude-sonnet-4-20250514",
    "claude-opus-4-20250514",
    "claude-haiku-3-20240307",
    "claude-3-5-sonnet-20241022",
    "claude-3-5-haiku-20241022",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchModelsRequest {
    pub original_id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub token: Option<String>,
    pub tool_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchModelsResponse {
    pub provider_id: String,
    pub models: Vec<String>,
}

#[tauri::command]
pub async fn fetch_models(input: FetchModelsRequest) -> Result<FetchModelsResponse, String> {
    fetch_models_on_network_thread(input)
        .await
        .map_err(|error| error.to_string())
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
    let tool_type = input
        .tool_type
        .as_deref()
        .and_then(|s| ToolType::from_str(s).ok())
        .unwrap_or_default();

    match tool_type {
        ToolType::Claude => fetch_claude_models(input, tool_type).await,
        ToolType::Codex => fetch_codex_models(input, tool_type).await,
    }
}

async fn fetch_claude_models(
    input: FetchModelsRequest,
    tool_type: ToolType,
) -> Result<FetchModelsResponse> {
    let base_url = normalize_claude_base_url(input.base_url.trim());
    if base_url.is_empty() {
        bail!("Base URL 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let original_id = input.original_id.as_deref().map(str::trim).unwrap_or_default();
    let token = resolve_token_for_request(&conn, &config_path, original_id, input.token.as_deref())?;

    let models = match fetch_claude_gateway_models(&base_url, &token.value).await {
        Ok(models) if !models.is_empty() => models,
        Ok(_) => claude_preset_models(),
        Err(error) => {
            log::debug!("claude model discovery failed, using preset models: {:?}", error);
            claude_preset_models()
        }
    };

    let provider_id = if !original_id.is_empty() && provider_exists(&conn, original_id)? {
        original_id.to_string()
    } else {
        next_provider_id(&conn, input.name.trim(), input.base_url.trim(), None, tool_type)?
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
    let tool_type_str = tool_type.as_str();
    let position = next_provider_position(&conn, tool_type)?;

    with_transaction(&conn, |conn| {
        conn.execute(
            r#"
            INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token, tool_type, position)
            VALUES (?1, ?2, ?3, ?4, 'responses', 0, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                base_url = excluded.base_url,
                model = excluded.model,
                token = COALESCE(excluded.token, providers.token)
            "#,
            &[
                SqlValue::Text(&provider_id),
                SqlValue::Text(provider_name),
                SqlValue::Text(input.base_url.trim()),
                SqlValue::OptionalText(optional_non_empty(selected_model)),
                SqlValue::OptionalText(stored_input_token.as_deref()),
                SqlValue::Text(tool_type_str),
                SqlValue::I64(position),
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

async fn fetch_claude_gateway_models(base_url: &str, token: &str) -> Result<Vec<String>> {
    let models_url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Claude model discovery HTTP client")?;

    log::debug!(
        "claude model discovery request: method=GET url={} auth_mode={}",
        models_url,
        if is_first_party_anthropic_base_url(base_url) {
            "x-api-key"
        } else {
            "bearer"
        },
    );

    let request = client
        .get(&models_url)
        .header("anthropic-version", ANTHROPIC_VERSION);
    let request = if is_first_party_anthropic_base_url(base_url) {
        request.header("X-Api-Key", token)
    } else {
        request.header("Authorization", format!("Bearer {}", token))
    };

    let response = request.send().await.context("请求 Claude 模型列表失败")?;
    let status = response.status();
    let response_text = response.text().await.context("读取 Claude 模型列表响应失败")?;
    if !status.is_success() {
        bail!("Claude 模型列表请求失败: HTTP {}", status.as_u16());
    }

    let body = serde_json::from_str::<Value>(&response_text).context("Claude 模型列表不是有效 JSON")?;
    let models = parse_model_ids(&body)
        .into_iter()
        .filter(|model| model.starts_with("claude") || model.starts_with("anthropic"))
        .collect::<Vec<_>>();
    Ok(models)
}

fn claude_preset_models() -> Vec<String> {
    CLAUDE_PRESET_MODELS.iter().map(|s| s.to_string()).collect()
}

async fn fetch_codex_models(
    input: FetchModelsRequest,
    tool_type: ToolType,
) -> Result<FetchModelsResponse> {
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

    log::debug!(
        "fetch_models request: method=GET models_url={} token_present=true",
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
            log::error!(
                "fetch_models request error: models_url={} error={:?}",
                models_url,
                error,
            );
            return Err(anyhow!(error).context("请求模型列表失败"));
        }
    };

    let status = response.status();
    let response_text = response.text().await.context("读取模型列表响应失败")?;
    log::debug!("fetch_models response status={}", status);

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
        next_provider_id(&conn, input.name.trim(), input.base_url.trim(), None, tool_type)?
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
    let tool_type_str = tool_type.as_str();
    let position = next_provider_position(&conn, tool_type)?;

    with_transaction(&conn, |conn| {
        conn.execute(
            r#"
            INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token, tool_type, position)
            VALUES (?1, ?2, ?3, ?4, 'responses', 1, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                base_url = excluded.base_url,
                model = excluded.model,
                token = COALESCE(excluded.token, providers.token)
            "#,
            &[
                SqlValue::Text(&provider_id),
                SqlValue::Text(provider_name),
                SqlValue::Text(&base_url),
                SqlValue::OptionalText(optional_non_empty(selected_model)),
                SqlValue::OptionalText(stored_input_token.as_deref()),
                SqlValue::Text(tool_type_str),
                SqlValue::I64(position),
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
