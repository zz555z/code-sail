use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{thread, time::Duration};

use crate::codex_config::{
    codex_config_path, normalize_base_url, parse_model_ids, resolve_token_for_request,
};
use crate::storage::{
    encrypt_optional_token, next_provider_id, open_database, optional_non_empty, provider_exists,
    replace_provider_models, with_transaction, SqlValue,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchModelsRequest {
    pub original_id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub token: Option<String>,
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
            VALUES (?1, ?2, ?3, ?4, 'responses', 1, ?5)
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
