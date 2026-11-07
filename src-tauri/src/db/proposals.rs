use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConfigProposal {
    pub id: String,
    pub workspace_id: String,
    pub kind: String,
    pub action: String,
    pub title: String,
    pub description: Option<String>,
    pub payload_json: String,
    pub status: String,
    pub requested_by: Option<String>,
    pub source_run_id: Option<String>,
    pub source_task_id: Option<String>,
    pub apply_error: Option<String>,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProposalInput {
    pub kind: String,
    #[serde(default = "default_action")]
    pub action: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub requested_by: Option<String>,
    #[serde(default)]
    pub source_run_id: Option<String>,
    #[serde(default)]
    pub source_task_id: Option<String>,
}

fn default_action() -> String {
    "CREATE".into()
}

const COLUMNS: &str = "id, workspace_id, kind, action, title, description, payload_json, status, \
    requested_by, source_run_id, source_task_id, apply_error, created_at, resolved_at, resolved_by";

pub async fn create(
    pool: &SqlitePool,
    workspace_id: &str,
    input: ProposalInput,
) -> AppResult<ConfigProposal> {
    let kind = input.kind.trim().to_uppercase();
    if !matches!(
        kind.as_str(),
        "AGENT" | "SKILL" | "COMMAND" | "RECIPE" | "ROUTINE" | "MCP" | "CONFIG"
    ) {
        return Err(AppError::BadRequest(format!(
            "proposal kind inválido '{kind}'"
        )));
    }
    let action = input.action.trim().to_uppercase();
    if !matches!(action.as_str(), "CREATE" | "UPDATE" | "DELETE") {
        return Err(AppError::BadRequest(format!(
            "proposal action inválida '{action}'"
        )));
    }
    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("proposal title é obrigatório".into()));
    }
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO config_proposals (
            id, workspace_id, kind, action, title, description, payload_json,
            requested_by, source_run_id, source_task_id
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(&kind)
    .bind(&action)
    .bind(input.title.trim())
    .bind(input.description.as_deref())
    .bind(input.payload.to_string())
    .bind(input.requested_by.as_deref())
    .bind(input.source_run_id.as_deref())
    .bind(input.source_task_id.as_deref())
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ConfigProposal> {
    let query = format!("SELECT {COLUMNS} FROM config_proposals WHERE id = ?");
    sqlx::query_as::<_, ConfigProposal>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("proposal {id}")))
}

pub async fn pending(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<ConfigProposal>> {
    let query = format!(
        "SELECT {COLUMNS} FROM config_proposals
         WHERE workspace_id = ? AND status = 'PENDING' ORDER BY created_at"
    );
    Ok(sqlx::query_as::<_, ConfigProposal>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn finalize(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    error: Option<&str>,
) -> AppResult<ConfigProposal> {
    sqlx::query("UPDATE config_proposals SET status = ?, apply_error = ? WHERE id = ?")
        .bind(status)
        .bind(error)
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}

pub async fn resolve(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    resolved_by: Option<&str>,
    error: Option<&str>,
) -> AppResult<ConfigProposal> {
    sqlx::query(
        "UPDATE config_proposals SET status = ?, resolved_by = ?, apply_error = ?,
            resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND status = 'PENDING'",
    )
    .bind(status)
    .bind(resolved_by)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}
