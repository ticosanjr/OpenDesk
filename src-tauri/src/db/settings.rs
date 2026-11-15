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

pub async fn delete(pool: &SqlitePool, key: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM app_settings WHERE key = ?")
        .bind(key)
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

/// Reads a boolean app setting, falling back to `default` when it is absent or
/// malformed. Used by operational toggles such as auto-start.
pub async fn bool_flag(pool: &SqlitePool, key: &str, default: bool) -> bool {
    match get(pool, key).await {
        Ok(Some(value)) => value.as_bool().unwrap_or(default),
        _ => default,
    }
}

pub async fn active_workspace_id(pool: &SqlitePool) -> AppResult<Option<String>> {
    if let Some(value) = get(pool, "active_workspace_id").await?
        && let Some(id) = value.as_str()
    {
        let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM workspaces WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        if exists.is_some() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn setting_roundtrip_and_active_workspace() {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("settings.db"))
            .await
            .expect("db");

        assert!(get(&pool, "missing").await.expect("get").is_none());
        set(&pool, "greeting", &serde_json::json!("olá"))
            .await
            .expect("set");
        set(&pool, "greeting", &serde_json::json!("oi"))
            .await
            .expect("overwrite");
        assert_eq!(
            get(&pool, "greeting").await.expect("get").unwrap(),
            serde_json::json!("oi")
        );
        assert_eq!(all(&pool).await.expect("all").len(), 1);

        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        assert_eq!(
            active_workspace_id(&pool).await.expect("active"),
            Some("ws-1".into())
        );
        set_active_workspace(&pool, "ws-1")
            .await
            .expect("set active");
        assert_eq!(
            active_workspace_id(&pool).await.expect("active 2"),
            Some("ws-1".into())
        );
    }
}
