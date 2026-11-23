use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

use super::AgentPerformance;
use super::profiles::get_profile;

pub async fn performance(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<AgentPerformance>> {
    let mut rows = sqlx::query_as::<_, AgentPerformance>(
        "SELECT r.agent_name AS agent_id,
                COUNT(*) AS runs,
                COALESCE(SUM(r.status = 'SUCCESS'), 0) AS success,
                COALESCE(SUM(r.status = 'FAILED'), 0) AS failed,
                COALESCE(SUM(r.status = 'CANCELLED'), 0) AS cancelled,
                CAST(COALESCE(SUM(r.estimated_cost_usd), 0) AS REAL) AS cost_usd,
                COALESCE(AVG(r.duration_ms), 0) AS avg_duration_ms,
                COALESCE(SUM(r.tokens_input + r.tokens_output), 0) AS tokens
         FROM task_runs r
         WHERE r.workspace_id = ?
           AND r.started_at >= strftime('%Y-%m-%dT%H:%M:%fZ','now','-30 days')
         GROUP BY r.agent_name
         ORDER BY runs DESC",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    for row in &mut rows {
        row.success_rate = if row.runs > 0 {
            row.success as f64 / row.runs as f64
        } else {
            0.0
        };
    }
    Ok(rows)
}

pub async fn monthly_spend(
    pool: &SqlitePool,
    workspace_id: &str,
    agent_id: &str,
) -> AppResult<f64> {
    Ok(sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(estimated_cost_usd), 0) AS REAL) FROM task_runs
         WHERE workspace_id = ? AND agent_name = ?
           AND started_at >= strftime('%Y-%m-01T00:00:00Z','now')",
    )
    .bind(workspace_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await?)
}

/// Monthly spend per agent for the whole workspace in one query. The dashboard
/// used to run one `monthly_spend` per profile.
pub async fn monthly_spend_by_agent(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<std::collections::HashMap<String, f64>> {
    let rows = sqlx::query_as::<_, (String, f64)>(
        "SELECT agent_name, CAST(COALESCE(SUM(estimated_cost_usd), 0) AS REAL) FROM task_runs
         WHERE workspace_id = ?
           AND started_at >= strftime('%Y-%m-01T00:00:00Z','now')
         GROUP BY agent_name",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

pub async fn check_budget(pool: &SqlitePool, workspace_id: &str, agent_id: &str) -> AppResult<()> {
    let Some(profile) = get_profile(pool, workspace_id, agent_id).await? else {
        return Ok(());
    };
    let Some(limit) = profile.monthly_budget_usd.filter(|value| *value > 0.0) else {
        return Ok(());
    };
    let spent = monthly_spend(pool, workspace_id, agent_id).await?;
    if spent >= limit {
        return Err(AppError::Conflict(format!(
            "budget do agente @{agent_id} esgotado: US$ {spent:.2} de US$ {limit:.2} neste mês"
        )));
    }
    Ok(())
}
