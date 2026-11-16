use crate::error::AppResult;
use sqlx::SqlitePool;

pub fn split_model(model: &str) -> (String, String) {
    let base = model.split('#').next().unwrap_or(model);
    match base.split_once('/') {
        Some((provider, id)) => (provider.to_string(), id.to_string()),
        None => (String::new(), base.to_string()),
    }
}

pub async fn record_failure(pool: &SqlitePool, model: &str, code: &str) -> AppResult<()> {
    let (provider_id, model_id) = split_model(model);
    if model_id.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO model_health (provider_id, model_id, code, failures, failed_at)
         VALUES (?, ?, ?, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(provider_id, model_id) DO UPDATE SET
            failures = model_health.failures + 1,
            code = excluded.code,
            failed_at = excluded.failed_at",
    )
    .bind(provider_id)
    .bind(model_id)
    .bind(code)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn record_cooldown(
    pool: &SqlitePool,
    model: &str,
    code: &str,
    minutes: i64,
) -> AppResult<()> {
    let (provider_id, model_id) = split_model(model);
    if model_id.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO model_health (provider_id, model_id, code, failures, failed_at, cooldown_until)
         VALUES (?, ?, ?, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?))
         ON CONFLICT(provider_id, model_id) DO UPDATE SET
            failures = model_health.failures + 1,
            code = excluded.code,
            failed_at = excluded.failed_at,
            cooldown_until = excluded.cooldown_until",
    )
    .bind(provider_id)
    .bind(model_id)
    .bind(code)
    .bind(format!("+{minutes} minutes"))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn record_success(pool: &SqlitePool, model: &str) -> AppResult<()> {
    let (provider_id, model_id) = split_model(model);
    sqlx::query("DELETE FROM model_health WHERE provider_id = ? AND model_id = ?")
        .bind(provider_id)
        .bind(model_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Models that failed recently and should be avoided by the router.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ModelHealth {
    pub provider_id: String,
    pub model_id: String,
    pub code: String,
    pub failures: i64,
    pub failed_at: String,
}

pub async fn list(pool: &SqlitePool, hours: i64) -> AppResult<Vec<ModelHealth>> {
    Ok(sqlx::query_as::<_, ModelHealth>(
        "SELECT provider_id, model_id, code, failures, failed_at FROM model_health
         WHERE failed_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?)
         ORDER BY failed_at DESC",
    )
    .bind(format!("-{hours} hours"))
    .fetch_all(pool)
    .await?)
}

pub async fn clear_model(pool: &SqlitePool, provider_id: &str, model_id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM model_health WHERE provider_id = ? AND model_id = ?")
        .bind(provider_id)
        .bind(model_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn cooling(pool: &SqlitePool, hours: i64) -> AppResult<Vec<String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT provider_id, model_id FROM model_health
         WHERE (cooldown_until IS NOT NULL AND cooldown_until > strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            OR (cooldown_until IS NULL AND failed_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?))",
    )
    .bind(format!("-{hours} hours"))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(provider, id)| format!("{provider}/{id}"))
        .collect())
}

#[allow(dead_code)]
pub async fn clear(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("DELETE FROM model_health")
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn records_and_cools_down_models() {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("health.db"))
            .await
            .expect("db");
        record_failure(&pool, "opencode/crouton#low", "model_unavailable")
            .await
            .expect("failure");
        assert_eq!(
            cooling(&pool, 24).await.expect("cooling"),
            vec!["opencode/crouton".to_string()]
        );
        record_success(&pool, "opencode/crouton")
            .await
            .expect("success");
        assert!(cooling(&pool, 24).await.expect("cooling").is_empty());
        record_cooldown(&pool, "opencode/crouton", "rate_limited", 5)
            .await
            .expect("cooldown");
        assert_eq!(
            cooling(&pool, 24).await.expect("cooldown cooling"),
            vec!["opencode/crouton".to_string()]
        );
        clear(&pool).await.expect("clear");
        assert!(cooling(&pool, 24).await.expect("cooling").is_empty());
    }
}
