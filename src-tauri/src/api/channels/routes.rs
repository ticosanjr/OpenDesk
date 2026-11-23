use super::WorkspaceQuery;
use crate::{
    app_state::AppState,
    db::channels::{self, ChannelRoute, UpsertRouteInput},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use std::sync::Arc;

pub async fn list_routes(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> AppResult<Json<Vec<ChannelRoute>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(channels::list(&state.db, &workspace_id).await?))
}

pub async fn upsert_route(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
    Json(input): Json<UpsertRouteInput>,
) -> AppResult<Json<ChannelRoute>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        channels::upsert(&state.db, &workspace_id, input).await?,
    ))
}

pub async fn delete_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    if !channels::delete(&state.db, &id).await? {
        return Err(AppError::NotFound(format!("channel route {id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}
