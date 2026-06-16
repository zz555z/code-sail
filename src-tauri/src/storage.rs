use aes::Aes256;
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use rusqlite::{params_from_iter, types::Value as SqliteValue, Connection, OptionalExtension};
use serde::Serialize;
use std::{fs, path::{Path, PathBuf}};

// Keep legacy filenames so existing installs keep their saved providers/tokens after the app rename.
const DATABASE_FILE_NAME: &str = "codex-config-desktop.sqlite3";
const TOKEN_KEY_FILE_NAME: &str = "codex-config-desktop.key";
const TOKEN_PREFIX: &str = "enc:v1:";

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    id: String,
    name: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    models: Vec<String>,
    token: Option<String>,
    token_present: bool,
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
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS providers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            base_url TEXT NOT NULL,
            model TEXT,
            wire_api TEXT NOT NULL DEFAULT 'responses',
            requires_openai_auth INTEGER NOT NULL DEFAULT 1,
            token TEXT
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
    if !table_has_column(&conn, "providers", "model")? {
        conn.execute_batch("ALTER TABLE providers ADD COLUMN model TEXT;")?;
    }
    encrypt_plaintext_tokens(&conn, config_path)?;
    restrict_file_permissions(&db_path)?;
    Ok(conn)
}

pub(crate) fn collect_providers_from_database(
    conn: &SqliteConnection,
    config_path: &Path,
) -> Result<Vec<ProviderView>> {
    conn.query_all(
        r#"
        SELECT
            id,
            name,
            base_url,
            model,
            token IS NOT NULL AND length(trim(token)) > 0 AS token_present
        FROM providers
        ORDER BY lower(name), lower(id)
        "#,
        &[],
        |row| {
            let id = row.string(0)?;
            Ok(ProviderView {
                id: id.clone(),
                name: Some(row.string(1)?),
                base_url: Some(row.string(2)?),
                model: row.optional_string(3)?,
                models: get_provider_models(conn, &id)?,
                token: get_provider_token(conn, config_path, &id)?,
                token_present: row.i64(4)? != 0,
            })
        },
    )
}

pub(crate) fn list_stored_providers(
    conn: &SqliteConnection,
    config_path: &Path,
) -> Result<Vec<StoredProvider>> {
    let mut providers = conn.query_all(
        r#"
        SELECT id, name, base_url, model, wire_api, requires_openai_auth, token
        FROM providers
        ORDER BY lower(name), lower(id)
        "#,
        &[],
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
        SELECT id, name, base_url, model, wire_api, requires_openai_auth, token
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

fn get_provider_models(conn: &SqliteConnection, provider_id: &str) -> Result<Vec<String>> {
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
    Ok(StoredProvider {
        id: row.string(0)?,
        name: row.string(1)?,
        base_url: row.string(2)?,
        model: row.optional_string(3)?,
        wire_api: row.string(4)?,
        requires_open_ai_auth: row.i64(5)? != 0,
        token: row.optional_string(6)?,
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

    #[test]
    fn creates_incrementing_provider_copy_names() -> Result<()> {
        let config_path = temp_config_path("copy-names");
        let conn = open_database(&config_path)?;
        conn.execute(
            r#"
            INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            &[
                SqlValue::Text("openai"),
                SqlValue::Text("OpenAI"),
                SqlValue::Text("https://api.openai.com/v1"),
                SqlValue::OptionalText(Some("gpt-5")),
                SqlValue::Text("responses"),
                SqlValue::I64(1),
                SqlValue::OptionalText(None),
            ],
        )?;
        conn.execute(
            r#"
            INSERT INTO providers (id, name, base_url, model, wire_api, requires_openai_auth, token)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            &[
                SqlValue::Text("openai-copy"),
                SqlValue::Text("OpenAI copy1"),
                SqlValue::Text("https://api.openai.com/v1"),
                SqlValue::OptionalText(Some("gpt-5")),
                SqlValue::Text("responses"),
                SqlValue::I64(1),
                SqlValue::OptionalText(None),
            ],
        )?;

        assert_eq!(next_copy_name(&conn, "OpenAI")?, "OpenAI copy2");
        assert_eq!(next_copy_name(&conn, "OpenAI copy1")?, "OpenAI copy2");

        if let Some(parent) = config_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
        Ok(())
    }
}
