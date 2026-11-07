use crate::{app_state::AppState, error::AppResult};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct AttentionQuery {
    pub workspace_id: Option<String>,
}

pub async fn attention(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AttentionQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let awaiting = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, title, phase_status FROM tasks
         WHERE workspace_id = ? AND (status = 'IN_REVIEW' OR phase_status = 'awaiting_approval')
         ORDER BY updated_at DESC LIMIT 20",
    )
    .bind(&workspace_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id, title, phase_status)| {
        serde_json::json!({
            "id": id, "title": title, "phase_status": phase_status
        })
    })
    .collect::<Vec<_>>();

    let blocked = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, title, blocked_reason FROM tasks
         WHERE workspace_id = ? AND status = 'BLOCKED'
         ORDER BY updated_at DESC LIMIT 20",
    )
    .bind(&workspace_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id, title, reason)| {
        serde_json::json!({
            "id": id, "title": title, "reason": reason
        })
    })
    .collect::<Vec<_>>();

    let failed = sqlx::query_as::<_, (String, Option<String>, String, Option<String>)>(
        "SELECT r.id, t.title, r.status, r.error_message
         FROM task_runs r LEFT JOIN tasks t ON t.id = r.task_id
         WHERE r.workspace_id = ? AND r.status = 'FAILED'
           AND r.started_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-24 hours')
         ORDER BY r.started_at DESC LIMIT 10",
    )
    .bind(&workspace_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id, title, status, error)| {
        serde_json::json!({
            "id": id, "title": title, "status": status, "error": error
        })
    })
    .collect::<Vec<_>>();

    Ok(Json(serde_json::json!({
        "awaiting_approval": awaiting,
        "blocked": blocked,
        "failed_runs": failed,
    })))
}
