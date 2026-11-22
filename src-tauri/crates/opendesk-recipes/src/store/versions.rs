use crate::error::{AppError, AppResult};

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::RecipeIndex;
use super::recipes::{save_yaml, yaml_for};
use super::sha256;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecipeVersionSummary {
    pub id: String,
    pub content_sha256: String,
    pub size_bytes: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecipeVersion {
    pub id: String,
    pub workspace_id: String,
    pub recipe_id: String,
    pub content: String,
    pub content_sha256: String,
    pub created_at: String,
}

pub(super) async fn record_version(
    pool: &SqlitePool,
    workspace_id: &str,
    recipe_id: &str,
    content: &str,
) -> AppResult<()> {
    let hash = sha256(content);
    let latest: Option<String> = sqlx::query_scalar(
        "SELECT content_sha256 FROM recipe_versions
         WHERE workspace_id = ? AND recipe_id = ?
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(workspace_id)
    .bind(recipe_id)
    .fetch_optional(pool)
    .await?;
    if latest.as_deref() == Some(hash.as_str()) {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO recipe_versions
            (id, workspace_id, recipe_id, content, content_sha256)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(workspace_id)
    .bind(recipe_id)
    .bind(content)
    .bind(&hash)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_versions(
    pool: &SqlitePool,
    workspace_id: &str,
    recipe_id: &str,
) -> AppResult<Vec<RecipeVersionSummary>> {
    Ok(sqlx::query_as::<_, RecipeVersionSummary>(
        "SELECT id, content_sha256, length(content) AS size_bytes, created_at
         FROM recipe_versions
         WHERE workspace_id = ? AND recipe_id = ?
         ORDER BY created_at DESC, id DESC",
    )
    .bind(workspace_id)
    .bind(recipe_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_version(
    pool: &SqlitePool,
    workspace_id: &str,
    recipe_id: &str,
    version_id: &str,
) -> AppResult<RecipeVersion> {
    sqlx::query_as::<_, RecipeVersion>(
        "SELECT id, workspace_id, recipe_id, content, content_sha256, created_at
         FROM recipe_versions
         WHERE id = ? AND workspace_id = ? AND recipe_id = ?",
    )
    .bind(version_id)
    .bind(workspace_id)
    .bind(recipe_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("versão {version_id} da receita {recipe_id}")))
}

/// Unified diff from a stored version to the current file (or another version).
pub async fn diff_version(
    pool: &SqlitePool,
    workspace_id: &str,
    recipe_id: &str,
    version_id: &str,
    against: Option<&str>,
) -> AppResult<String> {
    let from = get_version(pool, workspace_id, recipe_id, version_id).await?;
    let to = match against.filter(|value| *value != "current") {
        Some(other) => {
            get_version(pool, workspace_id, recipe_id, other)
                .await?
                .content
        }
        None => yaml_for(pool, workspace_id, recipe_id).await?,
    };
    Ok(similar::TextDiff::from_lines(&from.content, &to)
        .unified_diff()
        .context_radius(3)
        .header(
            &format!("{}@{}", recipe_id, &from.id[..8.min(from.id.len())]),
            "current",
        )
        .to_string())
}

pub async fn rollback_version(
    pool: &SqlitePool,
    workspace_id: &str,
    recipe_id: &str,
    version_id: &str,
) -> AppResult<RecipeIndex> {
    let version = get_version(pool, workspace_id, recipe_id, version_id).await?;
    save_yaml(pool, workspace_id, recipe_id, &version.content).await
}
