use crate::{
    app_state::AppState,
    db::{permissions, proposals},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
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
    let awaiting = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String)>(
        "SELECT t.id, t.title, t.phase_status, t.plan_markdown, t.updated_at FROM tasks t
         WHERE t.workspace_id = ?
           AND t.origin = 'MANUAL'
           AND (t.status = 'IN_REVIEW' OR t.phase_status = 'awaiting_approval')
           AND NOT EXISTS (
               SELECT 1 FROM alert_dismissals d
               WHERE d.workspace_id = t.workspace_id AND d.kind = 'APPROVAL'
                 AND d.ref_id = t.id AND d.version = t.updated_at
           )
         ORDER BY t.updated_at DESC LIMIT 20",
    )
    .bind(&workspace_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id, title, phase_status, plan, version)| {
        let plan_excerpt = plan.map(|markdown| {
            let trimmed = markdown.trim();
            if trimmed.chars().count() > 600 {
                format!("{}…", trimmed.chars().take(600).collect::<String>())
            } else {
                trimmed.to_string()
            }
        });
        serde_json::json!({
            "id": id, "title": title, "phase_status": phase_status,
            "plan_excerpt": plan_excerpt, "version": version
        })
    })
    .collect::<Vec<_>>();

    let blocked = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT t.id, t.title, t.blocked_reason, t.updated_at FROM tasks t
         WHERE t.workspace_id = ? AND t.status = 'BLOCKED' AND t.origin = 'MANUAL'
           AND NOT EXISTS (
               SELECT 1 FROM alert_dismissals d
               WHERE d.workspace_id = t.workspace_id AND d.kind = 'BLOCKED'
                 AND d.ref_id = t.id AND d.version = t.updated_at
           )
         ORDER BY t.updated_at DESC LIMIT 20",
    )
    .bind(&workspace_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id, title, reason, version)| {
        serde_json::json!({
            "id": id, "title": title, "reason": reason, "version": version
        })
    })
    .collect::<Vec<_>>();

    let failed = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            String,
        ),
    >(
        "SELECT r.id, r.task_id, t.title, r.status, r.error_message, r.started_at
         FROM task_runs r LEFT JOIN tasks t ON t.id = r.task_id
         WHERE r.workspace_id = ? AND r.status = 'FAILED'
           AND COALESCE(t.origin, 'MANUAL') = 'MANUAL'
           AND r.started_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-24 hours')
           AND NOT EXISTS (
               SELECT 1 FROM alert_dismissals d
               WHERE d.workspace_id = r.workspace_id AND d.kind = 'FAILED_RUN'
                 AND d.ref_id = r.id AND d.version = r.started_at
           )
         ORDER BY r.started_at DESC LIMIT 10",
    )
    .bind(&workspace_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id, task_id, title, status, error, version)| {
        serde_json::json!({
            "id": id, "task_id": task_id, "title": title, "status": status,
            "error": error, "version": version
        })
    })
    .collect::<Vec<_>>();

    // One request feeds the whole Attention Card. Previously the card made
    // four parallel calls and could render partially loaded sections.
    let permissions = permissions::pending(&state.db, &workspace_id).await?;
    let hitl = crate::recipes::store::pending_hitl(&state.db, &workspace_id).await?;
    let proposals = proposals::pending(&state.db, &workspace_id).await?;

    Ok(Json(serde_json::json!({
        "awaiting_approval": awaiting,
        "blocked": blocked,
        "failed_runs": failed,
        "requests": permissions,
        "hitl": hitl,
        "proposals": proposals,
    })))
}

#[derive(Debug, Deserialize)]
pub struct DismissBody {
    pub kind: String,
    pub ref_id: String,
    #[serde(default)]
    pub version: Option<String>,
    pub workspace_id: Option<String>,
}

/// Hide one attention item until its underlying reference changes. This is a
/// UI decision only; the task or run itself is untouched.
pub async fn dismiss(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DismissBody>,
) -> AppResult<StatusCode> {
    let kind = body.kind.trim().to_uppercase();
    let ref_id = body.ref_id.trim();
    if ref_id.is_empty() || !matches!(kind.as_str(), "APPROVAL" | "BLOCKED" | "FAILED_RUN") {
        return Err(AppError::BadRequest(
            "kind precisa ser APPROVAL, BLOCKED ou FAILED_RUN, e ref_id é obrigatório".into(),
        ));
    }
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    let owned: Option<i64> = match kind.as_str() {
        "FAILED_RUN" => {
            sqlx::query_scalar("SELECT 1 FROM task_runs WHERE id = ? AND workspace_id = ?")
                .bind(ref_id)
                .bind(&workspace_id)
                .fetch_optional(&state.db)
                .await?
        }
        _ => {
            sqlx::query_scalar("SELECT 1 FROM tasks WHERE id = ? AND workspace_id = ?")
                .bind(ref_id)
                .bind(&workspace_id)
                .fetch_optional(&state.db)
                .await?
        }
    };
    if owned.is_none() {
        return Err(AppError::NotFound("item de atenção não encontrado".into()));
    }
    let id = uuid::Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO alert_dismissals (id, workspace_id, kind, ref_id, version)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(workspace_id, kind, ref_id) DO UPDATE SET
            version = excluded.version,
            dismissed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(&id)
    .bind(&workspace_id)
    .bind(&kind)
    .bind(ref_id)
    .bind(body.version.as_deref().unwrap_or(""))
    .execute(&state.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
