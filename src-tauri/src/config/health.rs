use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{time::Duration, time::Instant};

use crate::codex_config::{
    codex_config_path, normalize_model_list_base_url, resolve_token_for_request,
};
use crate::storage::open_database;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckRequest {
    pub base_url: String,
    pub provider_id: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    pub available: bool,
    pub latency_ms: u64,
    pub status_code: Option<u16>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn check_provider_health(
    input: HealthCheckRequest,
) -> Result<HealthCheckResponse, String> {
    check_provider_health_inner(input)
        .await
        .map_err(|error| error.to_string())
}

async fn check_provider_health_inner(
    input: HealthCheckRequest,
) -> Result<HealthCheckResponse> {
    let config_path = codex_config_path()?;
    let conn = open_database(&config_path)?;
    let provider_id = input.provider_id.as_deref().map(str::trim).unwrap_or_default();
    let token =
        resolve_token_for_request(&conn, &config_path, provider_id, input.token.as_deref())?.value;

    check_model_list_health_inner(input, &token).await
}

async fn check_model_list_health_inner(
    input: HealthCheckRequest,
    token: &str,
) -> Result<HealthCheckResponse> {
    let base_url = normalize_model_list_base_url(input.base_url.trim());
    if base_url.is_empty() {
        return Ok(HealthCheckResponse {
            available: false,
            latency_ms: 0,
            status_code: None,
            error: Some("Base URL 不能为空".to_string()),
        });
    }

    let models_url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to build HTTP client")?;

    log::debug!(
        "health check request: method=GET url={} token_present={}",
        models_url,
        !token.is_empty(),
    );

    let start = Instant::now();

    let response = match client
        .get(&models_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            log::error!(
                "health check request error: url={} latency_ms={} error={:?}",
                models_url,
                latency_ms,
                error,
            );
            return Ok(HealthCheckResponse {
                available: false,
                latency_ms,
                status_code: None,
                error: Some(format!("请求失败: {}", error)),
            });
        }
    };

    let latency_ms = start.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();
    let available = response.status().is_success();

    let error = if !available {
        let body = response.text().await.unwrap_or_default();
        let detail = if body.is_empty() {
            format!("HTTP {}", status_code)
        } else {
            format!("HTTP {}: {}", status_code, body.chars().take(200).collect::<String>())
        };
        Some(detail)
    } else {
        None
    };

    log::debug!(
        "health check response: url={} available={} latency_ms={} status={}",
        models_url,
        available,
        latency_ms,
        status_code,
    );

    Ok(HealthCheckResponse {
        available,
        latency_ms,
        status_code: Some(status_code),
        error,
    })
}
