use crate::error::{AppError, AppResult};
use chrono::{Duration, Utc};
use sqlx::SqlitePool;

use super::{AgentProfile, AgentProfileInput};

const PROFILE_COLUMNS: &str = "workspace_id, agent_id, role, team, manager_id, \
    monthly_budget_usd, concurrency_quota, heartbeat_enabled, heartbeat_minutes, \
    last_heartbeat_at, next_heartbeat_at, notes, updated_at";

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
