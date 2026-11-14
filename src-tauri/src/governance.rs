//! Agent governance: hierarchy, goals, budgets, heartbeats and delegations.
//! Agent definitions remain native OpenCode files; this module stores only
//! operational governance state in SQLite.

use crate::{
    app_state::AppState,
    db::tasks::{self, CreateTaskInput},
    error::{AppError, AppResult},
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentProfile {
    pub workspace_id: String,
    pub agent_id: String,
    pub role: Option<String>,
    pub team: Option<String>,
    pub manager_id: Option<String>,
    pub monthly_budget_usd: Option<f64>,
    pub concurrency_quota: i64,
    pub heartbeat_enabled: bool,
    pub heartbeat_minutes: i64,
    pub last_heartbeat_at: Option<String>,
    pub next_heartbeat_at: Option<String>,
    pub notes: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentProfileInput {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub manager_id: Option<String>,
    #[serde(default)]
    pub monthly_budget_usd: Option<f64>,
    #[serde(default)]
    pub concurrency_quota: Option<i64>,
    #[serde(default)]
    pub heartbeat_enabled: Option<bool>,
    #[serde(default)]
    pub heartbeat_minutes: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentGoal {
    pub id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub title: String,
    pub description: Option<String>,
    pub metric: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: f64,
    pub unit: Option<String>,
    pub due_at: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentGoalInput {
    pub agent_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub target_value: Option<f64>,
    #[serde(default)]
    pub current_value: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentDelegation {
    pub id: String,
    pub workspace_id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub title: String,
    pub prompt: String,
    pub status: String,
    pub task_id: Option<String>,
    pub requested_by: Option<String>,
    pub requested_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DelegationInput {
    pub from_agent: String,
    pub to_agent: String,
    pub title: String,
    pub prompt: String,
    #[serde(default)]
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AgentPerformance {
    pub agent_id: String,
    pub runs: i64,
    pub success: i64,
    pub failed: i64,
    pub cancelled: i64,
    pub cost_usd: f64,
    pub avg_duration_ms: f64,
    pub tokens: i64,
    #[sqlx(default)]
    pub success_rate: f64,
}

const PROFILE_COLUMNS: &str = "workspace_id, agent_id, role, team, manager_id, \
    monthly_budget_usd, concurrency_quota, heartbeat_enabled, heartbeat_minutes, \
    last_heartbeat_at, next_heartbeat_at, notes, updated_at";
const GOAL_COLUMNS: &str = "id, workspace_id, agent_id, title, description, metric, \
    target_value, current_value, unit, due_at, status, created_at, updated_at";
const DELEGATION_COLUMNS: &str = "id, workspace_id, from_agent, to_agent, title, prompt, \
    status, task_id, requested_by, requested_at, resolved_at, resolved_by, error";

pub async fn list_profiles(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<AgentProfile>> {
    let query = format!(
        "SELECT {PROFILE_COLUMNS} FROM agent_profiles WHERE workspace_id = ? ORDER BY agent_id"
    );
    Ok(sqlx::query_as::<_, AgentProfile>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get_profile(
    pool: &SqlitePool,
    workspace_id: &str,
    agent_id: &str,
) -> AppResult<Option<AgentProfile>> {
    let query = format!(
        "SELECT {PROFILE_COLUMNS} FROM agent_profiles WHERE workspace_id = ? AND agent_id = ?"
    );
    Ok(sqlx::query_as::<_, AgentProfile>(&query)
        .bind(workspace_id)
        .bind(agent_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn upsert_profile(
    pool: &SqlitePool,
    workspace_id: &str,
    agent_id: &str,
    input: AgentProfileInput,
) -> AppResult<AgentProfile> {
    let agent_id = agent_id.trim();
    if agent_id.is_empty() {
        return Err(AppError::BadRequest("agent id é obrigatório".into()));
    }
    if input.monthly_budget_usd.is_some_and(|value| value < 0.0) {
        return Err(AppError::BadRequest("budget não pode ser negativo".into()));
    }
    let existing = get_profile(pool, workspace_id, agent_id).await?;
    let now = Utc::now();
    let heartbeat_enabled = input.heartbeat_enabled.unwrap_or_else(|| {
        existing
            .as_ref()
            .is_some_and(|profile| profile.heartbeat_enabled)
    });
    let heartbeat_minutes = input
        .heartbeat_minutes
        .unwrap_or_else(|| {
            existing
                .as_ref()
                .map_or(60, |profile| profile.heartbeat_minutes)
        })
        .clamp(5, 24 * 60);
    let next_heartbeat = if heartbeat_enabled {
        existing
            .as_ref()
            .and_then(|profile| profile.next_heartbeat_at.clone())
            .filter(|_| {
                existing
                    .as_ref()
                    .is_some_and(|profile| profile.heartbeat_enabled)
            })
            .or_else(|| Some((now + Duration::minutes(heartbeat_minutes)).to_rfc3339()))
    } else {
        None
    };
    let quota = input
        .concurrency_quota
        .unwrap_or_else(|| {
            existing
                .as_ref()
                .map_or(5, |profile| profile.concurrency_quota)
        })
        .clamp(1, 32);
    sqlx::query(
        "INSERT INTO agent_profiles (
            workspace_id, agent_id, role, team, manager_id, monthly_budget_usd,
            concurrency_quota, heartbeat_enabled, heartbeat_minutes, next_heartbeat_at, notes
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(workspace_id, agent_id) DO UPDATE SET
            role = excluded.role,
            team = excluded.team,
            manager_id = excluded.manager_id,
            monthly_budget_usd = excluded.monthly_budget_usd,
            concurrency_quota = excluded.concurrency_quota,
            heartbeat_enabled = excluded.heartbeat_enabled,
            heartbeat_minutes = excluded.heartbeat_minutes,
            next_heartbeat_at = excluded.next_heartbeat_at,
            notes = excluded.notes,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(workspace_id)
    .bind(agent_id)
    .bind(input.role.as_deref())
    .bind(input.team.as_deref())
    .bind(input.manager_id.as_deref())
    .bind(input.monthly_budget_usd)
    .bind(quota)
    .bind(heartbeat_enabled)
    .bind(heartbeat_minutes)
    .bind(next_heartbeat.as_deref())
    .bind(input.notes.as_deref())
    .execute(pool)
    .await?;
    get_profile(pool, workspace_id, agent_id)
        .await?
        .ok_or_else(|| AppError::Internal("agent profile missing after upsert".into()))
}

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

pub async fn due_heartbeats(pool: &SqlitePool) -> AppResult<Vec<AgentProfile>> {
    let query = format!(
        "SELECT {PROFILE_COLUMNS} FROM agent_profiles
         WHERE heartbeat_enabled = 1
           AND (next_heartbeat_at IS NULL OR next_heartbeat_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ORDER BY next_heartbeat_at"
    );
    Ok(sqlx::query_as::<_, AgentProfile>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn mark_heartbeat(
    pool: &SqlitePool,
    workspace_id: &str,
    agent_id: &str,
    minutes: i64,
) -> AppResult<()> {
    let next = (Utc::now() + Duration::minutes(minutes.clamp(5, 24 * 60))).to_rfc3339();
    sqlx::query(
        "UPDATE agent_profiles SET
            last_heartbeat_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            next_heartbeat_at = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE workspace_id = ? AND agent_id = ?",
    )
    .bind(&next)
    .bind(workspace_id)
    .bind(agent_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn heartbeat_prompt(agent_id: &str, goals: &str) -> (String, String) {
    (
        format!("Heartbeat · @{agent_id}"),
        format!(
            "Você é @{agent_id} no seu ciclo periódico de governança.\n\n## Metas ativas\n\n{goals}\n\n## Tarefa do heartbeat\n\n1. Revise o estado atual e o que já foi entregue.\n2. Identifique o próximo passo de maior impacto para cada meta.\n3. Proponha ações concretas, riscos e o que precisa de aprovação humana.\n4. Não execute ações destrutivas nem altere configurações sem propor HITL.\n5. Responda de forma curta e acionável; artefatos vão para a pasta de saída do run."
        ),
    )
}

/// Creates and dispatches one heartbeat task for an agent.
pub async fn run_heartbeat(
    state: &Arc<AppState>,
    workspace_id: &str,
    agent_id: &str,
) -> AppResult<tasks::Task> {
    check_budget(&state.db, workspace_id, agent_id).await?;
    let goals = active_goal_summary(&state.db, workspace_id, agent_id).await?;
    let (title, description) = heartbeat_prompt(agent_id, &goals);
    let task = tasks::create(
        &state.db,
        workspace_id,
        CreateTaskInput {
            title,
            description,
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: Some(agent_id.to_string()),
            preferred_model: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags: vec!["governance:heartbeat".into()],
            origin: Some("ROUTINE".into()),
            parent_task_id: None,
            dedupe: Some(false),
        },
    )
    .await?;
    let body = crate::api::tasks::DispatchBody {
        policy: None,
        manual_agent: Some(agent_id.to_string()),
        manual_model: None,
        manual_effort: None,
        execution_mode: None,
        prompt_append: None,
    };
    if let Err(error) = crate::api::tasks::dispatch_task_internal(state, &task.id, body, None).await
    {
        tracing::warn!(
            agent_id,
            task_id = %task.id,
            error = %error,
            "heartbeat task created without dispatch"
        );
    }
    tasks::get(&state.db, &task.id).await
}

pub fn start_heartbeat(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            if let Err(error) = heartbeat_tick(&state).await {
                tracing::warn!(error = %error, "agent heartbeat tick failed");
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}

async fn heartbeat_tick(state: &Arc<AppState>) -> AppResult<()> {
    for profile in due_heartbeats(&state.db).await? {
        // Claim the slot before doing work so a slow run cannot double-fire.
        mark_heartbeat(
            &state.db,
            &profile.workspace_id,
            &profile.agent_id,
            profile.heartbeat_minutes,
        )
        .await?;
        if let Err(error) = run_heartbeat(state, &profile.workspace_id, &profile.agent_id).await {
            tracing::warn!(
                workspace_id = %profile.workspace_id,
                agent_id = %profile.agent_id,
                error = %error,
                "heartbeat skipped"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[tokio::test]
    async fn profile_goals_performance_and_budget() {
        let ctx = test_support::context().await;
        let profile = upsert_profile(
            &ctx.state.db,
            &ctx.workspace_id,
            "build",
            AgentProfileInput {
                role: Some("Executor".into()),
                team: Some("Entrega".into()),
                manager_id: Some("plan".into()),
                monthly_budget_usd: Some(1.0),
                concurrency_quota: Some(2),
                heartbeat_enabled: Some(false),
                heartbeat_minutes: Some(30),
                notes: Some("teste".into()),
            },
        )
        .await
        .expect("profile");
        assert_eq!(profile.role.as_deref(), Some("Executor"));
        assert_eq!(profile.concurrency_quota, 2);

        let goal = create_goal(
            &ctx.state.db,
            &ctx.workspace_id,
            AgentGoalInput {
                agent_id: "build".into(),
                title: "Entregar 10 tarefas".into(),
                description: None,
                metric: Some("tarefas".into()),
                target_value: Some(10.0),
                current_value: Some(3.0),
                unit: Some("tarefas".into()),
                due_at: None,
                status: None,
            },
        )
        .await
        .expect("goal");
        assert_eq!(goal.current_value, 3.0);
        assert!(
            active_goal_summary(&ctx.state.db, &ctx.workspace_id, "build")
                .await
                .expect("summary")
                .contains("10")
        );

        let budget = check_budget(&ctx.state.db, &ctx.workspace_id, "build").await;
        assert!(budget.is_ok(), "budget: {budget:?}");
        sqlx::query(
            "INSERT INTO task_runs (
                id, workspace_id, agent_name, execution_mode, model_id, effort_level,
                status, estimated_cost_usd, duration_ms
             ) VALUES ('run-gov', ?, 'build', 'build', 'm', 'high', 'SUCCESS', 2.0, 1000)",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("run");
        assert!(
            check_budget(&ctx.state.db, &ctx.workspace_id, "build")
                .await
                .is_err()
        );
        let rows = performance(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("performance");
        assert_eq!(rows[0].agent_id, "build");
        assert_eq!(rows[0].success, 1);
        assert!((rows[0].success_rate - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn delegation_lifecycle() {
        let ctx = test_support::context().await;
        let delegation = create_delegation(
            &ctx.state.db,
            &ctx.workspace_id,
            DelegationInput {
                from_agent: "plan".into(),
                to_agent: "build".into(),
                title: "Implementar".into(),
                prompt: "Faça o build.".into(),
                requested_by: Some("test".into()),
            },
        )
        .await
        .expect("delegation");
        assert_eq!(delegation.status, "PENDING");
        let resolved = resolve_delegation(
            &ctx.state.db,
            &ctx.workspace_id,
            &delegation.id,
            "APPROVED",
            None,
            Some("test"),
            None,
        )
        .await
        .expect("resolve");
        assert_eq!(resolved.status, "APPROVED");
    }
}
