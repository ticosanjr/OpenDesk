use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskInteraction {
    pub id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub question: String,
    pub context_markdown: Option<String>,
    pub options_json: String,
    pub multiple: bool,
    pub allow_custom: bool,
    pub status: String,
    pub answer_json: Option<String>,
    pub expires_at: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

const COLUMNS: &str = "id, workspace_id, task_id, run_id, question, context_markdown, \
    options_json, multiple, allow_custom, status, answer_json, expires_at, created_at, resolved_at";

#[derive(Debug, Clone)]
pub struct CreateInteractionInput {
    pub workspace_id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub question: String,
    pub context_markdown: Option<String>,
    pub options: Vec<String>,
    pub multiple: bool,
    pub allow_custom: bool,
    pub expires_at: String,
}

pub async fn create(
    pool: &SqlitePool,
    input: CreateInteractionInput,
) -> AppResult<TaskInteraction> {
    let id = Uuid::now_v7().to_string();
    let options = serde_json::to_string(&input.options).unwrap_or_else(|_| "[]".to_string());
    sqlx::query(
        "INSERT INTO task_interactions (
            id, workspace_id, task_id, run_id, question, context_markdown,
            options_json, multiple, allow_custom, expires_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(&input.task_id)
    .bind(input.run_id.as_deref())
    .bind(input.question.trim())
    .bind(input.context_markdown.as_deref())
    .bind(&options)
    .bind(input.multiple)
    .bind(input.allow_custom)
    .bind(&input.expires_at)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<TaskInteraction> {
    let query = format!("SELECT {COLUMNS} FROM task_interactions WHERE id = ?");
    sqlx::query_as::<_, TaskInteraction>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("interaction {id}")))
}

pub async fn pending_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<TaskInteraction>> {
    let query = format!(
        "SELECT {COLUMNS} FROM task_interactions
         WHERE task_id = ? AND status = 'PENDING'
         ORDER BY created_at ASC"
    );
    Ok(sqlx::query_as::<_, TaskInteraction>(&query)
        .bind(task_id)
        .fetch_all(pool)
        .await?)
}

pub async fn pending_for_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<TaskInteraction>> {
    let query = format!(
        "SELECT {COLUMNS} FROM task_interactions
         WHERE workspace_id = ? AND status = 'PENDING'
         ORDER BY created_at ASC"
    );
    Ok(sqlx::query_as::<_, TaskInteraction>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn resolve(
    pool: &SqlitePool,
    id: &str,
    answer: &serde_json::Value,
    status: &str,
) -> AppResult<TaskInteraction> {
    if !matches!(status, "RESOLVED" | "EXPIRED" | "CANCELLED") {
        return Err(AppError::BadRequest(format!(
            "invalid interaction status '{status}'"
        )));
    }
    let answer_json = serde_json::to_string(answer)
        .map_err(|error| AppError::Internal(format!("invalid interaction answer: {error}")))?;
    let result = sqlx::query(
        "UPDATE task_interactions SET status = ?, answer_json = ?,
            resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND status = 'PENDING'",
    )
    .bind(status)
    .bind(&answer_json)
    .bind(id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "esta interação já foi respondida ou expirou".into(),
        ));
    }
    get(pool, id).await
}
