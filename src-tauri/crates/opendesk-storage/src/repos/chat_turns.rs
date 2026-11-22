use crate::{db::runs::RunUsage, error::AppResult};
use sqlx::SqlitePool;

use crate::repos::conversations::turns as repo;
pub use crate::repos::conversations::turns::{ChatTurn, ChatTurnUsage, CreateChatTurnInput};

pub async fn create(pool: &SqlitePool, input: CreateChatTurnInput) -> AppResult<ChatTurn> {
    repo::create(pool, input).await
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
    let usage = ChatTurnUsage {
        tokens_input: usage.tokens_input,
        tokens_output: usage.tokens_output,
        tokens_reasoning: usage.tokens_reasoning,
        tokens_cache_read: usage.tokens_cache_read,
        tokens_cache_write: usage.tokens_cache_write,
        estimated_cost_usd: usage.estimated_cost_usd,
    };
    repo::finish(
        pool,
        id,
        status,
        assistant_content,
        &usage,
        duration_ms,
        error_code,
        error_message,
    )
    .await
}

pub async fn set_run(pool: &SqlitePool, id: &str, run_id: &str) -> AppResult<()> {
    repo::set_run(pool, id, run_id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ChatTurn> {
    repo::get(pool, id).await
}

pub async fn latest_for_thread(pool: &SqlitePool, thread_id: &str) -> AppResult<Option<ChatTurn>> {
    repo::latest_for_thread(pool, thread_id).await
}

pub async fn active_for_thread(pool: &SqlitePool, thread_id: &str) -> AppResult<Option<ChatTurn>> {
    repo::active_for_thread(pool, thread_id).await
}

pub async fn mark_interrupted(pool: &SqlitePool) -> AppResult<u64> {
    repo::mark_interrupted(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_and_interrupts_a_chat_turn() {
        let ctx = crate::test_support::context().await;
        let thread = crate::repos::chat::create_thread(
            &ctx.db,
            crate::repos::chat::CreateThreadInput {
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
            &ctx.db,
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
            active_for_thread(&ctx.db, &thread.id)
                .await
                .expect("active")
                .is_some()
        );
        let finished = finish(
            &ctx.db,
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
            &ctx.db,
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
        assert_eq!(mark_interrupted(&ctx.db).await.expect("interrupt"), 1);
    }
}
