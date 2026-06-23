mod crypto;
mod provider;
mod settings;

pub(crate) use crypto::encrypt_optional_token;
pub(crate) use provider::*;
pub(crate) use settings::*;

use anyhow::{anyhow, Result};
use rusqlite::{types::Value as SqliteValue, Connection, OptionalExtension};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

const DATABASE_FILE_NAME: &str = "codex-config-desktop.sqlite3";

static INITIALIZED_DATABASES: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
            other => Err(anyhow!("unknown tool type: {other}")),
        }
    }
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Default for ToolType {
    fn default() -> Self {
        ToolType::Codex
    }
}

pub(crate) fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
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
            .map_err(|e| anyhow!("failed to open SQLite database: {}: {}", path.display(), e))?;
        Ok(Self { conn })
    }

    pub(crate) fn execute_batch(&self, sql: &str) -> Result<()> {
        self.conn
            .execute_batch(sql)
            .map_err(|e| anyhow!("SQLite execute batch failed: {}", e))?;
        Ok(())
    }

    pub(crate) fn execute(&self, sql: &str, params: &[SqlValue<'_>]) -> Result<()> {
        let params = sqlite_values(params);
        self.conn
            .execute(sql, rusqlite::params_from_iter(params))
            .map_err(|e| anyhow!("SQLite execute failed: {}", e))?;
        Ok(())
    }

    fn query_one<T>(
        &self,
        sql: &str,
        params: &[SqlValue<'_>],
        mapper: impl FnOnce(&SqliteRow<'_>) -> Result<T>,
    ) -> Result<Option<T>> {
        let params = sqlite_values(params);
        let mut statement = self.conn.prepare(sql).map_err(|e| anyhow!("SQLite prepare failed: {}", e))?;
        statement
            .query_row(rusqlite::params_from_iter(params), |row| {
                mapper(&SqliteRow { row }).map_err(to_rusqlite_error)
            })
            .optional()
            .map_err(|e| anyhow!("SQLite query failed: {}", e))
    }

    fn query_all<T>(
        &self,
        sql: &str,
        params: &[SqlValue<'_>],
        mut mapper: impl FnMut(&SqliteRow<'_>) -> Result<T>,
    ) -> Result<Vec<T>> {
        let params = sqlite_values(params);
        let mut statement = self.conn.prepare(sql).map_err(|e| anyhow!("SQLite prepare failed: {}", e))?;
        let mut rows = statement
            .query(rusqlite::params_from_iter(params))
            .map_err(|e| anyhow!("SQLite query failed: {}", e))?;
        let mut values = Vec::new();

        while let Some(row) = rows.next().map_err(|e| anyhow!("SQLite row read failed: {}", e))? {
            values.push(mapper(&SqliteRow { row })?);
        }

        Ok(values)
    }
}

impl SqliteRow<'_> {
    fn string(&self, index: usize) -> Result<String> {
        self.optional_string(index)?
            .ok_or_else(|| anyhow!("SQLite column {} is null", index))
    }

    fn optional_string(&self, index: usize) -> Result<Option<String>> {
        self.row
            .get::<_, Option<String>>(index)
            .map_err(|e| anyhow!("SQLite text column is not valid UTF-8: {}", e))
    }

    fn i64(&self, index: usize) -> Result<i64> {
        self.row
            .get::<_, i64>(index)
            .map_err(|e| anyhow!("SQLite integer column read failed: {}", e))
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

fn sqlite_identifier(identifier: &str) -> Result<String> {
    if identifier
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        Ok(format!("\"{}\"", identifier))
    } else {
        Err(anyhow!("invalid SQLite identifier: {}", identifier))
    }
}

fn table_has_column(conn: &SqliteConnection, table: &str, column: &str) -> Result<bool> {
    let sql = format!("PRAGMA table_info({})", sqlite_identifier(table)?);
    let columns = conn.query_all(&sql, &[], |row| row.string(1))?;
    Ok(columns.iter().any(|item| item == column))
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

fn database_path(config_path: &Path) -> Result<PathBuf> {
    let config_dir = config_path
        .parent()
        .ok_or_else(|| anyhow!("failed to locate config directory"))?;
    Ok(config_dir.join(DATABASE_FILE_NAME))
}

pub(crate) fn open_database(config_path: &Path) -> Result<SqliteConnection> {
    let db_path = database_path(config_path)?;
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow!("failed to create {}: {}", parent.display(), e))?;
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
    if !table_has_column(conn, "providers", "claude_haiku_model")? {
        conn.execute_batch("ALTER TABLE providers ADD COLUMN claude_haiku_model TEXT;")?;
    }
    if !table_has_column(conn, "providers", "claude_opus_model")? {
        conn.execute_batch("ALTER TABLE providers ADD COLUMN claude_opus_model TEXT;")?;
    }
    if !table_has_column(conn, "providers", "claude_sonnet_model")? {
        conn.execute_batch("ALTER TABLE providers ADD COLUMN claude_sonnet_model TEXT;")?;
    }
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_providers_tool_position
            ON providers (tool_type, position, name, id);
        CREATE INDEX IF NOT EXISTS idx_provider_models_provider_position
            ON provider_models (provider_id, position, model);
        "#,
    )?;
    provider::normalize_provider_positions(conn)?;
    crypto::encrypt_plaintext_tokens(conn, config_path)?;
    restrict_file_permissions(db_path)?;
    initialized.insert(db_path.to_path_buf());
    Ok(())
}

#[cfg(unix)]
pub(crate) fn restrict_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| anyhow!("failed to set permissions on {}: {}", path.display(), e))?;
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
        env,
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

        assert_eq!(provider::next_copy_name(&conn, "OpenAI")?, "OpenAI copy2");
        assert_eq!(provider::next_copy_name(&conn, "OpenAI copy1")?, "OpenAI copy2");

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
            provider::next_provider_id(&conn, "OpenAI", "https://api.example.com/v1", None)?,
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

        provider::reorder_provider_positions(
            &conn,
            ToolType::Codex,
            &["gamma".to_string(), "alpha".to_string(), "beta".to_string()],
        )?;

        let providers = provider::collect_providers_from_database(&conn, ToolType::Codex)?;
        let provider_ids = providers
            .into_iter()
            .map(|provider| provider.id)
            .collect::<Vec<_>>();

        assert_eq!(provider_ids, vec!["gamma", "alpha", "beta"]);
        assert_eq!(provider::next_provider_position(&conn, ToolType::Codex)?, 3);

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

        settings::set_active_provider(&conn, ToolType::Codex, "codex-provider")?;
        settings::set_active_model(&conn, ToolType::Codex, "gpt-5")?;
        settings::set_active_provider(&conn, ToolType::Claude, "claude-provider")?;
        settings::set_active_model(&conn, ToolType::Claude, "claude-opus-4-7")?;

        assert_eq!(
            settings::get_active_provider(&conn, ToolType::Codex)?.as_deref(),
            Some("codex-provider")
        );
        assert_eq!(settings::get_active_model(&conn, ToolType::Codex)?.as_deref(), Some("gpt-5"));
        assert_eq!(
            settings::get_active_provider(&conn, ToolType::Claude)?.as_deref(),
            Some("claude-provider")
        );
        assert_eq!(
            settings::get_active_model(&conn, ToolType::Claude)?.as_deref(),
            Some("claude-opus-4-7")
        );

        if let Some(parent) = config_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
        Ok(())
    }
}
