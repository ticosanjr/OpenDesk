use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{AgentDelegation, DelegationInput};

const DELEGATION_COLUMNS: &str = "id, workspace_id, from_agent, to_agent, title, prompt, \
    status, task_id, requested_by, requested_at, resolved_at, resolved_by, error";

pub async fn list_delegations(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<AgentDelegation>> {
    let query = format!(
        "SELECT {DELEGATION_COLUMNS} FROM agent_delegations WHERE workspace_id = ?
         ORDER BY CASE status WHEN 'PENDING' THEN 0 ELSE 1 END, requested_at DESC LIMIT 100"
    );
    Ok(sqlx::query_as::<_, AgentDelegation>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn create_delegation(
    pool: &SqlitePool,
    workspace_id: &str,
    input: DelegationInput,
) -> AppResult<AgentDelegation> {
    if input.from_agent.trim().is_empty() || input.to_agent.trim().is_empty() {
        return Err(AppError::BadRequest(
            "agente de origem e destino são obrigatórios".into(),
        ));
    }
    if input.title.trim().is_empty() || input.prompt.trim().is_empty() {
        return Err(AppError::BadRequest(
            "título e prompt da delegação são obrigatórios".into(),
        ));
    }
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO agent_delegations (
            id, workspace_id, from_agent, to_agent, title, prompt, requested_by
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(input.from_agent.trim())
    .bind(input.to_agent.trim())
    .bind(input.title.trim())
    .bind(input.prompt.trim())
    .bind(input.requested_by.as_deref())
    .execute(pool)
    .await?;
    get_delegation(pool, workspace_id, &id).await
}

pub async fn get_delegation(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
) -> AppResult<AgentDelegation> {
    let query = format!(
        "SELECT {DELEGATION_COLUMNS} FROM agent_delegations WHERE id = ? AND workspace_id = ?"
    );
    sqlx::query_as::<_, AgentDelegation>(&query)
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("delegation {id}")))
}

pub async fn resolve_delegation(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
    status: &str,
    task_id: Option<&str>,
    resolved_by: Option<&str>,
    error: Option<&str>,
) -> AppResult<AgentDelegation> {
    sqlx::query(
        "UPDATE agent_delegations SET status = ?, task_id = ?, resolved_by = ?, error = ?,
            resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND workspace_id = ? AND status = 'PENDING'",
    )
    .bind(status)
    .bind(task_id)
    .bind(resolved_by)
    .bind(error)
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    get_delegation(pool, workspace_id, id).await
}
