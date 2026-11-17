//! A/B experiment groups for worktree comparisons.
//!
//! A group owns two ordinary tasks tagged `ab:<group>` and `ab-slot:<slot>`.
//! The queue creates one isolated worktree per task, and the API compares the
//! resulting runs before merging the chosen slot.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AbGroup {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub prompt: String,
    pub status: String,
    pub winner_slot: Option<String>,
    pub created_at: String,
    pub decided_at: Option<String>,
}

const COLUMNS: &str =
    "id, workspace_id, title, prompt, status, winner_slot, created_at, decided_at";

pub async fn create(
    pool: &SqlitePool,
    workspace_id: &str,
    title: &str,
    prompt: &str,
) -> AppResult<AbGroup> {
    let title = title.trim();
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(AppError::BadRequest("prompt do A/B é obrigatório".into()));
    }
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO run_ab_groups (id, workspace_id, title, prompt, status)
         VALUES (?, ?, ?, ?, 'OPEN')",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(if title.is_empty() {
        "A/B sem título"
    } else {
        title
    })
    .bind(prompt)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<AbGroup> {
    sqlx::query_as::<_, AbGroup>(&format!("SELECT {COLUMNS} FROM run_ab_groups WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("grupo A/B {id}")))
}

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<AbGroup>> {
    Ok(sqlx::query_as::<_, AbGroup>(&format!(
        "SELECT {COLUMNS} FROM run_ab_groups
         WHERE workspace_id = ? ORDER BY created_at DESC LIMIT 50"
    ))
    .bind(workspace_id)
    .fetch_all(pool)
    .await?)
}

pub async fn decide(pool: &SqlitePool, id: &str, slot: &str) -> AppResult<AbGroup> {
    if slot != "a" && slot != "b" {
        return Err(AppError::BadRequest("slot A/B inválido".into()));
    }
    let result = sqlx::query(
        "UPDATE run_ab_groups
         SET status = 'DECIDED', winner_slot = ?,
             decided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND status = 'OPEN'",
    )
    .bind(slot)
    .bind(id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "grupo A/B já decidido ou inexistente".into(),
        ));
    }
    get(pool, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn creates_lists_and_decides_a_group() {
        let ctx = crate::test_support::context().await;
        let group = create(&ctx.db, &ctx.workspace_id, "Teste", "faça algo")
            .await
            .expect("group");
        assert_eq!(
            list(&ctx.db, &ctx.workspace_id).await.expect("list").len(),
            1
        );
        let decided = decide(&ctx.db, &group.id, "a").await.expect("decide");
        assert_eq!(decided.status, "DECIDED");
        assert_eq!(decided.winner_slot.as_deref(), Some("a"));
        assert!(decide(&ctx.db, &group.id, "b").await.is_err());
        assert!(create(&ctx.db, &ctx.workspace_id, "", "  ").await.is_err());
    }
}
