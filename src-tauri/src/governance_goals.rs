use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{AgentGoal, AgentGoalInput};

const GOAL_COLUMNS: &str = "id, workspace_id, agent_id, title, description, metric, \
    target_value, current_value, unit, due_at, status, created_at, updated_at";

pub async fn list_goals(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<AgentGoal>> {
    let query = format!(
        "SELECT {GOAL_COLUMNS} FROM agent_goals WHERE workspace_id = ?
         ORDER BY CASE status WHEN 'ACTIVE' THEN 0 WHEN 'PAUSED' THEN 1 ELSE 2 END,
                  due_at IS NULL, due_at, created_at"
    );
    Ok(sqlx::query_as::<_, AgentGoal>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn create_goal(
    pool: &SqlitePool,
    workspace_id: &str,
    input: AgentGoalInput,
) -> AppResult<AgentGoal> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest("goal title é obrigatório".into()));
    }
    if input.agent_id.trim().is_empty() {
        return Err(AppError::BadRequest("goal agent_id é obrigatório".into()));
    }
    let status = input.status.as_deref().unwrap_or("ACTIVE").to_uppercase();
    if !matches!(status.as_str(), "ACTIVE" | "DONE" | "PAUSED" | "CANCELLED") {
        return Err(AppError::BadRequest(format!(
            "status de meta inválido '{status}'"
        )));
    }
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO agent_goals (
            id, workspace_id, agent_id, title, description, metric,
            target_value, current_value, unit, due_at, status
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(input.agent_id.trim())
    .bind(title)
    .bind(input.description.as_deref())
    .bind(input.metric.as_deref())
    .bind(input.target_value)
    .bind(input.current_value.unwrap_or(0.0))
    .bind(input.unit.as_deref())
    .bind(input.due_at.as_deref())
    .bind(&status)
    .execute(pool)
    .await?;
    get_goal(pool, workspace_id, &id).await
}

pub async fn update_goal(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
    input: AgentGoalInput,
) -> AppResult<AgentGoal> {
    let current = get_goal(pool, workspace_id, id).await?;
    let status = input
        .status
        .as_deref()
        .map(str::to_uppercase)
        .unwrap_or(current.status);
    if !matches!(status.as_str(), "ACTIVE" | "DONE" | "PAUSED" | "CANCELLED") {
        return Err(AppError::BadRequest(format!(
            "status de meta inválido '{status}'"
        )));
    }
    sqlx::query(
        "UPDATE agent_goals SET
            agent_id = ?, title = ?, description = ?, metric = ?, target_value = ?,
            current_value = ?, unit = ?, due_at = ?, status = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND workspace_id = ?",
    )
    .bind(input.agent_id.trim())
    .bind(input.title.trim())
    .bind(input.description.or(current.description))
    .bind(input.metric.or(current.metric))
    .bind(input.target_value.or(current.target_value))
    .bind(input.current_value.unwrap_or(current.current_value))
    .bind(input.unit.or(current.unit))
    .bind(input.due_at.or(current.due_at))
    .bind(&status)
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    get_goal(pool, workspace_id, id).await
}

pub async fn get_goal(pool: &SqlitePool, workspace_id: &str, id: &str) -> AppResult<AgentGoal> {
    let query = format!("SELECT {GOAL_COLUMNS} FROM agent_goals WHERE id = ? AND workspace_id = ?");
    sqlx::query_as::<_, AgentGoal>(&query)
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("goal {id}")))
}

pub async fn delete_goal(pool: &SqlitePool, workspace_id: &str, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM agent_goals WHERE id = ? AND workspace_id = ?")
        .bind(id)
        .bind(workspace_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn active_goal_summary(
    pool: &SqlitePool,
    workspace_id: &str,
    agent_id: &str,
) -> AppResult<String> {
    let goals = sqlx::query_as::<_, (String, Option<f64>, f64, Option<String>)>(
        "SELECT title, target_value, current_value, unit FROM agent_goals
         WHERE workspace_id = ? AND agent_id = ? AND status = 'ACTIVE'
         ORDER BY due_at IS NULL, due_at LIMIT 8",
    )
    .bind(workspace_id)
    .bind(agent_id)
    .fetch_all(pool)
    .await?;
    if goals.is_empty() {
        return Ok("- Nenhuma meta ativa registrada.".into());
    }
    Ok(goals
        .into_iter()
        .map(|(title, target, current, unit)| {
            let unit = unit.unwrap_or_default();
            match target {
                Some(target) => format!("- {title}: {current:.0}/{target:.0} {unit}"),
                None => format!("- {title}: {current:.0} {unit}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n"))
}
