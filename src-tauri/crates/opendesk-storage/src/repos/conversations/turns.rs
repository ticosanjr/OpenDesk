use crate::error::{AppError, AppResult};
use opendesk_domain::RunStatus;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatTurn {
    pub id: String,
    pub workspace_id: String,
    pub thread_id: String,
    pub run_id: Option<String>,
    pub source: String,
    pub status: String,
    pub agent: String,
    pub model: Option<String>,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub tokens_cache_read: i64,
    pub tokens_cache_write: i64,
    pub estimated_cost_usd: f64,
    pub duration_ms: i64,
    pub user_content: String,
    pub assistant_content: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChatTurnUsage {
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub tokens_cache_read: i64,
    pub tokens_cache_write: i64,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone)]
pub struct CreateChatTurnInput {
    pub workspace_id: String,
    pub thread_id: String,
    pub source: String,
    pub agent: String,
    pub model: Option<String>,
    pub user_content: String,
}

const SELECT: &str = "SELECT m.id, m.workspace_id, m.conversation_id AS thread_id, m.run_id,
    COALESCE(json_extract(m.payload_json, '$.source'), 'desktop') AS source,
    COALESCE(m.status, 'RUNNING') AS status,
    COALESCE(json_extract(m.payload_json, '$.agent'), '') AS agent,
    json_extract(m.payload_json, '$.model') AS model,
    CAST(COALESCE(json_extract(m.payload_json, '$.tokens_input'), 0) AS INTEGER) AS tokens_input,
    CAST(COALESCE(json_extract(m.payload_json, '$.tokens_output'), 0) AS INTEGER) AS tokens_output,
    CAST(COALESCE(json_extract(m.payload_json, '$.tokens_reasoning'), 0) AS INTEGER) AS tokens_reasoning,
    CAST(COALESCE(json_extract(m.payload_json, '$.tokens_cache_read'), 0) AS INTEGER) AS tokens_cache_read,
    CAST(COALESCE(json_extract(m.payload_json, '$.tokens_cache_write'), 0) AS INTEGER) AS tokens_cache_write,
    CAST(COALESCE(json_extract(m.payload_json, '$.estimated_cost_usd'), 0) AS REAL) AS estimated_cost_usd,
    CAST(COALESCE(json_extract(m.payload_json, '$.duration_ms'), 0) AS INTEGER) AS duration_ms,
    COALESCE(json_extract(m.payload_json, '$.user_content'), '') AS user_content,
    NULLIF(m.body, '') AS assistant_content,
    json_extract(m.payload_json, '$.error_code') AS error_code,
    json_extract(m.payload_json, '$.error_message') AS error_message,
    m.created_at AS started_at, m.completed_at
  FROM messages m";

pub async fn create(pool: &SqlitePool, input: CreateChatTurnInput) -> AppResult<ChatTurn> {
    let id = Uuid::now_v7().to_string();
    let inserted = sqlx::query(
        "INSERT INTO messages
            (id, conversation_id, workspace_id, task_id, role, kind, body, payload_json, status)
         SELECT ?, c.id, c.workspace_id, c.task_id, 'assistant', 'turn', '',
            json_object(
                'source', ?, 'agent', ?, 'model', ?, 'user_content', ?,
                'tokens_input', 0, 'tokens_output', 0, 'tokens_reasoning', 0,
                'tokens_cache_read', 0, 'tokens_cache_write', 0,
                'estimated_cost_usd', 0.0, 'duration_ms', 0
            ), 'RUNNING'
         FROM conversations c WHERE c.id = ?",
    )
    .bind(&id)
    .bind(&input.source)
    .bind(&input.agent)
    .bind(&input.model)
    .bind(input.user_content.trim())
    .bind(&input.thread_id)
    .execute(pool)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "chat thread {}",
            input.thread_id
        )));
    }
    let _ = input.workspace_id;
    get(pool, &id).await
}

