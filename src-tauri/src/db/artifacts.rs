use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArtifactProvenance {
    pub id: String,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub run_id: String,
    pub file_path: String,
    pub absolute_path: String,
    pub file_type: String,
    pub file_size_bytes: i64,
    pub content_sha256: Option<String>,
    pub previous_sha256: Option<String>,
    pub action_type: String,
    pub created_by_agent: String,
    pub creation_reason: String,
    pub diff_patch: Option<String>,
    pub google_doc_url: Option<String>,
    pub google_drive_url: Option<String>,
    pub emailed_to: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateArtifactInput {
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub run_id: String,
    pub file_path: String,
    pub absolute_path: String,
    pub file_type: String,
    pub file_size_bytes: i64,
    pub content_sha256: Option<String>,
    pub previous_sha256: Option<String>,
    pub action_type: String,
    pub created_by_agent: String,
    pub creation_reason: String,
    pub diff_patch: Option<String>,
}

const COLUMNS: &str = "id, workspace_id, task_id, run_id, file_path, absolute_path, file_type, \
    file_size_bytes, content_sha256, previous_sha256, action_type, created_by_agent, \
    creation_reason, diff_patch, google_doc_url, google_drive_url, emailed_to, created_at";

pub async fn create(
    pool: &SqlitePool,
    input: CreateArtifactInput,
) -> AppResult<ArtifactProvenance> {
    if !matches!(
        input.action_type.as_str(),
        "CREATED" | "MODIFIED" | "DELETED"
    ) {
        return Err(AppError::BadRequest(format!(
            "invalid artifact action_type '{}'",
            input.action_type
        )));
    }

    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO artifacts_provenance (
            id, workspace_id, task_id, run_id, file_path, absolute_path, file_type,
            file_size_bytes, content_sha256, previous_sha256, action_type,
            created_by_agent, creation_reason, diff_patch
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(input.task_id.as_deref())
    .bind(&input.run_id)
    .bind(&input.file_path)
    .bind(&input.absolute_path)
    .bind(&input.file_type)
    .bind(input.file_size_bytes)
    .bind(input.content_sha256.as_deref())
    .bind(input.previous_sha256.as_deref())
    .bind(&input.action_type)
    .bind(&input.created_by_agent)
    .bind(&input.creation_reason)
    .bind(input.diff_patch.as_deref())
    .execute(pool)
    .await?;

    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ArtifactProvenance> {
    let query = format!("SELECT {COLUMNS} FROM artifacts_provenance WHERE id = ?");
    sqlx::query_as::<_, ArtifactProvenance>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("artifact {id}")))
}

pub async fn list(
    pool: &SqlitePool,
    workspace_id: &str,
    agent: Option<&str>,
    task_id: Option<&str>,
    action: Option<&str>,
    limit: i64,
) -> AppResult<Vec<ArtifactProvenance>> {
    let query = format!(
        "SELECT {COLUMNS} FROM artifacts_provenance
         WHERE workspace_id = ?
           AND (? IS NULL OR created_by_agent = ?)
           AND (? IS NULL OR task_id = ?)
           AND (? IS NULL OR action_type = ?)
         ORDER BY created_at DESC
         LIMIT ?"
    );
    Ok(sqlx::query_as::<_, ArtifactProvenance>(&query)
        .bind(workspace_id)
        .bind(agent)
        .bind(agent)
        .bind(task_id)
        .bind(task_id)
        .bind(action)
        .bind(action)
        .bind(limit.clamp(1, 1000))
        .fetch_all(pool)
        .await?)
}

pub async fn exists(pool: &SqlitePool, run_id: &str, file_path: &str) -> AppResult<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM artifacts_provenance WHERE run_id = ? AND file_path = ?",
    )
    .bind(run_id)
    .bind(file_path)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn count_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM artifacts_provenance WHERE task_id = ?")
            .bind(task_id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn set_destinations(
    pool: &SqlitePool,
    id: &str,
    google_doc_url: Option<&str>,
    google_drive_url: Option<&str>,
    emailed_to: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE artifacts_provenance SET
            google_doc_url = COALESCE(?, google_doc_url),
            google_drive_url = COALESCE(?, google_drive_url),
            emailed_to = COALESCE(?, emailed_to)
         WHERE id = ?",
    )
    .bind(google_doc_url)
    .bind(google_drive_url)
    .bind(emailed_to)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
