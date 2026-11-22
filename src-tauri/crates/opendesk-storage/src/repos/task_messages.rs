use crate::error::AppResult;
use sqlx::SqlitePool;

use crate::repos::conversations::messages as repo;
pub use crate::repos::conversations::messages::{CreateTaskMessageInput, TaskMessage};

pub async fn list(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<TaskMessage>> {
    repo::list(pool, task_id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<TaskMessage> {
    repo::get(pool, id).await
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
    repo::create(pool, task_id, role, kind, content, run_id, metadata_json).await
}