#[allow(clippy::too_many_arguments)]
pub async fn finish(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    assistant_content: Option<&str>,
    usage: &ChatTurnUsage,
    duration_ms: i64,
    error_code: Option<&str>,
    error_message: Option<&str>,
) -> AppResult<ChatTurn> {
    let status = RunStatus::parse(status)?;
    sqlx::query(
        "UPDATE messages SET
            status = ?, body = COALESCE(?, body),
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            payload_json = json_set(
                COALESCE(payload_json, '{}'),
                '$.tokens_input', ?, '$.tokens_output', ?, '$.tokens_reasoning', ?,
                '$.tokens_cache_read', ?, '$.tokens_cache_write', ?,
                '$.estimated_cost_usd', ?, '$.duration_ms', ?,
                '$.error_code', ?, '$.error_message', ?
            )
         WHERE id = ? AND kind = 'turn'",
    )
    .bind(status.as_str())
    .bind(assistant_content)
    .bind(usage.tokens_input)
    .bind(usage.tokens_output)
    .bind(usage.tokens_reasoning)
    .bind(usage.tokens_cache_read)
    .bind(usage.tokens_cache_write)
    .bind(usage.estimated_cost_usd)
    .bind(duration_ms)
    .bind(error_code)
    .bind(error_message)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_run(pool: &SqlitePool, id: &str, run_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE messages SET run_id = ? WHERE id = ? AND kind = 'turn'")
        .bind(run_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ChatTurn> {
    let query = format!("{SELECT} WHERE m.id = ? AND m.kind = 'turn'");
    sqlx::query_as::<_, ChatTurn>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("chat turn {id}")))
}

pub async fn latest_for_thread(pool: &SqlitePool, thread_id: &str) -> AppResult<Option<ChatTurn>> {
    let query = format!(
        "{SELECT} WHERE m.conversation_id = ? AND m.kind = 'turn'
         ORDER BY m.created_at DESC, m.id DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, ChatTurn>(&query)
        .bind(thread_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn active_for_thread(pool: &SqlitePool, thread_id: &str) -> AppResult<Option<ChatTurn>> {
    let query = format!(
        "{SELECT} WHERE m.conversation_id = ? AND m.kind = 'turn' AND m.status = 'RUNNING'
         ORDER BY m.created_at DESC, m.id DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, ChatTurn>(&query)
        .bind(thread_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn mark_interrupted(pool: &SqlitePool) -> AppResult<u64> {
    let _ = sqlx::query(
        "UPDATE task_runs SET status = 'FAILED',
            error_code = 'interrupted',
            error_message = 'o OpenDesk reiniciou durante o turno de chat',
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE execution_mode = 'CHAT' AND status = 'RUNNING'",
    )
    .execute(pool)
    .await;
    let result = sqlx::query(
        "UPDATE messages SET status = 'FAILED',
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            payload_json = json_set(
                COALESCE(payload_json, '{}'),
                '$.error_code', 'interrupted',
                '$.error_message', 'o OpenDesk reiniciou durante o turno'
            )
         WHERE kind = 'turn' AND status = 'RUNNING'",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::conversations::threads::{self, CreateThreadInput};
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool, String) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::pool::init(&temp.path().join("turns.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        let thread = threads::create_thread(
            &pool,
            CreateThreadInput {
                workspace_id: "ws-1".into(),
                title: Some("Turno".into()),
                agent: "build".into(),
                model: None,
                source: Some("telegram".into()),
                external_id: Some("42".into()),
            },
        )
        .await
        .expect("thread");
        (temp, pool, thread.id)
    }

    fn input(thread_id: &str) -> CreateChatTurnInput {
        CreateChatTurnInput {
            workspace_id: "ws-1".into(),
            thread_id: thread_id.into(),
            source: "telegram".into(),
            agent: "build".into(),
            model: None,
            user_content: "Gere um arquivo".into(),
        }
    }

    #[tokio::test]
    async fn records_and_interrupts_a_chat_turn() {
        let (_temp, pool, thread_id) = setup().await;
        let turn = create(&pool, input(&thread_id)).await.expect("turn");
        assert_eq!(turn.status, "RUNNING");
        assert_eq!(turn.user_content, "Gere um arquivo");
        assert!(
            active_for_thread(&pool, &thread_id)
                .await
                .expect("active")
                .is_some()
        );

        let finished = finish(
            &pool,
            &turn.id,
            "SUCCESS",
            Some("pronto"),
            &ChatTurnUsage {
                tokens_input: 10,
                tokens_output: 20,
                estimated_cost_usd: 0.01,
                ..Default::default()
            },
            1500,
            None,
            None,
        )
        .await
        .expect("finish");
        assert_eq!(finished.status, "SUCCESS");
        assert_eq!(finished.tokens_output, 20);
        assert_eq!(finished.assistant_content.as_deref(), Some("pronto"));
        assert!(finished.completed_at.is_some());
        assert_eq!(
            latest_for_thread(&pool, &thread_id)
                .await
                .expect("latest")
                .map(|turn| turn.id),
            Some(turn.id)
        );

        create(&pool, input(&thread_id)).await.expect("second");
        assert_eq!(mark_interrupted(&pool).await.expect("interrupt"), 1);
    }

    #[tokio::test]
    async fn rejects_unknown_statuses_and_threads() {
        let (_temp, pool, thread_id) = setup().await;
        let turn = create(&pool, input(&thread_id)).await.expect("turn");
        assert!(
            finish(
                &pool,
                &turn.id,
                "DONE",
                None,
                &ChatTurnUsage::default(),
                0,
                None,
                None
            )
            .await
            .is_err()
        );
        assert!(create(&pool, input("missing")).await.is_err());
    }
}
