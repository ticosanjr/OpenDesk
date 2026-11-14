use crate::{
    app_state::AppState,
    db::{self, permissions},
    error::AppResult,
    events::OpenDeskWsEvent,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct PermissionQuery {
    pub workspace_id: Option<String>,
}

pub async fn list_permissions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PermissionQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "requests": permissions::pending(&state.db, &workspace_id).await?,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ReplyBody {
    pub decision: String,
}

pub async fn reply_permission(
    State(state): State<Arc<AppState>>,
    Path(request_id): Path<String>,
    Json(body): Json<ReplyBody>,
) -> AppResult<Json<permissions::PermissionRequest>> {
    Ok(Json(
        decide_internal(&state, &request_id, &body.decision).await?,
    ))
}

/// Shared by the HTTP route, Telegram buttons and text replies.
pub(crate) async fn decide_internal(
    state: &Arc<AppState>,
    request_id: &str,
    decision: &str,
) -> AppResult<permissions::PermissionRequest> {
    let request = permissions::decide(&state.db, request_id, decision).await?;
    state.events.publish(OpenDeskWsEvent::PermissionDecided {
        request_id: request.request_id.clone(),
        decision: decision.to_string(),
    });
    Ok(request)
}

#[derive(Debug, serde::Serialize)]
pub struct AgentState {
    pub agent: String,
    pub state: String,
    pub running: i64,
    pub pending: i64,
}

pub async fn agent_states(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PermissionQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
        "SELECT r.agent_name, r.status, r.started_at, r.completed_at
         FROM task_runs r
         WHERE r.workspace_id = ?
         ORDER BY r.started_at DESC LIMIT 500",
    )
    .bind(&workspace_id)
    .fetch_all(&state.db)
    .await?;

    let pending = sqlx::query_as::<_, (String, i64)>(
        "SELECT r.agent_name, COUNT(*)
         FROM permission_requests p
         JOIN task_runs r ON r.id = p.run_id
         WHERE p.workspace_id = ? AND p.status = 'PENDING'
         GROUP BY r.agent_name",
    )
    .bind(&workspace_id)
    .fetch_all(&state.db)
    .await?;

    let mut states: std::collections::BTreeMap<String, AgentState> = Default::default();
    for (agent, status, _started, _completed) in rows {
        let entry = states.entry(agent.clone()).or_insert(AgentState {
            agent,
            state: "IDLE".into(),
            running: 0,
            pending: 0,
        });
        if status == "RUNNING" {
            entry.running += 1;
        }
        if entry.state != "BLOCKED" && entry.state != "WORKING" {
            entry.state = if status == "RUNNING" {
                "WORKING".into()
            } else if status == "SUCCESS" {
                "DONE".into()
            } else {
                "IDLE".into()
            };
        }
    }
    for (agent, count) in pending {
        let entry = states.entry(agent.clone()).or_insert(AgentState {
            agent,
            state: "BLOCKED".into(),
            running: 0,
            pending: 0,
        });
        entry.pending = count;
        entry.state = "BLOCKED".into();
    }
    for entry in states.values_mut() {
        if entry.pending > 0 {
            entry.state = "BLOCKED".into();
        } else if entry.running > 0 {
            entry.state = "WORKING".into();
        }
    }

    let mut agents = states.into_values().collect::<Vec<_>>();
    agents.sort_by(|left, right| {
        let rank = |state: &str| match state {
            "BLOCKED" => 0,
            "WORKING" => 1,
            "DONE" => 2,
            _ => 3,
        };
        rank(&left.state)
            .cmp(&rank(&right.state))
            .then_with(|| left.agent.cmp(&right.agent))
    });

    Ok(Json(serde_json::json!({ "agents": agents })))
}

pub async fn context(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let task = db::tasks::get(&state.db, &id).await?;
    let run = sqlx::query_as::<_, (Option<String>, i64, i64)>(
        "SELECT session_id, tokens_input, tokens_output FROM task_runs
         WHERE task_id = ? ORDER BY started_at DESC LIMIT 1",
    )
    .bind(&task.id)
    .fetch_optional(&state.db)
    .await?;
    let (session_id, tokens_input, tokens_output) = run.unwrap_or((None, 0, 0));
    let messages = match (&state.daemon.client().await, session_id.as_deref()) {
        (Some(client), Some(session_id)) => client.context(session_id).await.unwrap_or_default(),
        _ => Vec::new(),
    };
    Ok(Json(serde_json::json!({
        "session_id": session_id,
        "tokens_input": tokens_input,
        "tokens_output": tokens_output,
        "window": 128_000,
        "percent": (tokens_input as f64 / 128_000.0) * 100.0,
        "messages": messages.len(),
    })))
}

pub async fn compact(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let task = db::tasks::get(&state.db, &id).await?;
    let session_id = db::runs::latest_session_for_task(&state.db, &task.id)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::BadRequest("tarefa ainda não tem sessão para compactar".into())
        })?;
    let client = state
        .daemon
        .client()
        .await
        .ok_or_else(|| crate::error::AppError::Daemon("OpenCode is offline".into()))?;
    let result = client
        .compact_session(&session_id)
        .await
        .map_err(|error| crate::error::AppError::Daemon(format!("{error:#}")))?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "session_id": session_id,
        "compaction": result.get("data"),
    })))
}
