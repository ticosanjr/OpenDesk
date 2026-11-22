use crate::error::AppResult;
use sqlx::SqlitePool;

use opendesk_storage::repos::conversations::messages as repo;
pub use opendesk_storage::repos::conversations::messages::{CreateTaskMessageInput, TaskMessage};

pub async fn list(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<TaskMessage>> {
    Ok(repo::list(pool, task_id).await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<TaskMessage> {
    Ok(repo::get(pool, id).await?)
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
    Ok(repo::create(pool, task_id, role, kind, content, run_id, metadata_json).await?)
}
