use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{thread, time::Duration, time::Instant};

use crate::claude_config::{
    is_first_party_anthropic_base_url, normalize_claude_base_url, ANTHROPIC_VERSION,
};
use crate::codex_config::normalize_base_url;
use crate::storage::ToolType;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckRequest {
    pub base_url: String,
    pub token: String,
    pub model: Option<String>,
    pub tool_type: Option<String>,
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
    check_provider_health_on_network_thread(input)
        .await
        .map_err(|error| error.to_string())
}

async fn check_provider_health_on_network_thread(
    input: HealthCheckRequest,
) -> Result<HealthCheckResponse> {
    let (sender, receiver) = tokio::sync::oneshot::channel();

    thread::Builder::new()
        .name("codex-health-check".to_string())
        .spawn(move || {
            let result = (|| -> Result<HealthCheckResponse> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .context("failed to build health check Tokio runtime")?;
                runtime.block_on(check_provider_health_inner(input))
            })();
            let _ = sender.send(result);
        })
        .context("failed to spawn health check network thread")?;

    receiver
        .await
        .context("health check network thread exited without result")?
}

async fn check_provider_health_inner(
    input: HealthCheckRequest,
) -> Result<HealthCheckResponse> {
    let tool_type = input
        .tool_type
        .as_deref()
        .and_then(|s| ToolType::from_str(s).ok())
        .unwrap_or_default();

    match tool_type {
        ToolType::Claude => check_claude_health_inner(input).await,
        ToolType::Codex => check_codex_health_inner(input).await,
    }
}

async fn check_codex_health_inner(
    input: HealthCheckRequest,
) -> Result<HealthCheckResponse> {
    let base_url = normalize_base_url(input.base_url.trim());
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
        !input.token.is_empty(),
    );

    let start = Instant::now();

    let response = match client
        .get(&models_url)
        .header("Authorization", format!("Bearer {}", input.token))
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

async fn check_claude_health_inner(
    input: HealthCheckRequest,
) -> Result<HealthCheckResponse> {
    let base_url = normalize_claude_base_url(input.base_url.trim());
    if base_url.is_empty() {
        return Ok(HealthCheckResponse {
            available: false,
            latency_ms: 0,
            status_code: None,
            error: Some("Base URL 不能为空".to_string()),
        });
    }

    let token = input.token.trim();
    if token.is_empty() {
        return Ok(HealthCheckResponse {
            available: false,
            latency_ms: 0,
            status_code: None,
            error: Some("Token 不能为空".to_string()),
        });
    }

    let model = input
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or("claude-sonnet-4-20250514");
    let count_tokens_url = format!("{}/v1/messages/count_tokens", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to build Claude health check HTTP client")?;

    log::debug!(
        "claude health check request: method=POST url={} model={} auth_mode={}",
        count_tokens_url,
        model,
        if is_first_party_anthropic_base_url(&base_url) {
            "x-api-key"
        } else {
            "bearer"
        },
    );

    let start = Instant::now();
    let request = client
        .post(&count_tokens_url)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .json(&json!({
            "model": model,
            "messages": [
                { "role": "user", "content": "ping" }
            ]
        }));
    let request = if is_first_party_anthropic_base_url(&base_url) {
        request.header("X-Api-Key", token)
    } else {
        request.header("Authorization", format!("Bearer {}", token))
    };

    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            log::error!(
                "claude health check request error: url={} latency_ms={} error={:?}",
                count_tokens_url,
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

    Ok(HealthCheckResponse {
        available,
        latency_ms,
        status_code: Some(status_code),
        error,
    })
}
