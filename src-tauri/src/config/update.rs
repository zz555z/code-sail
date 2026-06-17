use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, thread, time::Duration};

use crate::tasks::run_background_task;
use crate::tools::open_external_url;

const CODESAIL_RELEASES_URL: &str = "https://github.com/zz555z/code-sail/releases";
const CODESAIL_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/zz555z/code-sail/releases/latest";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: String,
    pub release_name: Option<String>,
    pub published_at: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    name: Option<String>,
    published_at: Option<String>,
}

#[tauri::command]
pub async fn check_app_update(current_version: String) -> Result<AppUpdateInfo, String> {
    check_app_update_on_network_thread(current_version)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn open_app_update() -> Result<(), String> {
    run_background_task("codesail-open-update", || {
        open_external_url(CODESAIL_RELEASES_URL)
    })
    .await
    .map_err(|error| error.to_string())
}

async fn check_app_update_on_network_thread(current_version: String) -> Result<AppUpdateInfo> {
    let (sender, receiver) = tokio::sync::oneshot::channel();

    thread::Builder::new()
        .name("codesail-check-update".to_string())
        .spawn(move || {
            let result = (|| -> Result<AppUpdateInfo> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .context("failed to build check_app_update Tokio runtime")?;
                runtime.block_on(check_app_update_inner(current_version))
            })();
            let _ = sender.send(result);
        })
        .context("failed to spawn check_app_update network thread")?;

    receiver
        .await
        .context("check_app_update network thread exited without result")?
}

async fn check_app_update_inner(current_version: String) -> Result<AppUpdateInfo> {
    let current_version = normalize_version(&current_version);
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(15))
        .user_agent("CodeSail")
        .build()
        .context("failed to build GitHub release client")?;

    let response = client
        .get(CODESAIL_LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("检查 GitHub 最新版本失败")?;
    let status = response.status();
    if !status.is_success() {
        let detail = if status.as_u16() == 404 {
            "没有找到已发布的 GitHub Release；草稿 Release 不会被检测到".to_string()
        } else {
            format!("GitHub 返回 HTTP {}", status.as_u16())
        };
        return Ok(AppUpdateInfo {
            current_version,
            latest_version: None,
            update_available: false,
            release_url: CODESAIL_RELEASES_URL.to_string(),
            release_name: None,
            published_at: None,
            detail: Some(detail),
        });
    }

    let release = response
        .json::<GitHubRelease>()
        .await
        .context("GitHub 最新版本响应格式不正确")?;
    let latest_version = normalize_version(&release.tag_name);
    let update_available = compare_versions(&latest_version, &current_version)
        .map(|ordering| ordering.is_gt())
        .unwrap_or(false);

    Ok(AppUpdateInfo {
        current_version,
        latest_version: Some(latest_version),
        update_available,
        release_url: release.html_url,
        release_name: release.name,
        published_at: release.published_at,
        detail: None,
    })
}

fn normalize_version(version: &str) -> String {
    version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

fn compare_versions(left: &str, right: &str) -> Option<Ordering> {
    let left_parts = semantic_version_parts(&normalize_version(left))?;
    let right_parts = semantic_version_parts(&normalize_version(right))?;
    Some(left_parts.cmp(&right_parts))
}

fn semantic_version_parts(version: &str) -> Option<[u64; 3]> {
    let mut parts = [0_u64; 3];
    for (index, value) in version.split('.').take(3).enumerate() {
        let digits = value
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>();
        parts[index] = digits.parse::<u64>().ok()?;
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semantic_versions_numerically() {
        assert_eq!(compare_versions("0.0.10", "0.0.9"), Some(Ordering::Greater));
        assert_eq!(compare_versions("v1.2.3", "1.2.3"), Some(Ordering::Equal));
        assert_eq!(compare_versions("1.2.3", "1.3.0"), Some(Ordering::Less));
        assert_eq!(compare_versions("1.2.3-beta.1", "1.2.3"), Some(Ordering::Equal));
    }
}
