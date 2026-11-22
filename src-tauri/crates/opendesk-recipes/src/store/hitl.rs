use crate::error::{AppError, AppResult};

use sqlx::SqlitePool;

use super::{HitlRequest, RecipeRun};

const HITL_COLUMNS: &str = "token, recipe_run_id, workspace_id, checkpoint_id, interaction_type, \
    title, question_markdown, context_json, options_json, status, on_timeout, \
    dispatched_channels_json, resolved_by_channel, resolved_by_sender, resolution_type, \
    resolution_payload, expires_at, created_at, resolved_at";

#[allow(clippy::too_many_arguments)]
pub async fn create_hitl(
    pool: &SqlitePool,
    token: &str,
    run: &RecipeRun,
    checkpoint_id: &str,
    interaction_type: &str,
    title: &str,
    question: &str,
    context: &serde_json::Value,
    options: &[String],
    on_timeout: &str,
    expires_at: &str,
) -> AppResult<HitlRequest> {
    sqlx::query(
        "INSERT INTO hitl_requests (
            token, recipe_run_id, workspace_id, checkpoint_id, interaction_type,
            title, question_markdown, context_json, options_json, on_timeout, expires_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(token)
    .bind(&run.id)
    .bind(&run.workspace_id)
    .bind(checkpoint_id)
    .bind(interaction_type)
    .bind(title)
    .bind(question)
    .bind(context.to_string())
    .bind(serde_json::to_string(options).unwrap_or_else(|_| "[]".into()))
    .bind(on_timeout)
    .bind(expires_at)
    .execute(pool)
    .await?;
    get_hitl(pool, token).await
}

pub async fn get_hitl(pool: &SqlitePool, token: &str) -> AppResult<HitlRequest> {
    let query = format!("SELECT {HITL_COLUMNS} FROM hitl_requests WHERE token = ?");
    sqlx::query_as::<_, HitlRequest>(&query)
        .bind(token)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("hitl {token}")))
}

pub async fn resolve_hitl(
    pool: &SqlitePool,
    token: &str,
    channel: &str,
    sender: Option<&str>,
    resolution_type: &str,
    payload: Option<&str>,
) -> AppResult<Option<HitlRequest>> {
    let result = sqlx::query(
        "UPDATE hitl_requests SET status = 'RESOLVED', resolved_by_channel = ?,
            resolved_by_sender = ?, resolution_type = ?, resolution_payload = ?,
            resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE token = ? AND status = 'PENDING'",
    )
    .bind(channel)
    .bind(sender)
    .bind(resolution_type)
    .bind(payload)
    .bind(token)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    Ok(Some(get_hitl(pool, token).await?))
}

pub async fn dispatch_channel(
    pool: &SqlitePool,
    token: &str,
    channel: &str,
    external_id: &str,
) -> AppResult<()> {
    let current = get_hitl(pool, token).await?;
    let mut value: serde_json::Value = serde_json::from_str(&current.dispatched_channels_json)
        .unwrap_or_else(|_| serde_json::json!({}));
    value[channel] = serde_json::Value::String(external_id.to_string());
    sqlx::query("UPDATE hitl_requests SET dispatched_channels_json = ? WHERE token = ?")
        .bind(value.to_string())
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn pending_hitl(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<HitlRequest>> {
    let query = format!(
        "SELECT {HITL_COLUMNS} FROM hitl_requests
         WHERE workspace_id = ? AND status = 'PENDING' ORDER BY created_at"
    );
    Ok(sqlx::query_as::<_, HitlRequest>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn pending_hitl_for_run(
    pool: &SqlitePool,
    recipe_run_id: &str,
) -> AppResult<Vec<HitlRequest>> {
    let query = format!(
        "SELECT {HITL_COLUMNS} FROM hitl_requests
         WHERE recipe_run_id = ? AND status = 'PENDING' ORDER BY created_at"
    );
    Ok(sqlx::query_as::<_, HitlRequest>(&query)
        .bind(recipe_run_id)
        .fetch_all(pool)
        .await?)
}

pub async fn pending_hitl_for_checkpoint(
    pool: &SqlitePool,
    run_id: &str,
    checkpoint_id: &str,
) -> AppResult<Option<HitlRequest>> {
    let query = format!(
        "SELECT {HITL_COLUMNS} FROM hitl_requests
         WHERE recipe_run_id = ? AND checkpoint_id = ? AND status = 'PENDING'
         ORDER BY created_at DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, HitlRequest>(&query)
        .bind(run_id)
        .bind(checkpoint_id)
        .fetch_optional(pool)
        .await?)
}
