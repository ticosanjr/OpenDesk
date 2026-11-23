use super::RecipeQuery;
use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    recipes::{engine, parser, store},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct RunBody {
    #[serde(default)]
    pub trigger_context: serde_json::Value,
    #[serde(default)]
    pub dry_run: bool,
}

pub async fn run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RecipeQuery>,
    Json(body): Json<RunBody>,
) -> AppResult<Json<store::RecipeRun>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let _index = store::get(&state.db, &workspace_id, &id).await?;
    let yaml = store::yaml_for(&state.db, &workspace_id, &id).await?;
    let recipe = parser::parse(&yaml)?;
    let trigger = if body.trigger_context.is_null() {
        serde_json::json!({ "manual": true })
    } else {
        body.trigger_context
    };
    Ok(Json(
        engine::start(
            crate::app_state::recipe_context(&state),
            &workspace_id,
            recipe,
            trigger,
            body.dry_run,
        )
        .await?,
    ))
}

pub async fn runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RecipeQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "runs": store::list_runs(&state.db, &workspace_id, 100).await?,
    })))
}

pub async fn run_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let run = store::get_run(&state.db, &id).await?;
    let hitl = store::pending_hitl_for_run(&state.db, &id).await?;
    let nodes = store::list_run_nodes(&state.db, &id).await?;
    Ok(Json(
        serde_json::json!({ "run": run, "hitl": hitl, "nodes": nodes }),
    ))
}

#[derive(Debug, Deserialize, Default)]
pub struct HitlQuery {
    pub workspace_id: Option<String>,
}

pub async fn hitl_pending(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HitlQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "requests": store::pending_hitl(&state.db, &workspace_id).await?,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ResolveBody {
    pub resolution: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub option_index: Option<usize>,
    #[serde(default)]
    pub file_path: Option<String>,
}

pub async fn resolve_hitl(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Json(body): Json<ResolveBody>,
) -> AppResult<Json<serde_json::Value>> {
    let request = store::get_hitl(&state.db, &token).await?;
    let (kind, payload) = match body.resolution.to_uppercase().as_str() {
        "APPROVED" => ("APPROVED".to_string(), None),
        "REJECTED" => ("REJECTED".to_string(), body.text),
        "CHOICE" => (
            "CHOICE".to_string(),
            Some(
                serde_json::json!({
                    "option_index": body.option_index.unwrap_or(0),
                    "option_value": body.text.unwrap_or_default(),
                })
                .to_string(),
            ),
        ),
        "ARTIFACT" => (
            "ARTIFACT".to_string(),
            Some(serde_json::json!({ "path": body.file_path.unwrap_or_default() }).to_string()),
        ),
        _ => (
            "FEEDBACK".to_string(),
            Some(
                serde_json::json!({
                    "feedback_text": body.text.unwrap_or_default(),
                    "attached_files": body.file_path.map(|path| vec![path]).unwrap_or_default(),
                })
                .to_string(),
            ),
        ),
    };
    let resolved = crate::recipes::hitl::resolve(
        &crate::app_state::recipe_context(&state),
        &token,
        "DESKTOP_UI",
        None,
        &kind,
        payload.as_deref(),
    )
    .await?;
    if resolved.is_none() {
        return Err(AppError::Conflict(
            "checkpoint já resolvido ou expirado".into(),
        ));
    }
    Ok(Json(serde_json::json!({
        "ok": true,
        "run_id": request.recipe_run_id,
        "resolution": kind,
    })))
}

/// Boot/manual sync: recreate sources for every recipe and remove orphans.
pub async fn cancel_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<store::RecipeRun>> {
    let run = store::get_run(&state.db, &id).await?;
    if !matches!(
        run.status.as_str(),
        "RUNNING_STEP" | "RUNNING_SINKS" | "WAITING_HITL"
    ) {
        return Err(AppError::Conflict(format!(
            "run já terminou como {}",
            run.status
        )));
    }
    store::request_cancel(&state.db, &id).await?;
    if let Some(task_id) = run.task_id.as_deref()
        && let Ok(jobs) = crate::db::queue::list(&state.db, Some(&run.workspace_id)).await
    {
        for job in jobs
            .into_iter()
            .filter(|job| job.task_id.as_deref() == Some(task_id))
            .filter(|job| matches!(job.status.as_str(), "RUNNING" | "QUEUED" | "PAUSED"))
        {
            let _ = state.queue.cancel(&job.id).await;
        }
    }
    state.events.publish(OpenDeskWsEvent::RecipeRunUpdated {
        run_id: run.id.clone(),
        recipe_id: run.recipe_id.clone(),
        workspace_id: run.workspace_id.clone(),
        status: "CANCELLING".into(),
    });
    Ok(Json(store::get_run(&state.db, &id).await?))
}

pub async fn retry_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<store::RecipeRun>> {
    let run = store::get_run(&state.db, &id).await?;
    if !matches!(run.status.as_str(), "FAILED" | "REJECTED" | "TIMED_OUT") {
        return Err(AppError::Conflict(
            "só é possível repetir runs com falha, rejeitados ou expirados".into(),
        ));
    }
    let yaml = store::yaml_for(&state.db, &run.workspace_id, &run.recipe_id).await?;
    let recipe = parser::parse(&yaml)?;
    store::reset_for_retry(&state.db, &id).await?;
    let refreshed = store::get_run(&state.db, &id).await?;
    engine::resume(
        crate::app_state::recipe_context(&state),
        recipe,
        refreshed.clone(),
    )
    .await?;
    Ok(Json(refreshed))
}
