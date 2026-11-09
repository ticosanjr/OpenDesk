use crate::{
    app_state::AppState,
    browse::{self, BrowseStatus},
    error::AppResult,
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct BrowseQuery {
    pub workspace_id: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct BrowseBody {
    pub workspace_id: Option<String>,
    pub scope: Option<String>,
}

fn scope(value: Option<&str>) -> String {
    match value.unwrap_or("workspace") {
        "global" => "global".into(),
        _ => "workspace".into(),
    }
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BrowseQuery>,
) -> AppResult<Json<BrowseStatus>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        browse::status(&state, &workspace_id, &scope(query.scope.as_deref())).await?,
    ))
}

pub async fn enable(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BrowseBody>,
) -> AppResult<Json<BrowseStatus>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    Ok(Json(
        browse::enable(&state, &workspace_id, &scope(body.scope.as_deref())).await?,
    ))
}

pub async fn disable(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BrowseBody>,
) -> AppResult<Json<BrowseStatus>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    Ok(Json(
        browse::disable(&state, &workspace_id, &scope(body.scope.as_deref())).await?,
    ))
}
