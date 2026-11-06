use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskMessage {
    pub id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub role: String,
    pub kind: String,
    pub content: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskMessageInput {
    pub content: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub continue_execution: bool,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

pub async fn list(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<TaskMessage>> {
    Ok(sqlx::query_as::<_, TaskMessage>(
        "SELECT id, task_id, run_id, role, kind, content, metadata_json, created_at
         FROM task_messages WHERE task_id = ? ORDER BY created_at ASC, id ASC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?)
}

pub async fn create(
    pool: &SqlitePool,
    task_id: &str,
    role: &str,
    kind: &str,
    content: &str,
    run_id: Option<&str>,
    metadata_json: Option<&str>,
) -> AppResult<TaskMessage> {
    let content = content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest("message content is required".into()));
    }
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO task_messages (
            id, task_id, run_id, role, kind, content, metadata_json
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(task_id)
    .bind(run_id)
    .bind(role)
    .bind(kind)
    .bind(content)
    .bind(metadata_json)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, TaskMessage>(
        "SELECT id, task_id, run_id, role, kind, content, metadata_json, created_at
         FROM task_messages WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}
