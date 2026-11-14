use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

pub const CHANNELS: [&str; 2] = ["telegram", "signal"];

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskWatcher {
    pub id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub channel: String,
    pub recipient: Option<String>,
    pub created_at: String,
}

const COLUMNS: &str = "id, workspace_id, task_id, channel, recipient, created_at";

pub async fn list_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<TaskWatcher>> {
    let query = format!(
        "SELECT {COLUMNS} FROM task_watchers WHERE task_id = ? ORDER BY channel, created_at"
    );
    Ok(sqlx::query_as::<_, TaskWatcher>(&query)
        .bind(task_id)
        .fetch_all(pool)
        .await?)
}

/// Replaces the watcher set for a task. The UI sends the enabled channels, so
/// this is an idempotent reconcile instead of a sequence of add/remove calls.
pub async fn replace(
    pool: &SqlitePool,
    workspace_id: &str,
    task_id: &str,
    channels: &[String],
) -> AppResult<Vec<TaskWatcher>> {
    let mut wanted = Vec::new();
    for channel in channels {
        let channel = channel.trim().to_lowercase();
        if channel.is_empty() || wanted.iter().any(|item| item == &channel) {
            continue;
        }
        if !CHANNELS.contains(&channel.as_str()) {
            return Err(AppError::BadRequest(format!(
                "canal de acompanhamento inválido: {channel}"
            )));
        }
        wanted.push(channel);
    }

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM task_watchers WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut *tx)
        .await?;
    for channel in &wanted {
        sqlx::query(
            "INSERT INTO task_watchers (id, workspace_id, task_id, channel, recipient)
             VALUES (?, ?, ?, ?, NULL)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(workspace_id)
        .bind(task_id)
        .bind(channel)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    list_for_task(pool, task_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replaces_and_deduplicates_watchers() {
        let ctx = crate::test_support::context().await;
        let task = crate::db::tasks::create(
            &ctx.state.db,
            &ctx.workspace_id,
            crate::db::tasks::CreateTaskInput {
                title: "Acompanhar".into(),
                description: "Teste de watcher".into(),
                execution_mode: None,
                priority: None,
                assigned_agent: None,
                preferred_model: None,
                effort_level: None,
                care_level: None,
                notify_email: None,
                export_to_gdocs: false,
                tags: Vec::new(),
                origin: None,
                parent_task_id: None,
                dedupe: None,
            },
        )
        .await
        .expect("task");

        let watchers = replace(
            &ctx.state.db,
            &ctx.workspace_id,
            &task.id,
            &["telegram".into(), "telegram".into()],
        )
        .await
        .expect("replace");
        assert_eq!(watchers.len(), 1);
        assert_eq!(watchers[0].channel, "telegram");

        let watchers = replace(
            &ctx.state.db,
            &ctx.workspace_id,
            &task.id,
            &["signal".into()],
        )
        .await
        .expect("replace again");
        assert_eq!(watchers.len(), 1);
        assert_eq!(watchers[0].channel, "signal");
    }
}
