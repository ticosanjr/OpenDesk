use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TaskConversation {
    pub id: String,
    pub workspace_id: String,
}

/// Returns (creating when needed) the conversation that owns a task's
/// comments and HITL requests. `thread_key` is stable, so concurrent callers
/// converge on the same row even if the insert races.
pub async fn ensure(pool: &SqlitePool, task_id: &str) -> AppResult<TaskConversation> {
    let workspace_id: Option<String> =
        sqlx::query_scalar("SELECT workspace_id FROM tasks WHERE id = ?")
            .bind(task_id)
            .fetch_optional(pool)
            .await?;
    let Some(workspace_id) = workspace_id else {
        return Err(AppError::NotFound(format!("task {task_id}")));
    };
    let thread_key = format!("task:{task_id}");

    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM conversations
         WHERE workspace_id = ? AND channel = 'task' AND thread_key = ?",
    )
    .bind(&workspace_id)
    .bind(&thread_key)
    .fetch_optional(pool)
    .await?
    {
        return Ok(TaskConversation { id, workspace_id });
    }

    let id = Uuid::now_v7().to_string();
    let inserted = sqlx::query(
        "INSERT INTO conversations
            (id, workspace_id, task_id, channel, thread_key, title, source)
         VALUES (?, ?, ?, 'task', ?, ?, 'task')
         ON CONFLICT DO NOTHING",
    )
    .bind(&id)
    .bind(&workspace_id)
    .bind(task_id)
    .bind(&thread_key)
    .bind(format!("Tarefa {task_id}"))
    .execute(pool)
    .await?;
    if inserted.rows_affected() == 0 {
        // A concurrent caller won the insert.
        let existing = sqlx::query_scalar::<_, String>(
            "SELECT id FROM conversations
             WHERE workspace_id = ? AND channel = 'task' AND thread_key = ?",
        )
        .bind(&workspace_id)
        .bind(&thread_key)
        .fetch_one(pool)
        .await?;
        return Ok(TaskConversation {
            id: existing,
            workspace_id,
        });
    }
    Ok(TaskConversation { id, workspace_id })
}
