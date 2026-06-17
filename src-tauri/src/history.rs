use anyhow::{bail, Context, Result};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::tasks::run_background_task;
use crate::terminal::open_codex_command_in_terminal;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryProviderGroup {
    provider: String,
    sessions: Vec<HistorySessionSummary>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionSummary {
    session_id: String,
    provider: String,
    title: String,
    timestamp: Option<i64>,
    path: String,
    message_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryConversation {
    session_id: String,
    provider: String,
    title: String,
    timestamp: Option<i64>,
    path: String,
    messages: Vec<HistoryMessage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteHistoryResponse {
    success_count: usize,
    failure_count: usize,
    errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadHistorySessionRequest {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeHistorySessionRequest {
    session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteHistorySessionRequest {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteHistoryProviderRequest {
    provider: String,
}

#[derive(Clone)]
struct CachedSummary {
    len: u64,
    modified_millis: u128,
    summary: Option<HistorySessionSummary>,
}

static SUMMARY_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedSummary>>> = OnceLock::new();

const MAX_SESSIONS: usize = 200;

#[tauri::command]
pub async fn list_history_sessions() -> Result<Vec<HistoryProviderGroup>, String> {
    run_background_task("codex-list-history-sessions", list_history_sessions_inner)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn read_history_session(
    input: ReadHistorySessionRequest,
) -> Result<HistoryConversation, String> {
    run_background_task("codex-read-history-session", move || {
        read_history_session_inner(&input.path)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn resume_history_session(input: ResumeHistorySessionRequest) -> Result<(), String> {
    run_background_task("codex-resume-history-session", move || {
        resume_history_session_inner(&input.session_id)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn delete_history_session(
    input: DeleteHistorySessionRequest,
) -> Result<DeleteHistoryResponse, String> {
    run_background_task("codex-delete-history-session", move || {
        delete_history_session_inner(&input.path)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn delete_history_provider(
    input: DeleteHistoryProviderRequest,
) -> Result<DeleteHistoryResponse, String> {
    run_background_task("codex-delete-history-provider", move || {
        delete_history_provider_inner(&input.provider)
    })
    .await
    .map_err(|error| error.to_string())
}

fn list_history_sessions_inner() -> Result<Vec<HistoryProviderGroup>> {
    let root = sessions_root()?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut session_files = Vec::new();
    collect_session_files(&root, &mut session_files)?;
    prune_summary_cache(&session_files);

    let mut all_sessions: Vec<HistorySessionSummary> = session_files
        .iter()
        .filter_map(|f| summarize_session_file_cached(f).transpose())
        .collect::<Result<Vec<_>>>()?;

    all_sessions.sort_by(|left, right| {
        right
            .timestamp
            .unwrap_or_default()
            .cmp(&left.timestamp.unwrap_or_default())
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
    });

    all_sessions.truncate(MAX_SESSIONS);

    let mut groups = BTreeMap::<String, Vec<HistorySessionSummary>>::new();
    for summary in all_sessions {
        groups
            .entry(summary.provider.clone())
            .or_default()
            .push(summary);
    }

    let mut grouped = groups
        .into_iter()
        .map(|(provider, sessions)| HistoryProviderGroup { provider, sessions })
        .collect::<Vec<_>>();

    grouped.sort_by(|left, right| left.provider.to_lowercase().cmp(&right.provider.to_lowercase()));
    Ok(grouped)
}

fn prune_summary_cache(session_files: &[PathBuf]) {
    let Some(cache) = SUMMARY_CACHE.get() else {
        return;
    };
    let live_paths = session_files.iter().collect::<HashSet<_>>();
    if let Ok(mut items) = cache.lock() {
        items.retain(|path, _| live_paths.contains(path));
    }
}

fn summarize_session_file_cached(session_file: &Path) -> Result<Option<HistorySessionSummary>> {
    let metadata = fs::metadata(session_file)
        .with_context(|| format!("无法读取会话文件信息: {}", session_file.display()))?;
    let len = metadata.len();
    let modified_millis = system_time_millis(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH));
    let cache_key = session_file.to_path_buf();
    let cache = SUMMARY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(cached) = cache
        .lock()
        .ok()
        .and_then(|items| items.get(&cache_key).cloned())
        .filter(|cached| cached.len == len && cached.modified_millis == modified_millis)
    {
        return Ok(cached.summary);
    }

    let summary = summarize_session_file(session_file)?;
    if let Ok(mut items) = cache.lock() {
        items.insert(
            cache_key,
            CachedSummary {
                len,
                modified_millis,
                summary: summary.clone(),
            },
        );
    }
    Ok(summary)
}

fn system_time_millis(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn read_history_session_inner(path: &str) -> Result<HistoryConversation> {
    let session_file = validated_session_file(path)?;

    let summary = summarize_session_file_cached(&session_file)?.unwrap_or_else(|| {
        let session_id = session_id_from_file(&session_file);
        HistorySessionSummary {
            session_id,
            provider: "unknown".to_string(),
            title: "未命名会话".to_string(),
            timestamp: None,
            path: session_file.display().to_string(),
            message_count: 0,
        }
    });
    let messages = read_history_messages(&session_file)?;

    Ok(HistoryConversation {
        session_id: summary.session_id,
        provider: summary.provider,
        title: summary.title,
        timestamp: summary.timestamp,
        path: summary.path,
        messages,
    })
}

fn resume_history_session_inner(session_id: &str) -> Result<()> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        bail!("Session ID 不能为空");
    }
    if !session_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!("Session ID 格式不合法");
    }

    open_resume_session(session_id)
}

fn open_resume_session(session_id: &str) -> Result<()> {
    open_codex_command_in_terminal(&["resume", session_id])
        .with_context(|| format!("无法恢复会话: {session_id}"))?;

    Ok(())
}

fn delete_history_session_inner(path: &str) -> Result<DeleteHistoryResponse> {
    let session_file = validated_session_file(path)?;
    match fs::remove_file(&session_file) {
        Ok(()) => Ok(DeleteHistoryResponse {
            success_count: 1,
            failure_count: 0,
            errors: Vec::new(),
        }),
        Err(error) => Ok(DeleteHistoryResponse {
            success_count: 0,
            failure_count: 1,
            errors: vec![format!("{}: {}", session_file.display(), error)],
        }),
    }
}

fn delete_history_provider_inner(provider: &str) -> Result<DeleteHistoryResponse> {
    let provider = provider.trim();
    if provider.is_empty() {
        bail!("Provider 不能为空");
    }

    let groups = list_history_sessions_inner()?;
    let Some(group) = groups.into_iter().find(|group| group.provider == provider) else {
        return Ok(DeleteHistoryResponse {
            success_count: 0,
            failure_count: 0,
            errors: Vec::new(),
        });
    };

    let mut response = DeleteHistoryResponse {
        success_count: 0,
        failure_count: 0,
        errors: Vec::new(),
    };

    for session in group.sessions {
        let session_file = validated_session_file(&session.path)?;
        match fs::remove_file(&session_file) {
            Ok(()) => response.success_count += 1,
            Err(error) => {
                response.failure_count += 1;
                response
                    .errors
                    .push(format!("{}: {}", session_file.display(), error));
            }
        }
    }

    Ok(response)
}

fn sessions_root() -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to locate the home directory")?;
    Ok(home.join(".codex").join("sessions"))
}

fn collect_session_files(dir: &Path, session_files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("无法读取目录: {}", dir.display()))? {
        let entry = entry.with_context(|| format!("无法读取目录项: {}", dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("无法读取文件类型: {}", entry.path().display()))?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_session_files(&path, session_files)?;
        } else if file_type.is_file() && path.extension().and_then(|item| item.to_str()) == Some("jsonl") {
            session_files.push(path);
        }
    }

    Ok(())
}

fn summarize_session_file(session_file: &Path) -> Result<Option<HistorySessionSummary>> {
    let file = File::open(session_file)
        .with_context(|| format!("无法读取会话文件: {}", session_file.display()))?;
    let reader = BufReader::new(file);

    let mut session_id = session_id_from_file(session_file);
    let mut provider: Option<String> = None;
    let mut title: Option<String> = None;
    let mut timestamp: Option<i64> = None;
    let mut message_count = 0usize;

    for line in reader.lines() {
        let line = line.with_context(|| format!("无法读取 JSONL 行: {}", session_file.display()))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if string_field(&value, "type").as_deref() == Some("session_meta") {
            let payload = value.get("payload").unwrap_or(&Value::Null);
            if is_internal_subagent_session(payload) {
                return Ok(None);
            }
            if provider.is_none() {
                provider = string_field(payload, "model_provider").filter(|item| !item.trim().is_empty());
            }
            if timestamp.is_none() {
                timestamp = timestamp_field(payload, "timestamp").or_else(|| timestamp_field(&value, "timestamp"));
            }
            if let Some(id) = string_field(payload, "id") {
                if !id.trim().is_empty() {
                    session_id = id;
                }
            }
        } else if timestamp.is_none() {
            timestamp = timestamp_field(&value, "timestamp");
        }

        let messages = messages_from_value(&value);
        message_count += messages.len();
        if title.is_none() {
            title = messages
                .iter()
                .find(|message| message.role.eq_ignore_ascii_case("user") && !message.content.trim().is_empty())
                .map(|message| title_from_content(&message.content));
        }
    }

    let Some(provider) = provider else {
        return Ok(None);
    };

    Ok(Some(HistorySessionSummary {
        session_id,
        provider,
        title: title.unwrap_or_else(|| "未命名会话".to_string()),
        timestamp,
        path: session_file.display().to_string(),
        message_count,
    }))
}

fn is_internal_subagent_session(payload: &Value) -> bool {
    string_field(payload, "thread_source").as_deref() == Some("subagent")
        || payload.get("source").and_then(|source| source.get("subagent")).is_some()
}

fn read_history_messages(session_file: &Path) -> Result<Vec<HistoryMessage>> {
    let file = File::open(session_file)
        .with_context(|| format!("无法读取会话文件: {}", session_file.display()))?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

    for line in reader.lines() {
        let line = line.with_context(|| format!("无法读取 JSONL 行: {}", session_file.display()))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        messages.extend(messages_from_value(&value));
    }

    Ok(messages)
}

fn messages_from_value(value: &Value) -> Vec<HistoryMessage> {
    let top_type = string_field(value, "type").unwrap_or_default();
    let payload = value.get("payload").unwrap_or(&Value::Null);
    let payload_type = string_field(payload, "type").unwrap_or_default();

    if top_type == "event_msg" && payload_type == "user_message" {
        let content = string_field(payload, "message").unwrap_or_default();
        if content.trim().is_empty() {
            return Vec::new();
        }
        return vec![HistoryMessage {
            role: "user".to_string(),
            content,
        }];
    }

    if top_type == "response_item" && payload_type == "message" {
        let role = string_field(payload, "role").unwrap_or_else(|| "unknown".to_string());
        if role != "assistant" {
            return Vec::new();
        }
        let content = payload
            .get("content")
            .map(content_to_string)
            .unwrap_or_default();
        if content.trim().is_empty() {
            return Vec::new();
        }
        return vec![HistoryMessage { role, content }];
    }

    Vec::new()
}

fn content_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(content_to_string)
            .filter(|item| !item.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => ["text", "content", "value"]
            .iter()
            .find_map(|key| map.get(*key).map(content_to_string))
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| serde_json::to_string(value).unwrap_or_default()),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn timestamp_field(value: &Value, field: &str) -> Option<i64> {
    let value = value.get(field)?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|item| i64::try_from(item).ok()))
        .or_else(|| value.as_f64().map(|item| item as i64))
        .or_else(|| {
            value.as_str().and_then(|item| {
                item.parse::<i64>()
                    .ok()
                    .or_else(|| DateTime::parse_from_rfc3339(item).ok().map(|time| time.timestamp_millis()))
            })
        })
}

fn title_from_content(content: &str) -> String {
    let title = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        "未命名会话".to_string()
    } else if title.chars().count() > 120 {
        format!("{}...", title.chars().take(120).collect::<String>())
    } else {
        title
    }
}

fn session_id_from_file(session_file: &Path) -> String {
    let stem = session_file
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("unknown-session")
        .to_string();

    if stem.len() >= 36 {
        let suffix = &stem[stem.len() - 36..];
        if is_uuid_like(suffix) {
            return suffix.to_string();
        }
    }

    stem
}

fn is_uuid_like(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        matches!(index, 8 | 13 | 18 | 23) && character == '-'
            || !matches!(index, 8 | 13 | 18 | 23) && character.is_ascii_hexdigit()
    })
}

fn validated_session_file(path: &str) -> Result<PathBuf> {
    let path = path.trim();
    if path.is_empty() {
        bail!("会话路径不能为空");
    }

    let root = fs::canonicalize(sessions_root()?).context("未找到 ~/.codex/sessions")?;
    let input = fs::canonicalize(PathBuf::from(path))
        .with_context(|| format!("会话路径不存在: {path}"))?;
    if !input.starts_with(&root) {
        bail!("会话路径不在 ~/.codex/sessions 下");
    }
    if !input.is_file() {
        bail!("会话路径不是文件: {}", input.display());
    }
    if input.extension().and_then(|item| item.to_str()) != Some("jsonl") {
        bail!("会话文件不是 JSONL: {}", input.display());
    }

    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_session_file(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("codesail-history-{name}-{unique}.jsonl"))
    }

    #[test]
    fn summarizes_codex_history_jsonl_sessions() -> Result<()> {
        let path = temp_session_file("summary");
        fs::write(
            &path,
            r#"{"type":"session_meta","payload":{"id":"session-1","model_provider":"openai","timestamp":"2026-06-16T08:00:00Z"}}
{"type":"event_msg","payload":{"type":"user_message","message":"Hello from history"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"Hi there"}]}}
"#,
        )?;

        let summary = summarize_session_file(&path)?.expect("summary");
        assert_eq!(summary.session_id, "session-1");
        assert_eq!(summary.provider, "openai");
        assert_eq!(summary.title, "Hello from history");
        assert_eq!(summary.message_count, 2);

        let messages = read_history_messages(&path)?;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].content, "Hi there");

        let _ = fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn skips_internal_subagent_history_sessions() -> Result<()> {
        let path = temp_session_file("subagent");
        fs::write(
            &path,
            r#"{"type":"session_meta","payload":{"model_provider":"openai","thread_source":"subagent"}}
"#,
        )?;

        assert!(summarize_session_file(&path)?.is_none());

        let _ = fs::remove_file(path);
        Ok(())
    }
}
