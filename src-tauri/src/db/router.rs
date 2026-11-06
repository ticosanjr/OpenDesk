use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RouterRule {
    pub id: String,
    pub workspace_id: Option<String>,
    pub name: String,
    pub priority_order: i64,
    pub condition_json: String,
    pub target_mode: String,
    pub target_agent: Option<String>,
    pub target_model: String,
    pub target_effort: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRuleInput {
    pub workspace_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub priority_order: i64,
    pub condition_json: String,
    pub target_mode: String,
    pub target_agent: Option<String>,
    pub target_model: String,
    pub target_effort: String,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateRuleInput {
    pub name: Option<String>,
    pub priority_order: Option<i64>,
    pub condition_json: Option<String>,
    pub target_mode: Option<String>,
    pub target_agent: Option<String>,
    pub target_model: Option<String>,
    pub target_effort: Option<String>,
    pub is_enabled: Option<bool>,
}

fn default_true() -> bool {
    true
}

const COLUMNS: &str = "id, workspace_id, name, priority_order, condition_json, target_mode, \
    target_agent, target_model, target_effort, is_enabled";

fn validate_mode(mode: &str) -> AppResult<String> {
    let mode = mode.trim().to_lowercase();
    if matches!(mode.as_str(), "plan" | "build" | "plan_then_build") {
        Ok(mode)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid target_mode '{mode}'"
        )))
    }
}

fn validate_effort(effort: &str) -> AppResult<String> {
    let effort = effort.trim().to_lowercase();
    if matches!(effort.as_str(), "low" | "medium" | "high") {
        Ok(effort)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid target_effort '{effort}'"
        )))
    }
}

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<RouterRule>> {
    let query = format!(
        "SELECT {COLUMNS} FROM router_rules
         WHERE workspace_id IS NULL OR workspace_id = ?
         ORDER BY priority_order ASC, name ASC"
    );
    Ok(sqlx::query_as::<_, RouterRule>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<RouterRule> {
    let query = format!("SELECT {COLUMNS} FROM router_rules WHERE id = ?");
    sqlx::query_as::<_, RouterRule>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("router rule {id}")))
}

pub async fn create(pool: &SqlitePool, input: CreateRuleInput) -> AppResult<RouterRule> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("rule name is required".into()));
    }
    if input.target_model.trim().is_empty() {
        return Err(AppError::BadRequest("target_model is required".into()));
    }
    serde_json::from_str::<serde_json::Value>(&input.condition_json)
        .map_err(|error| AppError::BadRequest(format!("invalid condition_json: {error}")))?;
    let mode = validate_mode(&input.target_mode)?;
    let effort = validate_effort(&input.target_effort)?;

    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO router_rules (
            id, workspace_id, name, priority_order, condition_json,
            target_mode, target_agent, target_model, target_effort, is_enabled
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(input.workspace_id.as_deref())
    .bind(input.name.trim())
    .bind(input.priority_order)
    .bind(&input.condition_json)
    .bind(&mode)
    .bind(input.target_agent.as_deref())
    .bind(input.target_model.trim())
    .bind(&effort)
    .bind(input.is_enabled)
    .execute(pool)
    .await?;

    get(pool, &id).await
}

pub async fn update(pool: &SqlitePool, id: &str, input: UpdateRuleInput) -> AppResult<RouterRule> {
    let current = get(pool, id).await?;
    let mode = match input.target_mode {
        Some(mode) => validate_mode(&mode)?,
        None => current.target_mode,
    };
    let effort = match input.target_effort {
        Some(effort) => validate_effort(&effort)?,
        None => current.target_effort,
    };
    let condition = input.condition_json.unwrap_or(current.condition_json);
    serde_json::from_str::<serde_json::Value>(&condition)
        .map_err(|error| AppError::BadRequest(format!("invalid condition_json: {error}")))?;

    sqlx::query(
        "UPDATE router_rules SET
            name = ?, priority_order = ?, condition_json = ?, target_mode = ?,
            target_agent = ?, target_model = ?, target_effort = ?, is_enabled = ?
         WHERE id = ?",
    )
    .bind(input.name.unwrap_or(current.name))
    .bind(input.priority_order.unwrap_or(current.priority_order))
    .bind(&condition)
    .bind(&mode)
    .bind(input.target_agent.or(current.target_agent))
    .bind(input.target_model.unwrap_or(current.target_model))
    .bind(&effort)
    .bind(input.is_enabled.unwrap_or(current.is_enabled))
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM router_rules WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("router rule {id}")));
    }
    Ok(())
}

pub async fn workspace_policy(pool: &SqlitePool, workspace_id: &str) -> AppResult<String> {
    Ok(
        sqlx::query_scalar("SELECT default_router_policy FROM workspaces WHERE id = ?")
            .bind(workspace_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| "COST_BENEFIT".to_string()),
    )
}
