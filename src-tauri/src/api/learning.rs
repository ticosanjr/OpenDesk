use crate::{
    app_state::AppState,
    db::{proposals, workspaces},
    events::OpenDeskWsEvent,
    learning::{self, LearningSettings},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct LearningQuery {
    pub workspace_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ScanQuery {
    pub workspace_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ResolveLearningBody {
    #[serde(default)]
    pub requested_by: Option<String>,
}

pub async fn overview(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LearningQuery>,
) -> crate::error::AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let settings = learning::get_settings(&state.db, &workspace_id).await?;
    let stats = learning::stats(&state.db, &workspace_id).await?;
    let lessons = learning::list_lessons(
        &state.db,
        &workspace_id,
        query.status.as_deref(),
        query.limit.unwrap_or(100),
    )
    .await?;
    let pending = proposals::pending(&state.db, &workspace_id).await?;
    Ok(Json(json!({
        "settings": settings,
        "stats": stats,
        "lessons": lessons,
        "proposals": pending,
    })))
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LearningQuery>,
    Json(settings): Json<LearningSettings>,
) -> crate::error::AppResult<Json<LearningSettings>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let saved = learning::set_settings(&state.db, &workspace_id, settings).await?;
    state.events.publish(OpenDeskWsEvent::LearningUpdated {
        workspace_id: workspace_id.clone(),
    });
    Ok(Json(saved))
}

pub async fn scan(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ScanQuery>,
) -> crate::error::AppResult<Json<learning::ScanResult>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let result = learning::scan(&state.db, &workspace_id, query.limit.unwrap_or(50)).await?;
    state.events.publish(OpenDeskWsEvent::LearningUpdated {
        workspace_id: workspace_id.clone(),
    });
    Ok(Json(result))
}

pub async fn dismiss(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<LearningQuery>,
) -> crate::error::AppResult<Json<learning::LearningLesson>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let lesson = learning::dismiss(&state.db, &workspace_id, &id).await?;
    state.events.publish(OpenDeskWsEvent::LearningUpdated {
        workspace_id: workspace_id.clone(),
    });
    Ok(Json(lesson))
}

pub async fn remember(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<LearningQuery>,
) -> crate::error::AppResult<Json<learning::LearningLesson>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let lesson = learning::remember(
        &state.db,
        std::path::Path::new(&workspace.root_path),
        &state.paths.backups_dir,
        &workspace_id,
        &id,
    )
    .await?;
    state.events.publish(OpenDeskWsEvent::LearningUpdated {
        workspace_id: workspace_id.clone(),
    });
    Ok(Json(lesson))
}

pub async fn promote(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<LearningQuery>,
    body: Option<Json<ResolveLearningBody>>,
) -> crate::error::AppResult<Json<proposals::ConfigProposal>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let requested_by = body
        .and_then(|Json(body)| body.requested_by)
        .or_else(|| Some("learning".into()));
    let proposal =
        learning::promote(&state.db, &workspace_id, &id, requested_by.as_deref()).await?;
    state.events.publish(OpenDeskWsEvent::LearningUpdated {
        workspace_id: workspace_id.clone(),
    });
    Ok(Json(proposal))
}

pub async fn restore(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<LearningQuery>,
) -> crate::error::AppResult<Json<learning::LearningLesson>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let lesson = learning::restore(&state.db, &workspace_id, &id).await?;
    state.events.publish(OpenDeskWsEvent::LearningUpdated {
        workspace_id: workspace_id.clone(),
    });
    Ok(Json(lesson))
}

pub async fn forget(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<LearningQuery>,
) -> crate::error::AppResult<Json<learning::LearningLesson>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let lesson = learning::forget(
        &state.db,
        std::path::Path::new(&workspace.root_path),
        &state.paths.backups_dir,
        &workspace_id,
        &id,
    )
    .await?;
    state.events.publish(OpenDeskWsEvent::LearningUpdated {
        workspace_id: workspace_id.clone(),
    });
    Ok(Json(lesson))
}
