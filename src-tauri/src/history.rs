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
use crate::terminal::{open_claude_command_in_terminal, open_codex_command_in_terminal};
use crate::storage::ToolType;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryProviderGroup {
    provider: String,
    tool_type: ToolType,
    sessions: Vec<HistorySessionSummary>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionSummary {
    session_id: String,
    provider: String,
    tool_type: ToolType,
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
    tool_type: ToolType,
    title: String,
    timestamp: Option<i64>,
    path: String,
    messages: Vec<HistoryMessage>,
}

#[derive(Debug, Serialize, Clone)]
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
    tool_type: ToolType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeHistorySessionRequest {
    session_id: String,
    tool_type: ToolType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteHistorySessionRequest {
    path: String,
    tool_type: ToolType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteHistoryProviderRequest {
    provider: String,
    tool_type: ToolType,
}

#[derive(Clone)]
struct SessionParseResult {
    session_id: String,
    provider: String,
    tool_type: ToolType,
    title: String,
    timestamp: Option<i64>,
    path: String,
    message_count: usize,
    messages: Vec<HistoryMessage>,
}

#[derive(Clone)]
struct CachedSummary {
    len: u64,
    modified_millis: u128,
    result: Option<SessionParseResult>,
}

static SUMMARY_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedSummary>>> = OnceLock::new();

const MAX_SESSIONS: usize = 200;

#[tauri::command]
pub async fn list_tool_history_sessions(tool_type: ToolType) -> Result<Vec<HistoryProviderGroup>, String> {
    run_background_task("codex-list-tool-history-sessions", move || {
        list_history_sessions_inner(tool_type)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn read_history_session(
    input: ReadHistorySessionRequest,
) -> Result<HistoryConversation, String> {
    run_background_task("codex-read-history-session", move || {
        read_history_session_inner(&input.path, input.tool_type)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn resume_history_session(input: ResumeHistorySessionRequest) -> Result<(), String> {
    run_background_task("codex-resume-history-session", move || {
        resume_history_session_inner(&input.session_id, input.tool_type)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn delete_history_session(
    input: DeleteHistorySessionRequest,
) -> Result<DeleteHistoryResponse, String> {
    run_background_task("codex-delete-history-session", move || {
        delete_history_session_inner(&input.path, input.tool_type)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn delete_history_provider(
    input: DeleteHistoryProviderRequest,
) -> Result<DeleteHistoryResponse, String> {
    run_background_task("codex-delete-history-provider", move || {
        delete_history_provider_inner(&input.provider, input.tool_type)
    })
    .await
    .map_err(|error| error.to_string())
}

fn list_history_sessions_inner(tool_type: ToolType) -> Result<Vec<HistoryProviderGroup>> {
    let root = sessions_root(tool_type)?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut session_files = Vec::new();
    collect_session_files(&root, &mut session_files)?;
    prune_summary_cache(&session_files);

    session_files.sort_by(|left, right| {
        file_modified_millis(right)
            .cmp(&file_modified_millis(left))
            .then_with(|| left.cmp(right))
    });

    let mut all_sessions = Vec::<HistorySessionSummary>::new();
    for session_file in &session_files {
        if let Some(summary) = parse_session_file_cached(session_file, tool_type)? {
            all_sessions.push(summary);
            if all_sessions.len() >= MAX_SESSIONS {
                break;
            }
        }
    }

    all_sessions.sort_by(|left, right| {
        right
            .timestamp
            .unwrap_or_default()
            .cmp(&left.timestamp.unwrap_or_default())
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
    });

    let mut groups = BTreeMap::<String, Vec<HistorySessionSummary>>::new();
    for summary in all_sessions {
        groups
            .entry(summary.provider.clone())
            .or_default()
            .push(summary);
    }

    let mut grouped = groups
        .into_iter()
        .map(|(provider, sessions)| HistoryProviderGroup {
            provider,
            tool_type,
            sessions,
        })
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

fn parse_session_file_cached(
    session_file: &Path,
    tool_type: ToolType,
) -> Result<Option<HistorySessionSummary>> {
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
        return Ok(cached.result.as_ref().map(summary_from_parse_result));
    }

    let parse_result = parse_session_file(session_file, tool_type)?;
    let summary = parse_result.as_ref().map(summary_from_parse_result);
    if let Ok(mut items) = cache.lock() {
        items.insert(
            cache_key,
            CachedSummary {
                len,
                modified_millis,
                result: parse_result,
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

fn file_modified_millis(path: &Path) -> u128 {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(system_time_millis)
        .unwrap_or_default()
}

fn read_history_session_inner(path: &str, tool_type: ToolType) -> Result<HistoryConversation> {
    let session_file = validated_session_file(path, tool_type)?;

    let summary = parse_session_file_cached(&session_file, tool_type)?.unwrap_or_else(|| {
        let session_id = session_id_from_file(&session_file);
        HistorySessionSummary {
            session_id,
            provider: "unknown".to_string(),
            tool_type,
            title: "未命名会话".to_string(),
            timestamp: None,
            path: session_file.display().to_string(),
            message_count: 0,
        }
    });
    let messages = read_history_messages_cached(&session_file, tool_type)?;

    Ok(HistoryConversation {
        session_id: summary.session_id,
        provider: summary.provider,
        tool_type: summary.tool_type,
        title: summary.title,
        timestamp: summary.timestamp,
        path: summary.path,
        messages,
    })
}

fn resume_history_session_inner(session_id: &str, tool_type: ToolType) -> Result<()> {
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

    open_resume_session(session_id, tool_type)
}

fn open_resume_session(session_id: &str, tool_type: ToolType) -> Result<()> {
    match tool_type {
        ToolType::Codex => open_codex_command_in_terminal(&["resume", session_id])
            .with_context(|| format!("无法恢复 Codex 会话: {session_id}"))?,
        ToolType::Claude => open_claude_command_in_terminal(&["--resume", session_id])
            .with_context(|| format!("无法恢复 Claude 会话: {session_id}"))?,
    }

    Ok(())
}

fn delete_history_session_inner(path: &str, tool_type: ToolType) -> Result<DeleteHistoryResponse> {
    let session_file = validated_session_file(path, tool_type)?;
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

fn delete_history_provider_inner(provider: &str, tool_type: ToolType) -> Result<DeleteHistoryResponse> {
    let provider = provider.trim();
    if provider.is_empty() {
        bail!("Provider 不能为空");
    }

    let groups = list_history_sessions_inner(tool_type)?;
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
        let session_file = validated_session_file(&session.path, tool_type)?;
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

fn sessions_root(tool_type: ToolType) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to locate the home directory")?;
    Ok(match tool_type {
        ToolType::Codex => home.join(".codex").join("sessions"),
        ToolType::Claude => home.join(".claude").join("projects"),
    })
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

fn parse_session_file(
    session_file: &Path,
    tool_type: ToolType,
) -> Result<Option<SessionParseResult>> {
    let file = File::open(session_file)
        .with_context(|| format!("无法读取会话文件: {}", session_file.display()))?;
    let reader = BufReader::new(file);

    let mut session_id = session_id_from_file(session_file);
    let mut provider: Option<String> = None;
    let mut title: Option<String> = None;
    let mut timestamp: Option<i64> = None;
    let mut raw_messages: Vec<HistoryMessage> = Vec::new();

    for line in reader.lines() {
        let line = line.with_context(|| format!("无法读取 JSONL 行: {}", session_file.display()))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        match tool_type {
            ToolType::Codex => {
                if string_field(&value, "type").as_deref() == Some("session_meta") {
                    let payload = value.get("payload").unwrap_or(&Value::Null);
                    if is_internal_subagent_session(payload) {
                        return Ok(None);
                    }
                    if provider.is_none() {
                        provider = string_field(payload, "model_provider")
                            .filter(|item| !item.trim().is_empty());
                    }
                    if timestamp.is_none() {
                        timestamp = timestamp_field(payload, "timestamp")
                            .or_else(|| timestamp_field(&value, "timestamp"));
                    }
                    if let Some(id) = string_field(payload, "id") {
                        if !id.trim().is_empty() {
                            session_id = id;
                        }
                    }
                } else if timestamp.is_none() {
                    timestamp = timestamp_field(&value, "timestamp");
                }
            }
            ToolType::Claude => {
                if value.get("isSidechain").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                if provider.is_none() {
                    provider = claude_provider_label_from_value(&value);
                }
                if timestamp.is_none() {
                    timestamp = timestamp_field(&value, "timestamp");
                }
                if let Some(id) = string_field(&value, "sessionId") {
                    if !id.trim().is_empty() {
                        session_id = id;
                    }
                }
            }
        }

        let messages = messages_from_value(&value, tool_type);
        if title.is_none() {
            title = messages
                .iter()
                .find(|message| message.role.eq_ignore_ascii_case("user") && !message.content.trim().is_empty())
                .map(|message| title_from_content(&message.content));
        }
        raw_messages.extend(messages);
    }

    let messages = if tool_type == ToolType::Claude {
        merge_consecutive_messages(raw_messages)
    } else {
        raw_messages
    };
    let message_count = messages.len();

    let provider = match tool_type {
        ToolType::Codex => provider,
        ToolType::Claude => provider.or_else(|| claude_provider_label_from_path(session_file)),
    };
    let Some(provider) = provider else {
        return Ok(None);
    };

    Ok(Some(SessionParseResult {
        session_id,
        provider,
        tool_type,
        title: title.unwrap_or_else(|| "未命名会话".to_string()),
        timestamp,
        path: session_file.display().to_string(),
        message_count,
        messages,
    }))
}

fn summary_from_parse_result(result: &SessionParseResult) -> HistorySessionSummary {
    HistorySessionSummary {
        session_id: result.session_id.clone(),
        provider: result.provider.clone(),
        tool_type: result.tool_type,
        title: result.title.clone(),
        timestamp: result.timestamp,
        path: result.path.clone(),
        message_count: result.message_count,
    }
}

fn is_internal_subagent_session(payload: &Value) -> bool {
    string_field(payload, "thread_source").as_deref() == Some("subagent")
        || payload.get("source").and_then(|source| source.get("subagent")).is_some()
}

fn claude_provider_label_from_value(value: &Value) -> Option<String> {
    string_field(value, "cwd")
        .and_then(|cwd| {
            Path::new(&cwd)
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToString::to_string)
        })
        .filter(|name| !name.trim().is_empty())
}

fn claude_provider_label_from_path(session_file: &Path) -> Option<String> {
    session_file
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(|name| name.trim_matches('-').to_string())
        .filter(|name| !name.trim().is_empty())
        .or_else(|| Some("claude".to_string()))
}

#[cfg(test)]
fn read_history_messages(session_file: &Path, tool_type: ToolType) -> Result<Vec<HistoryMessage>> {
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
        if tool_type == ToolType::Claude
            && value.get("isSidechain").and_then(Value::as_bool) == Some(true)
        {
            continue;
        }
        messages.extend(messages_from_value(&value, tool_type));
    }

    if tool_type == ToolType::Claude {
        messages = merge_consecutive_messages(messages);
    }

    Ok(messages)
}

fn read_history_messages_cached(
    session_file: &Path,
    tool_type: ToolType,
) -> Result<Vec<HistoryMessage>> {
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
        if let Some(result) = cached.result {
            return Ok(result.messages);
        }
    }

    let parse_result = parse_session_file(session_file, tool_type)?;
    let messages = parse_result
        .as_ref()
        .map(|r| r.messages.clone())
        .unwrap_or_default();
    if let Ok(mut items) = cache.lock() {
        items.insert(
            cache_key,
            CachedSummary {
                len,
                modified_millis,
                result: parse_result,
            },
        );
    }
    Ok(messages)
}

fn messages_from_value(value: &Value, tool_type: ToolType) -> Vec<HistoryMessage> {
    match tool_type {
        ToolType::Codex => codex_messages_from_value(value),
        ToolType::Claude => claude_messages_from_value(value),
    }
}

fn codex_messages_from_value(value: &Value) -> Vec<HistoryMessage> {
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

fn claude_messages_from_value(value: &Value) -> Vec<HistoryMessage> {
    let top_type = string_field(value, "type").unwrap_or_default();
    if top_type != "user" && top_type != "assistant" {
        return Vec::new();
    }

    let message = value.get("message").unwrap_or(value);
    let role = string_field(message, "role").unwrap_or(top_type);
    if role != "user" && role != "assistant" {
        return Vec::new();
    }

    let content = message
        .get("content")
        .map(claude_content_to_string)
        .or_else(|| string_field(value, "content"))
        .unwrap_or_default();
    if content.trim().is_empty() {
        return Vec::new();
    }

    vec![HistoryMessage { role, content }]
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
        Value::Object(map) => {
            let content_type = map.get("type").and_then(Value::as_str).unwrap_or_default();
            ["text", "content", "value"]
                .iter()
                .find_map(|key| map.get(*key).map(content_to_string))
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| {
                    if content_type.is_empty() {
                        serde_json::to_string(value).unwrap_or_default()
                    } else {
                        String::new()
                    }
                })
        }
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn claude_content_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(claude_content_block_to_string)
            .filter(|item| !item.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(_) => claude_content_block_to_string(value),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn claude_content_block_to_string(value: &Value) -> String {
    let Value::Object(map) = value else {
        return content_to_string(value);
    };

    let block_type = map.get("type").and_then(Value::as_str).unwrap_or_default();
    match block_type {
        "text" => map
            .get("text")
            .map(content_to_string)
            .unwrap_or_default(),
        "thinking" => String::new(),
        "tool_use" => {
            let tool_name = map.get("name").and_then(Value::as_str).unwrap_or("unknown");
            let input = map.get("input").cloned().unwrap_or(Value::Null);
            let summary = match tool_name {
                "Read" | "read_file" => {
                    let path = input.get("file_path").and_then(Value::as_str).unwrap_or("?");
                    format!("[读取] {path}")
                }
                "Edit" | "Write" | "edit_file" | "write_file" => {
                    let path = input.get("file_path").and_then(Value::as_str).unwrap_or("?");
                    format!("[编辑] {path}")
                }
                "Bash" | "bash" | "execute_command" => {
                    let cmd = input.get("command").and_then(Value::as_str).unwrap_or("?");
                    let truncated = if cmd.chars().count() > 60 {
                        format!("{}...", cmd.chars().take(60).collect::<String>())
                    } else {
                        cmd.to_string()
                    };
                    format!("[终端] {truncated}")
                }
                "Grep" | "Glob" | "grep" | "glob" => {
                    let pattern = input
                        .get("pattern")
                        .or_else(|| input.get("query"))
                        .and_then(Value::as_str)
                        .unwrap_or("?");
                    format!("[搜索] {pattern}")
                }
                "WebFetch" | "WebSearch" => {
                    let url_or_query = input
                        .get("url")
                        .or_else(|| input.get("query"))
                        .and_then(Value::as_str)
                        .unwrap_or("?");
                    format!("[网络] {url_or_query}")
                }
                "TodoWrite" => "[任务列表]".to_string(),
                "AskUserQuestion" => "[提问]".to_string(),
                _ => format!("[工具] {tool_name}"),
            };
            summary
        }
        "tool_result" => {
            let result_content = map.get("content").unwrap_or(&Value::Null);
            let text = match result_content {
                Value::String(s) => s.clone(),
                Value::Array(items) => items
                    .iter()
                    .filter_map(|item| {
                        let obj = item.as_object()?;
                        if obj.get("type").and_then(Value::as_str) == Some("text") {
                            obj.get("text").and_then(Value::as_str).map(ToString::to_string)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            let trimmed = text.trim();
            if trimmed.is_empty() {
                String::new()
            } else if trimmed.chars().count() > 100 {
                format!("[结果] {}...", trimmed.chars().take(100).collect::<String>())
            } else {
                format!("[结果] {trimmed}")
            }
        }
        _ => {
            ["text", "content", "value"]
                .iter()
                .find_map(|key| map.get(*key).map(content_to_string))
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_default()
        }
    }
}

fn merge_consecutive_messages(messages: Vec<HistoryMessage>) -> Vec<HistoryMessage> {
    let mut merged: Vec<HistoryMessage> = Vec::new();
    for message in messages {
        if let Some(last) = merged.last_mut() {
            if last.role == message.role {
                if !last.content.is_empty() && !message.content.is_empty() {
                    last.content.push_str("\n\n");
                }
                last.content.push_str(&message.content);
                continue;
            }
        }
        merged.push(message);
    }
    merged
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

fn validated_session_file(path: &str, tool_type: ToolType) -> Result<PathBuf> {
    let path = path.trim();
    if path.is_empty() {
        bail!("会话路径不能为空");
    }

    let root = fs::canonicalize(sessions_root(tool_type)?)
        .with_context(|| format!("未找到 {} 历史目录", tool_type))?;
    let input = fs::canonicalize(PathBuf::from(path))
        .with_context(|| format!("会话路径不存在: {path}"))?;
    if !input.starts_with(&root) {
        bail!("会话路径不在 {} 历史目录下", tool_type);
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

        let summary = parse_session_file(&path, ToolType::Codex)?.expect("summary");
        assert_eq!(summary.session_id, "session-1");
        assert_eq!(summary.provider, "openai");
        assert_eq!(summary.tool_type, ToolType::Codex);
        assert_eq!(summary.title, "Hello from history");
        assert_eq!(summary.message_count, 2);

        let messages = read_history_messages(&path, ToolType::Codex)?;
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

        assert!(parse_session_file(&path, ToolType::Codex)?.is_none());

        let _ = fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn summarizes_claude_history_jsonl_sessions() -> Result<()> {
        let path = temp_session_file("claude-summary");
        fs::write(
            &path,
            r#"{"type":"queue-operation","operation":"enqueue","timestamp":"2026-06-18T08:38:30.914Z","sessionId":"claude-session-1","content":"ignored"}
{"type":"user","message":{"role":"user","content":"Hello Claude history"},"timestamp":"2026-06-18T08:41:42.237Z","cwd":"/tmp/example-project","sessionId":"claude-session-1"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hidden"},{"type":"text","text":"Hello from Claude"}]},"timestamp":"2026-06-18T08:41:56.034Z","cwd":"/tmp/example-project","sessionId":"claude-session-1"}
{"type":"system","subtype":"turn_duration","timestamp":"2026-06-18T08:41:57.034Z","sessionId":"claude-session-1"}
"#,
        )?;

        let summary = parse_session_file(&path, ToolType::Claude)?.expect("summary");
        assert_eq!(summary.session_id, "claude-session-1");
        assert_eq!(summary.provider, "example-project");
        assert_eq!(summary.tool_type, ToolType::Claude);
        assert_eq!(summary.title, "Hello Claude history");
        assert_eq!(summary.message_count, 2);

        let messages = read_history_messages(&path, ToolType::Claude)?;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Hello from Claude");

        let _ = fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn parses_claude_tool_use_and_tool_result_messages() -> Result<()> {
        let path = temp_session_file("claude-tools");
        fs::write(
            &path,
            r#"{"type":"user","message":{"role":"user","content":"Read my file"},"timestamp":"2026-06-22T10:00:00Z","cwd":"/tmp/my-project","sessionId":"tool-session-1"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me read the file"},{"type":"tool_use","id":"call_1","name":"Read","input":{"file_path":"/tmp/my-project/src/main.rs"}}]},"timestamp":"2026-06-22T10:00:01Z","cwd":"/tmp/my-project","sessionId":"tool-session-1"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"call_2","name":"Bash","input":{"command":"cargo test"}}]},"timestamp":"2026-06-22T10:00:02Z","cwd":"/tmp/my-project","sessionId":"tool-session-1"}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_1","content":"fn main() {}"}]},"timestamp":"2026-06-22T10:00:03Z","cwd":"/tmp/my-project","sessionId":"tool-session-1"}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_2","content":"test result: ok. 3 passed"}]},"timestamp":"2026-06-22T10:00:04Z","cwd":"/tmp/my-project","sessionId":"tool-session-1"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"All tests passed!"}]},"timestamp":"2026-06-22T10:00:05Z","cwd":"/tmp/my-project","sessionId":"tool-session-1"}
"#,
        )?;

        let summary = parse_session_file(&path, ToolType::Claude)?.expect("summary");
        assert_eq!(summary.session_id, "tool-session-1");
        assert_eq!(summary.provider, "my-project");
        assert_eq!(summary.title, "Read my file");
        // After merge: user, assistant(thinking dropped + 2 tool_use), user(2 tool_result), assistant(text) = 4
        assert_eq!(summary.message_count, 4);

        let messages = read_history_messages(&path, ToolType::Claude)?;
        assert_eq!(messages.len(), 4);

        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Read my file");

        assert_eq!(messages[1].role, "assistant");
        assert!(messages[1].content.contains("[读取] /tmp/my-project/src/main.rs"));
        assert!(messages[1].content.contains("[终端] cargo test"));

        assert_eq!(messages[2].role, "user");
        assert!(messages[2].content.contains("[结果] fn main()"));
        assert!(messages[2].content.contains("[结果] test result: ok. 3 passed"));

        assert_eq!(messages[3].role, "assistant");
        assert_eq!(messages[3].content, "All tests passed!");

        let _ = fs::remove_file(path);
        Ok(())
    }
}
