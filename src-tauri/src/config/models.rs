use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::codex_config::{
    codex_config_path, normalize_model_list_base_url, parse_model_ids, resolve_token_for_request,
};
use crate::http;
use crate::storage::{
    open_database, provider_belongs_to_tool, replace_provider_models, ToolType,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchModelsRequest {
    pub original_id: Option<String>,
    pub base_url: String,
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
    fetch_models_inner(input)
        .await
        .map_err(|error| error.to_string())
}

async fn fetch_models_inner(input: FetchModelsRequest) -> Result<FetchModelsResponse> {
    let tool_type = input
        .tool_type
        .as_deref()
        .and_then(|s| ToolType::from_str(s).ok())
        .unwrap_or_default();

    fetch_provider_models(input, tool_type).await
}

async fn fetch_provider_models(
    input: FetchModelsRequest,
    tool_type: ToolType,
) -> Result<FetchModelsResponse> {
    let base_url = normalize_model_list_base_url(input.base_url.trim());
    if base_url.is_empty() {
        bail!("Base URL 不能为空");
    }

    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let original_id = input.original_id.as_deref().map(str::trim).unwrap_or_default();
    let token = resolve_token_for_request(&conn, &config_path, original_id, input.token.as_deref())?;
    let models_url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = http::shared_client();

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

    let provider_id = if !original_id.is_empty()
        && provider_belongs_to_tool(&conn, original_id, tool_type)?
    {
        replace_provider_models(&conn, original_id, &models)?;
        original_id.to_string()
    } else {
        String::new()
    };

    Ok(FetchModelsResponse {
        provider_id,
        models,
    })
}
