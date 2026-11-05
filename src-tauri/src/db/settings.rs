use super::models::AppSetting;
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

pub async fn get(pool: &SqlitePool, key: &str) -> AppResult<Option<serde_json::Value>> {
    let row = sqlx::query_as::<_, AppSetting>(
        "SELECT key, value_json, updated_at FROM app_settings WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        serde_json::from_str(&row.value_json)
            .map_err(|error| AppError::Internal(format!("invalid setting '{key}': {error}")))
    })
    .transpose()
}

pub async fn set(pool: &SqlitePool, key: &str, value: &serde_json::Value) -> AppResult<()> {
    let encoded = serde_json::to_string(value)
        .map_err(|error| AppError::Internal(format!("could not encode setting: {error}")))?;
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at)
         VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(key) DO UPDATE SET
            value_json = excluded.value_json,
            updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(encoded)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn all(pool: &SqlitePool) -> AppResult<Vec<AppSetting>> {
    Ok(sqlx::query_as::<_, AppSetting>(
        "SELECT key, value_json, updated_at FROM app_settings ORDER BY key",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn active_workspace_id(pool: &SqlitePool) -> AppResult<Option<String>> {
    if let Some(value) = get(pool, "active_workspace_id").await? {
        if let Some(id) = value.as_str() {
            return Ok(Some(id.to_string()));
        }
    }

    let first: Option<String> =
        sqlx::query_scalar("SELECT id FROM workspaces ORDER BY last_opened_at DESC LIMIT 1")
            .fetch_optional(pool)
            .await?;
    Ok(first)
}

pub async fn set_active_workspace(pool: &SqlitePool, workspace_id: &str) -> AppResult<()> {
    set(
        pool,
        "active_workspace_id",
        &serde_json::Value::String(workspace_id.to_string()),
    )
    .await
}
