use crate::error::AppResult;
use sqlx::SqlitePool;

use opendesk_storage::repos::conversations::interactions as repo;
pub use opendesk_storage::repos::conversations::interactions::{
    CreateInteractionInput, TaskInteraction,
};

pub async fn create(
    pool: &SqlitePool,
    input: CreateInteractionInput,
) -> AppResult<TaskInteraction> {
    Ok(repo::create(pool, input).await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<TaskInteraction> {
    Ok(repo::get(pool, id).await?)
}

pub async fn pending_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<TaskInteraction>> {
    Ok(repo::pending_for_task(pool, task_id).await?)
}

pub async fn pending_for_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<TaskInteraction>> {
    Ok(repo::pending_for_workspace(pool, workspace_id).await?)
}

pub async fn find_pending_by_prefix(
    pool: &SqlitePool,
    prefix: &str,
) -> AppResult<Option<TaskInteraction>> {
    Ok(repo::find_pending_by_prefix(pool, prefix).await?)
}

pub async fn resolve(
    pool: &SqlitePool,
    id: &str,
    answer: &serde_json::Value,
    status: &str,
) -> AppResult<TaskInteraction> {
    Ok(repo::resolve(pool, id, answer, status).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn finds_pending_interactions_by_short_prefix() {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("interactions.db"))
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

        let created = create(
            &pool,
            CreateInteractionInput {
                workspace_id: "ws-1".into(),
                task_id: "task-1".into(),
                run_id: None,
                question: "Aprovar?".into(),
                context_markdown: None,
                options: vec![],
                kind: "TEXT".into(),
                multiple: false,
                allow_custom: true,
                expires_at: "2999-01-01T00:00:00Z".into(),
            },
        )
        .await
        .expect("interaction");

        let prefix = &created.id[..8];
        let found = find_pending_by_prefix(&pool, prefix)
            .await
            .expect("find")
            .expect("interaction");
        assert_eq!(found.task_id, "task-1");
        assert!(
            find_pending_by_prefix(&pool, "abc")
                .await
                .expect("short")
                .is_none()
        );
    }
}
