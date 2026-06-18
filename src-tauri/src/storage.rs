use aes::Aes256;
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use rusqlite::{params_from_iter, types::Value as SqliteValue, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

// Keep legacy filenames so existing installs keep their saved providers/tokens after the app rename.
const DATABASE_FILE_NAME: &str = "codex-config-desktop.sqlite3";
const TOKEN_KEY_FILE_NAME: &str = "codex-config-desktop.key";
const TOKEN_PREFIX: &str = "enc:v1:";
static INITIALIZED_DATABASES: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolType {
    Codex,
    Claude,
}

impl ToolType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolType::Codex => "codex",
            ToolType::Claude => "claude",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_lowercase().as_str() {
            "codex" => Ok(ToolType::Codex),
            "claude" => Ok(ToolType::Claude),
            other => bail!("unknown tool type: {other}"),
        }
    }
}

impl fmt::Display for ToolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Default for ToolType {
    fn default() -> Self {
        ToolType::Codex
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    id: String,
    name: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    models: Vec<String>,
    token_present: bool,
    tool_type: ToolType,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredProvider {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) base_url: String,
    pub(crate) model: Option<String>,
    pub(crate) wire_api: String,
    pub(crate) requires_open_ai_auth: bool,
    pub(crate) token: Option<String>,
    pub(crate) tool_type: ToolType,
}

fn database_path(config_path: &Path) -> Result<PathBuf> {
    let config_dir = config_path
        .parent()
        .context("failed to locate config directory")?;
    Ok(config_dir.join(DATABASE_FILE_NAME))
}

pub(crate) fn open_database(config_path: &Path) -> Result<SqliteConnection> {
    let db_path = database_path(config_path)?;
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let conn = SqliteConnection::open(&db_path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    initialize_database_once(&conn, config_path, &db_path)?;
    Ok(conn)
}

fn initialize_database_once(
    conn: &SqliteConnection,
    config_path: &Path,
    db_path: &Path,
) -> Result<()> {
    let initialized = INITIALIZED_DATABASES.get_or_init(|| Mutex::new(HashSet::new()));
    let mut initialized = initialized
        .lock()
        .map_err(|_| anyhow!("database initialization lock is poisoned"))?;
    if initialized.contains(db_path) {
        return Ok(());
    }

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS providers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            base_url TEXT NOT NULL,
            model TEXT,
            wire_api TEXT NOT NULL DEFAULT 'responses',
            requires_openai_auth INTEGER NOT NULL DEFAULT 1,
            token TEXT,
            position INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS provider_models (
            provider_id TEXT NOT NULL,
            model TEXT NOT NULL,
            position INTEGER NOT NULL,
            PRIMARY KEY (provider_id, model),
            FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )?;
    if !table_has_column(conn, "providers", "model")? {
        conn.execute_batch("ALTER TABLE providers ADD COLUMN model TEXT;")?;
    }
    if !table_has_column(conn, "providers", "tool_type")? {
        conn.execute_batch(
            "ALTER TABLE providers ADD COLUMN tool_type TEXT NOT NULL DEFAULT 'codex';",
        )?;
    }
    if !table_has_column(conn, "providers", "position")? {
        conn.execute_batch("ALTER TABLE providers ADD COLUMN position INTEGER NOT NULL DEFAULT 0;")?;
    }
    normalize_provider_positions(conn)?;
    encrypt_plaintext_tokens(conn, config_path)?;
    restrict_file_permissions(db_path)?;
    initialized.insert(db_path.to_path_buf());
    Ok(())
}

pub(crate) fn collect_providers_from_database(
    conn: &SqliteConnection,
    tool_type: ToolType,
) -> Result<Vec<ProviderView>> {
    let tool_type_str = tool_type.as_str();
    let provider_models = get_provider_models_for_tool(conn, tool_type)?;
    conn.query_all(
        r#"
        SELECT
            id,
            name,
            base_url,
            model,
            token IS NOT NULL AND length(trim(token)) > 0 AS token_present,
            tool_type
        FROM providers
        WHERE tool_type = ?1
        ORDER BY position, lower(name), lower(id)
        "#,
        &[SqlValue::Text(tool_type_str)],
        |row| {
            let id = row.string(0)?;
            let tool_type_str = row.string(5)?;
            Ok(ProviderView {
                id: id.clone(),
                name: Some(row.string(1)?),
                base_url: Some(row.string(2)?),
                model: row.optional_string(3)?,
                models: provider_models.get(&id).cloned().unwrap_or_default(),
                token_present: row.i64(4)? != 0,
                tool_type: ToolType::from_str(&tool_type_str).unwrap_or_default(),
            })
        },
    )
}

pub(crate) fn list_stored_providers(
    conn: &SqliteConnection,
    config_path: &Path,
    tool_type: ToolType,
) -> Result<Vec<StoredProvider>> {
    let tool_type_str = tool_type.as_str();
    let mut providers = conn.query_all(
        r#"
        SELECT id, name, base_url, model, wire_api, requires_openai_auth, token, tool_type
        FROM providers
        WHERE tool_type = ?1
        ORDER BY position, lower(name), lower(id)
        "#,
        &[SqlValue::Text(tool_type_str)],
        stored_provider_from_row,
    )?;
    for provider in &mut providers {
        provider.token = decrypt_optional_token(config_path, provider.token.as_deref())?;
    }
    Ok(providers)
}

pub(crate) fn get_provider(
    conn: &SqliteConnection,
    config_path: &Path,
    provider_id: &str,
) -> Result<Option<StoredProvider>> {
    let mut provider = conn.query_one(
        r#"
        SELECT id, name, base_url, model, wire_api, requires_openai_auth, token, tool_type
        FROM providers
        WHERE id = ?1
        "#,
        &[SqlValue::Text(provider_id)],
        stored_provider_from_row,
    )?;
    if let Some(provider) = &mut provider {
        provider.token = decrypt_optional_token(config_path, provider.token.as_deref())?;
    }
    Ok(provider)
}

pub(crate) fn get_provider_token(
    conn: &SqliteConnection,
    config_path: &Path,
    provider_id: &str,
) -> Result<Option<String>> {
    conn.query_one(
        "SELECT token FROM providers WHERE id = ?1",
        &[SqlValue::Text(provider_id)],
        |row| row.optional_string(0),
    )
    .map(Option::flatten)
    .and_then(|token| decrypt_optional_token(config_path, token.as_deref()))
}

fn encrypt_plaintext_tokens(conn: &SqliteConnection, config_path: &Path) -> Result<()> {
    let tokens = conn.query_all(
        r#"
        SELECT id, token
        FROM providers
        WHERE token IS NOT NULL AND length(trim(token)) > 0
        "#,
        &[],
        |row| Ok((row.string(0)?, row.string(1)?)),
    )?;

    for (provider_id, token) in tokens {
        if token.starts_with(TOKEN_PREFIX) {
            continue;
        }
        let encrypted = encrypt_token(config_path, &token)?;
        conn.execute(
            "UPDATE providers SET token = ?1 WHERE id = ?2",
            &[SqlValue::Text(&encrypted), SqlValue::Text(&provider_id)],
        )?;
    }

    Ok(())
}

pub(crate) fn get_provider_models(conn: &SqliteConnection, provider_id: &str) -> Result<Vec<String>> {
    conn.query_all(
        r#"
        SELECT model
        FROM provider_models
        WHERE provider_id = ?1
        ORDER BY position, model
        "#,
        &[SqlValue::Text(provider_id)],
        |row| row.string(0),
    )
}

fn get_provider_models_for_tool(
    conn: &SqliteConnection,
    tool_type: ToolType,
) -> Result<HashMap<String, Vec<String>>> {
    let rows = conn.query_all(
        r#"
        SELECT pm.provider_id, pm.model
        FROM provider_models pm
        INNER JOIN providers p ON p.id = pm.provider_id
        WHERE p.tool_type = ?1
        ORDER BY pm.provider_id, pm.position, pm.model
        "#,
        &[SqlValue::Text(tool_type.as_str())],
        |row| Ok((row.string(0)?, row.string(1)?)),
    )?;
    let mut models = HashMap::<String, Vec<String>>::new();
    for (provider_id, model) in rows {
        models.entry(provider_id).or_default().push(model);
    }
    Ok(models)
}

pub(crate) fn replace_provider_models(
    conn: &SqliteConnection,
    provider_id: &str,
    models: &[String],
) -> Result<()> {
    conn.execute(
        "DELETE FROM provider_models WHERE provider_id = ?1",
        &[SqlValue::Text(provider_id)],
    )?;

    for (index, model) in models.iter().enumerate() {
        conn.execute(
            r#"
            INSERT INTO provider_models (provider_id, model, position)
            VALUES (?1, ?2, ?3)
            "#,
            &[
                SqlValue::Text(provider_id),
                SqlValue::Text(model),
                SqlValue::I64(index as i64),
            ],
        )?;
    }

    Ok(())
}

pub(crate) fn copy_provider_models(conn: &SqliteConnection, source_id: &str, target_id: &str) -> Result<()> {
    let models = get_provider_models(conn, source_id)?;
    replace_provider_models(conn, target_id, &models)
}

pub(crate) fn next_provider_position(conn: &SqliteConnection, tool_type: ToolType) -> Result<i64> {
    conn.query_one(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM providers WHERE tool_type = ?1",
        &[SqlValue::Text(tool_type.as_str())],
        |row| row.i64(0),
    )
    .map(|value| value.unwrap_or(0))
}

pub(crate) fn reorder_provider_positions(
    conn: &SqliteConnection,
    tool_type: ToolType,
    provider_ids: &[String],
) -> Result<()> {
    let tool_type_str = tool_type.as_str();
    let current_ids = provider_ids_for_tool(conn, tool_type_str)?;
    let current_set = current_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let requested_set = provider_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    if current_ids.len() != provider_ids.len() || current_set != requested_set {
        bail!("配置列表已变化，请刷新后再排序");
    }

    with_transaction(conn, |conn| {
        for (index, provider_id) in provider_ids.iter().enumerate() {
            conn.execute(
                "UPDATE providers SET position = ?1 WHERE id = ?2 AND tool_type = ?3",
                &[
                    SqlValue::I64(index as i64),
                    SqlValue::Text(provider_id),
                    SqlValue::Text(tool_type_str),
                ],
            )?;
        }
        Ok(())
    })
}

fn normalize_provider_positions(conn: &SqliteConnection) -> Result<()> {
    let tool_types = conn.query_all(
        "SELECT DISTINCT tool_type FROM providers ORDER BY tool_type",
        &[],
        |row| row.string(0),
    )?;

    for tool_type in tool_types {
        let provider_ids = provider_ids_for_tool(conn, &tool_type)?;
        for (index, provider_id) in provider_ids.iter().enumerate() {
            conn.execute(
                "UPDATE providers SET position = ?1 WHERE id = ?2 AND tool_type = ?3",
                &[
                    SqlValue::I64(index as i64),
                    SqlValue::Text(provider_id),
                    SqlValue::Text(&tool_type),
                ],
            )?;
        }
    }

    Ok(())
}

fn provider_ids_for_tool(conn: &SqliteConnection, tool_type: &str) -> Result<Vec<String>> {
    conn.query_all(
        r#"
        SELECT id
        FROM providers
        WHERE tool_type = ?1
        ORDER BY position, lower(name), lower(id)
        "#,
        &[SqlValue::Text(tool_type)],
        |row| row.string(0),
    )
}

fn table_has_column(conn: &SqliteConnection, table: &str, column: &str) -> Result<bool> {
    let sql = format!("PRAGMA table_info({})", sqlite_identifier(table)?);
    let columns = conn.query_all(&sql, &[], |row| row.string(1))?;
    Ok(columns.iter().any(|item| item == column))
}

pub(crate) fn next_provider_id(
    conn: &SqliteConnection,
    name: &str,
    base_url: &str,
    skip_id: Option<&str>,
) -> Result<String> {
    let base = slugify_provider_id(name)
        .or_else(|| infer_provider_id_from_url(base_url))
        .unwrap_or_else(|| "provider".to_string());
    let mut candidate = base.clone();
    let mut index = 2;
    loop {
        if Some(candidate.as_str()) == skip_id || !provider_exists(conn, &candidate)? {
            return Ok(candidate);
        }
        candidate = format!("{base}-{index}");
        index += 1;
    }
}

fn slugify_provider_id(input: &str) -> Option<String> {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for character in input.trim().chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() || character == '_' {
            slug.push(character);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

fn infer_provider_id_from_url(base_url: &str) -> Option<String> {
    let without_scheme = base_url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = without_scheme
        .split('/')
        .next()
        .unwrap_or_default()
        .trim_start_matches("www.");
    let parts = host
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let candidate = if parts.len() > 1 {
        parts.get(parts.len() - 2).copied()
    } else {
        parts.first().copied()
    };
    candidate.and_then(slugify_provider_id)
}

pub(crate) fn optional_non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn sqlite_identifier(identifier: &str) -> Result<String> {
    if identifier
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        Ok(format!("\"{identifier}\""))
    } else {
        bail!("invalid SQLite identifier: {identifier}")
    }
}

fn stored_provider_from_row(row: &SqliteRow<'_>) -> Result<StoredProvider> {
    let tool_type_str = row.string(7)?;
    Ok(StoredProvider {
        id: row.string(0)?,
        name: row.string(1)?,
        base_url: row.string(2)?,
        model: row.optional_string(3)?,
        wire_api: row.string(4)?,
        requires_open_ai_auth: row.i64(5)? != 0,
        token: row.optional_string(6)?,
        tool_type: ToolType::from_str(&tool_type_str).unwrap_or_default(),
    })
}

pub(crate) fn next_copy_id(conn: &SqliteConnection, source_id: &str) -> Result<String> {
    let base = format!("{source_id}-copy");
    let mut candidate = base.clone();
    let mut index = 2;

    while provider_exists(conn, &candidate)? {
        candidate = format!("{base}-{index}");
        index += 1;
    }

    Ok(candidate)
}

pub(crate) fn next_copy_name(conn: &SqliteConnection, source_name: &str) -> Result<String> {
    let base = strip_copy_suffix(source_name.trim()).trim();
    let base = if base.is_empty() { "provider" } else { base };
    let mut index = 1;

    loop {
        let candidate = format!("{base} copy{index}");
        if !provider_name_exists(conn, &candidate)? {
            return Ok(candidate);
        }
        index += 1;
    }
}

fn strip_copy_suffix(name: &str) -> &str {
    let Some((prefix, suffix)) = name.rsplit_once(" copy") else {
        return name;
    };

    if !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit()) {
        prefix
    } else {
        name
    }
}

fn provider_name_exists(conn: &SqliteConnection, provider_name: &str) -> Result<bool> {
    let count = conn
        .query_one(
            "SELECT COUNT(*) FROM providers WHERE lower(name) = lower(?1)",
            &[SqlValue::Text(provider_name)],
            |row| row.i64(0),
        )?
        .unwrap_or(0);
    Ok(count > 0)
}

pub(crate) fn provider_exists(conn: &SqliteConnection, provider_id: &str) -> Result<bool> {
    let count = conn
        .query_one(
            "SELECT COUNT(*) FROM providers WHERE id = ?1",
            &[SqlValue::Text(provider_id)],
            |row| row.i64(0),
        )?
        .unwrap_or(0);
    Ok(count > 0)
}

pub(crate) fn get_active_tool(conn: &SqliteConnection) -> Result<ToolType> {
    let value = get_setting(conn, "active_tool")?;
    match value.as_deref() {
        Some(s) => Ok(ToolType::from_str(s).unwrap_or_default()),
        None => Ok(ToolType::default()),
    }
}

pub(crate) fn set_active_tool(conn: &SqliteConnection, tool_type: ToolType) -> Result<()> {
    set_setting(conn, "active_tool", tool_type.as_str())
}

pub(crate) fn get_active_provider(conn: &SqliteConnection, tool_type: ToolType) -> Result<Option<String>> {
    if let Some(provider_id) = get_tool_setting(conn, "active_provider", tool_type)? {
        return Ok(Some(provider_id));
    }

    let legacy = get_setting(conn, "active_provider")?;
    match legacy {
        Some(provider_id) if provider_belongs_to_tool(conn, &provider_id, tool_type)? => {
            Ok(Some(provider_id))
        }
        _ => Ok(None),
    }
}

pub(crate) fn get_active_model(conn: &SqliteConnection, tool_type: ToolType) -> Result<Option<String>> {
    if let Some(model) = get_tool_setting(conn, "active_model", tool_type)? {
        return Ok(Some(model));
    }

    let legacy_provider = get_setting(conn, "active_provider")?;
    let legacy_model = get_setting(conn, "active_model")?;
    match (legacy_provider, legacy_model) {
        (Some(provider_id), Some(model)) if provider_belongs_to_tool(conn, &provider_id, tool_type)? => {
            Ok(Some(model))
        }
        _ => Ok(None),
    }
}

pub(crate) fn set_active_provider(conn: &SqliteConnection, tool_type: ToolType, provider_id: &str) -> Result<()> {
    set_tool_setting(conn, "active_provider", tool_type, provider_id)
}

pub(crate) fn set_active_model(conn: &SqliteConnection, tool_type: ToolType, model: &str) -> Result<()> {
    set_tool_setting(conn, "active_model", tool_type, model)
}

pub(crate) fn delete_active_settings(conn: &SqliteConnection, tool_type: ToolType) -> Result<()> {
    delete_setting(conn, &tool_setting_key("active_provider", tool_type))?;
    delete_setting(conn, &tool_setting_key("active_model", tool_type))?;
    Ok(())
}

fn get_tool_setting(conn: &SqliteConnection, key: &str, tool_type: ToolType) -> Result<Option<String>> {
    get_setting(conn, &tool_setting_key(key, tool_type))
}

fn set_tool_setting(conn: &SqliteConnection, key: &str, tool_type: ToolType, value: &str) -> Result<()> {
    set_setting(conn, &tool_setting_key(key, tool_type), value)
}

fn tool_setting_key(key: &str, tool_type: ToolType) -> String {
    format!("{}_{}", key, tool_type.as_str())
}

pub(crate) fn provider_belongs_to_tool(conn: &SqliteConnection, provider_id: &str, tool_type: ToolType) -> Result<bool> {
    let count = conn
        .query_one(
            "SELECT COUNT(*) FROM providers WHERE id = ?1 AND tool_type = ?2",
            &[SqlValue::Text(provider_id), SqlValue::Text(tool_type.as_str())],
            |row| row.i64(0),
        )?
        .unwrap_or(0);
    Ok(count > 0)
}

pub(crate) fn get_setting(conn: &SqliteConnection, key: &str) -> Result<Option<String>> {
    conn.query_one(
        "SELECT value FROM settings WHERE key = ?1",
        &[SqlValue::Text(key)],
        |row| row.string(0),
    )
}

pub(crate) fn set_setting(conn: &SqliteConnection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO settings (key, value)
        VALUES (?1, ?2)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value
        "#,
        &[SqlValue::Text(key), SqlValue::Text(value)],
    )
}

pub(crate) fn delete_setting(conn: &SqliteConnection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", &[SqlValue::Text(key)])
}

pub(crate) fn with_transaction<T>(
    conn: &SqliteConnection,
    operation: impl FnOnce(&SqliteConnection) -> Result<T>,
) -> Result<T> {
    conn.execute_batch("BEGIN IMMEDIATE;")?;
    match operation(conn) {
        Ok(value) => {
            conn.execute_batch("COMMIT;")?;
            Ok(value)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(error)
        }
    }
}

fn token_key_path(config_path: &Path) -> Result<PathBuf> {
    let config_dir = config_path
        .parent()
        .context("failed to locate config directory")?;
    Ok(config_dir.join(TOKEN_KEY_FILE_NAME))
}

pub(crate) fn encrypt_optional_token(config_path: &Path, token: Option<&str>) -> Result<Option<String>> {
    token
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| encrypt_token(config_path, token))
        .transpose()
}

