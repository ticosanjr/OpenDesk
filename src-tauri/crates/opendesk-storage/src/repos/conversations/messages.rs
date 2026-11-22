use super::task;
use crate::error::{AppError, AppResult};
use opendesk_domain::MessageRole;
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

const COLUMNS: &str = "id, task_id, run_id, role, kind, body AS content, metadata_json, \
    created_at";

/// Task comments and follow-ups. Chat, turns and HITL requests live in the same
/// table but are excluded here so the Kanban stream stays focused.
pub async fn list(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<TaskMessage>> {
    let query = format!(
        "SELECT {COLUMNS} FROM messages
         WHERE task_id = ? AND kind NOT IN ('chat', 'turn', 'hitl_request')
         ORDER BY created_at ASC, id ASC"
    );
    Ok(sqlx::query_as::<_, TaskMessage>(&query)
        .bind(task_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<TaskMessage> {
    let query = format!(
        "SELECT {COLUMNS} FROM messages
         WHERE id = ? AND kind NOT IN ('chat', 'turn', 'hitl_request')"
    );
    sqlx::query_as::<_, TaskMessage>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task message {id}")))
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
    let role = MessageRole::parse(role)?;
    let kind = kind.trim();
    let kind = if kind.is_empty() { "comment" } else { kind };
    if matches!(kind, "chat" | "turn" | "hitl_request") {
        return Err(AppError::BadRequest(format!(
            "reserved message kind '{kind}'"
        )));
    }
    let conversation = task::ensure(pool, task_id).await?;
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO messages (
            id, conversation_id, workspace_id, task_id, run_id, role, kind, body,
            metadata_json, status
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'COMPLETED')",
    )
    .bind(&id)
    .bind(&conversation.id)
    .bind(&conversation.workspace_id)
    .bind(task_id)
    .bind(run_id)
    .bind(role.as_str())
    .bind(kind)
    .bind(content)
    .bind(metadata_json)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE conversations SET last_message_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&conversation.id)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::pool::init(&temp.path().join("messages.db"))
            .await
            .expect("db");
        sqlx::query(
            "INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', '/tmp/ws-1')",
        )
        .execute(&pool)
        .await
        .expect("workspace");
        sqlx::query(
            "INSERT INTO tasks (id, workspace_id, title, description)
             VALUES ('task-1', 'ws-1', 'Tarefa', 'Descrição')",
        )
        .execute(&pool)
        .await
        .expect("task");
        (temp, pool)
    }

    #[tokio::test]
    async fn comments_share_the_task_conversation() {
        let (_temp, pool) = setup().await;
        let first = create(&pool, "task-1", "user", "comment", "primeiro", None, None)
            .await
            .expect("first");
        assert_eq!(first.task_id, "task-1");
        let second = create(
            &pool,
            "task-1",
            "assistant",
            "follow_up",
            "segundo",
            None,
            Some("{\"a\":1}"),
        )
        .await
        .expect("second");
        let list = list(&pool, "task-1").await.expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].id, second.id);
        assert_eq!(list[1].kind, "follow_up");

        let conversations: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE channel = 'task'")
                .fetch_one(&pool)
                .await
                .expect("conversations");
        assert_eq!(conversations, 1);
    }

    #[tokio::test]
    async fn rejects_reserved_kinds_and_unknown_tasks() {
        let (_temp, pool) = setup().await;
        assert!(
            create(&pool, "task-1", "user", "turn", "x", None, None)
                .await
                .is_err()
        );
        assert!(
            create(&pool, "missing", "user", "comment", "x", None, None)
                .await
                .is_err()
        );
    }
}
