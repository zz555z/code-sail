use anyhow::Result;

use super::{SqliteConnection, SqlValue, ToolType};

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