fn decrypt_optional_token(config_path: &Path, token: Option<&str>) -> Result<Option<String>> {
    token
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| decrypt_token(config_path, token))
        .transpose()
}

fn encrypt_token(config_path: &Path, token: &str) -> Result<String> {
    let key = load_or_create_token_key(config_path)?;
    let mut iv = [0_u8; 16];
    getrandom::fill(&mut iv).map_err(|error| anyhow!("failed to generate token iv: {error}"))?;

    let ciphertext = Aes256CbcEnc::new((&key[..]).into(), (&iv[..]).into())
        .encrypt_padded_vec_mut::<Pkcs7>(token.as_bytes());
    let mut encoded = Vec::with_capacity(iv.len() + ciphertext.len());
    encoded.extend_from_slice(&iv);
    encoded.extend_from_slice(&ciphertext);
    Ok(format!("{TOKEN_PREFIX}{}", BASE64.encode(encoded)))
}

fn decrypt_token(config_path: &Path, token: &str) -> Result<String> {
    if !token.starts_with(TOKEN_PREFIX) {
        return Ok(token.to_string());
    }

    let encoded = token.trim_start_matches(TOKEN_PREFIX);
    let decoded = BASE64.decode(encoded).context("token 密文不是有效 base64")?;
    if decoded.len() <= 16 {
        bail!("token 密文格式不完整");
    }

    let key = load_or_create_token_key(config_path)?;
    let iv = &decoded[..16];
    let ciphertext = &decoded[16..];
    let decrypted = Aes256CbcDec::new((&key[..]).into(), iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
        .map_err(|_| anyhow!("token 解密失败"))?;
    String::from_utf8(decrypted).context("token 明文不是有效 UTF-8")
}

fn load_or_create_token_key(config_path: &Path) -> Result<[u8; 32]> {
    let key_path = token_key_path(config_path)?;
    if key_path.exists() {
        let raw = fs::read_to_string(&key_path)
            .with_context(|| format!("failed to read {}", key_path.display()))?;
        let decoded = BASE64
            .decode(raw.trim())
            .with_context(|| format!("failed to decode {}", key_path.display()))?;
        if decoded.len() != 32 {
            bail!("token key 文件格式错误: {}", key_path.display());
        }
        let mut key = [0_u8; 32];
        key.copy_from_slice(&decoded);
        return Ok(key);
    }

    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut key = [0_u8; 32];
    getrandom::fill(&mut key).map_err(|error| anyhow!("failed to generate token key: {error}"))?;
    fs::write(&key_path, format!("{}\n", BASE64.encode(key)))
        .with_context(|| format!("failed to write {}", key_path.display()))?;
    restrict_file_permissions(&key_path)?;
    Ok(key)
}

pub(crate) fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

pub(crate) enum SqlValue<'a> {
    Text(&'a str),
    OptionalText(Option<&'a str>),
    I64(i64),
}

pub(crate) struct SqliteConnection {
    conn: Connection,
}

struct SqliteRow<'row> {
    row: &'row rusqlite::Row<'row>,
}

impl SqliteConnection {
    fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open SQLite database: {}", path.display()))?;
        Ok(Self { conn })
    }

    fn execute_batch(&self, sql: &str) -> Result<()> {
        self.conn
            .execute_batch(sql)
            .context("SQLite execute batch failed")?;
        Ok(())
    }

    pub(crate) fn execute(&self, sql: &str, params: &[SqlValue<'_>]) -> Result<()> {
        let params = sqlite_values(params);
        self.conn
            .execute(sql, params_from_iter(params))
            .context("SQLite execute failed")?;
        Ok(())
    }

    fn query_one<T>(
        &self,
        sql: &str,
        params: &[SqlValue<'_>],
        mapper: impl FnOnce(&SqliteRow<'_>) -> Result<T>,
    ) -> Result<Option<T>> {
        let params = sqlite_values(params);
        let mut statement = self.conn.prepare(sql).context("SQLite prepare failed")?;
        statement
            .query_row(params_from_iter(params), |row| {
                mapper(&SqliteRow { row }).map_err(to_rusqlite_error)
            })
            .optional()
            .context("SQLite query failed")
    }

    fn query_all<T>(
        &self,
        sql: &str,
        params: &[SqlValue<'_>],
        mut mapper: impl FnMut(&SqliteRow<'_>) -> Result<T>,
    ) -> Result<Vec<T>> {
        let params = sqlite_values(params);
        let mut statement = self.conn.prepare(sql).context("SQLite prepare failed")?;
        let mut rows = statement
            .query(params_from_iter(params))
            .context("SQLite query failed")?;
        let mut values = Vec::new();

        while let Some(row) = rows.next().context("SQLite row read failed")? {
            values.push(mapper(&SqliteRow { row })?);
        }

        Ok(values)
    }
}

impl SqliteRow<'_> {
    fn string(&self, index: usize) -> Result<String> {
        self.optional_string(index)?
            .with_context(|| format!("SQLite column {index} is null"))
    }

    fn optional_string(&self, index: usize) -> Result<Option<String>> {
        self.row
            .get::<_, Option<String>>(index)
            .context("SQLite text column is not valid UTF-8")
    }

    fn i64(&self, index: usize) -> Result<i64> {
        self.row
            .get::<_, i64>(index)
            .context("SQLite integer column read failed")
    }
}

fn sqlite_values(params: &[SqlValue<'_>]) -> Vec<SqliteValue> {
    params
        .iter()
        .map(|value| match value {
            SqlValue::Text(value) => SqliteValue::Text((*value).to_string()),
            SqlValue::OptionalText(Some(value)) => SqliteValue::Text((*value).to_string()),
            SqlValue::OptionalText(None) => SqliteValue::Null,
            SqlValue::I64(value) => SqliteValue::Integer(*value),
        })
        .collect()
}

fn to_rusqlite_error(error: anyhow::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        error.to_string(),
    )))
}

#[cfg(unix)]
pub(crate) fn restrict_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn restrict_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_config_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir()
            .join(format!("codesail-test-{name}-{unique}"))
            .join("config.toml")
    }

    fn insert_test_provider(conn: &SqliteConnection, id: &str, name: &str) -> Result<()> {
        insert_test_provider_for_tool(conn, id, name, ToolType::Codex)
    }

    fn insert_test_provider_for_tool(
        conn: &SqliteConnection,
        id: &str,
        name: &str,
        tool_type: ToolType,
    ) -> Result<()> {
        conn.execute(
            r#"
            INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token, tool_type)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            &[
                SqlValue::Text(id),
                SqlValue::Text(name),
                SqlValue::Text("https://api.example.com/v1"),
                SqlValue::OptionalText(Some("gpt-5")),
                SqlValue::Text("responses"),
                SqlValue::I64(1),
                SqlValue::OptionalText(None),
                SqlValue::Text(tool_type.as_str()),
            ],
        )
    }

    #[test]
    fn creates_incrementing_provider_copy_names() -> Result<()> {
        let config_path = temp_config_path("copy-names");
        let conn = open_database(&config_path)?;
        insert_test_provider(&conn, "openai", "OpenAI")?;
        insert_test_provider(&conn, "openai-copy", "OpenAI copy1")?;

        assert_eq!(next_copy_name(&conn, "OpenAI")?, "OpenAI copy2");
        assert_eq!(next_copy_name(&conn, "OpenAI copy1")?, "OpenAI copy2");

        if let Some(parent) = config_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
        Ok(())
    }

    #[test]
    fn creates_provider_ids_unique_across_tools() -> Result<()> {
        let config_path = temp_config_path("provider-id-tools");
        let conn = open_database(&config_path)?;
        insert_test_provider_for_tool(&conn, "openai", "OpenAI", ToolType::Codex)?;

        assert_eq!(
            next_provider_id(&conn, "OpenAI", "https://api.example.com/v1", None)?,
            "openai-2"
        );

        if let Some(parent) = config_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
        Ok(())
    }

    #[test]
    fn reorders_providers_for_the_active_tool() -> Result<()> {
        let config_path = temp_config_path("provider-order");
        let conn = open_database(&config_path)?;
        insert_test_provider(&conn, "alpha", "Alpha")?;
        insert_test_provider(&conn, "beta", "Beta")?;
        insert_test_provider(&conn, "gamma", "Gamma")?;

        reorder_provider_positions(
            &conn,
            ToolType::Codex,
            &["gamma".to_string(), "alpha".to_string(), "beta".to_string()],
        )?;

        let providers = collect_providers_from_database(&conn, ToolType::Codex)?;
        let provider_ids = providers
            .into_iter()
            .map(|provider| provider.id)
            .collect::<Vec<_>>();

        assert_eq!(provider_ids, vec!["gamma", "alpha", "beta"]);
        assert_eq!(next_provider_position(&conn, ToolType::Codex)?, 3);

        if let Some(parent) = config_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
        Ok(())
    }

    #[test]
    fn keeps_active_provider_and_model_separate_per_tool() -> Result<()> {
        let config_path = temp_config_path("active-tool-settings");
        let conn = open_database(&config_path)?;
        insert_test_provider_for_tool(&conn, "codex-provider", "Codex Provider", ToolType::Codex)?;
        insert_test_provider_for_tool(&conn, "claude-provider", "Claude Provider", ToolType::Claude)?;

        set_active_provider(&conn, ToolType::Codex, "codex-provider")?;
        set_active_model(&conn, ToolType::Codex, "gpt-5")?;
        set_active_provider(&conn, ToolType::Claude, "claude-provider")?;
        set_active_model(&conn, ToolType::Claude, "claude-opus-4-7")?;

        assert_eq!(
            get_active_provider(&conn, ToolType::Codex)?.as_deref(),
            Some("codex-provider")
        );
        assert_eq!(get_active_model(&conn, ToolType::Codex)?.as_deref(), Some("gpt-5"));
        assert_eq!(
            get_active_provider(&conn, ToolType::Claude)?.as_deref(),
            Some("claude-provider")
        );
        assert_eq!(
            get_active_model(&conn, ToolType::Claude)?.as_deref(),
            Some("claude-opus-4-7")
        );

        if let Some(parent) = config_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
        Ok(())
    }
}
