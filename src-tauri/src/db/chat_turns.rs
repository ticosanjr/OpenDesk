use crate::{
    db::runs::RunUsage,
    error::{AppError, AppResult},
};
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

const COLUMNS: &str = "id, workspace_id, thread_id, run_id, source, status, agent, model, \
    tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write, \
    estimated_cost_usd, duration_ms, user_content, assistant_content, error_code, error_message, \
    started_at, completed_at";

#[derive(Debug, Clone)]
pub struct CreateChatTurnInput {
    pub workspace_id: String,
    pub thread_id: String,
    pub source: String,
    pub agent: String,
    pub model: Option<String>,
    pub user_content: String,
}

pub async fn create(pool: &SqlitePool, input: CreateChatTurnInput) -> AppResult<ChatTurn> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO chat_turns (
            id, workspace_id, thread_id, source, status, agent, model, user_content
         ) VALUES (?, ?, ?, ?, 'RUNNING', ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(&input.thread_id)
    .bind(&input.source)
    .bind(&input.agent)
    .bind(input.model.as_deref())
    .bind(input.user_content.trim())
    .execute(pool)
    .await?;
    get(pool, &id).await
}

#[allow(clippy::too_many_arguments)]
pub async fn finish(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    assistant_content: Option<&str>,
    usage: &RunUsage,
    duration_ms: i64,
    error_code: Option<&str>,
    error_message: Option<&str>,
) -> AppResult<ChatTurn> {
    sqlx::query(
        "UPDATE chat_turns SET
            status = ?, assistant_content = ?, tokens_input = ?, tokens_output = ?,
            tokens_reasoning = ?, tokens_cache_read = ?, tokens_cache_write = ?,
            estimated_cost_usd = ?, duration_ms = ?, error_code = ?, error_message = ?,
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(status)
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
    sqlx::query("UPDATE chat_turns SET run_id = ? WHERE id = ?")
        .bind(run_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ChatTurn> {
    let query = format!("SELECT {COLUMNS} FROM chat_turns WHERE id = ?");
    sqlx::query_as::<_, ChatTurn>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("chat turn {id}")))
}

pub async fn latest_for_thread(pool: &SqlitePool, thread_id: &str) -> AppResult<Option<ChatTurn>> {
    let query = format!(
        "SELECT {COLUMNS} FROM chat_turns WHERE thread_id = ? ORDER BY started_at DESC LIMIT 1"
    );
    sqlx::query_as::<_, ChatTurn>(&query)
        .bind(thread_id)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
}

pub async fn active_for_thread(pool: &SqlitePool, thread_id: &str) -> AppResult<Option<ChatTurn>> {
    let query = format!(
        "SELECT {COLUMNS} FROM chat_turns
         WHERE thread_id = ? AND status = 'RUNNING'
         ORDER BY started_at DESC LIMIT 1"
    );
    sqlx::query_as::<_, ChatTurn>(&query)
        .bind(thread_id)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
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
        "UPDATE chat_turns SET status = 'FAILED',
            error_code = 'interrupted',
            error_message = 'o OpenDesk reiniciou durante o turno',
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE status = 'RUNNING'",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_and_interrupts_a_chat_turn() {
        let ctx = crate::test_support::context().await;
        let thread = crate::db::chat::create_thread(
            &ctx.state.db,
            crate::db::chat::CreateThreadInput {
                workspace_id: ctx.workspace_id.clone(),
                title: Some("Turno".into()),
                agent: "build".into(),
                model: None,
                source: Some("telegram".into()),
                external_id: Some("42".into()),
            },
        )
        .await
        .expect("thread");
        let turn = create(
            &ctx.state.db,
            CreateChatTurnInput {
                workspace_id: ctx.workspace_id.clone(),
                thread_id: thread.id.clone(),
                source: "telegram".into(),
                agent: "build".into(),
                model: None,
                user_content: "Gere um arquivo".into(),
            },
        )
        .await
        .expect("turn");
        assert_eq!(turn.status, "RUNNING");
        assert!(
            active_for_thread(&ctx.state.db, &thread.id)
                .await
                .expect("active")
                .is_some()
        );
        let finished = finish(
            &ctx.state.db,
            &turn.id,
            "SUCCESS",
            Some("pronto"),
            &RunUsage {
                tokens_input: 10,
                tokens_output: 20,
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
        assert!(finished.completed_at.is_some());

        let second = create(
            &ctx.state.db,
            CreateChatTurnInput {
                workspace_id: ctx.workspace_id.clone(),
                thread_id: thread.id,
                source: "telegram".into(),
                agent: "build".into(),
                model: None,
                user_content: "Outro".into(),
            },
        )
        .await
        .expect("second");
        assert!(second.id != turn.id);
        assert_eq!(mark_interrupted(&ctx.state.db).await.expect("interrupt"), 1);
    }
}
