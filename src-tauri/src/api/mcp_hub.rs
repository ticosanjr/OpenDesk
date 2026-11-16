use crate::{
    app_state::AppState,
    error::AppResult,
    mcp_hub::{self, CatalogEntry, HubServer, InstallInput, TestResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct HubQuery {
    pub workspace_id: Option<String>,
}

pub async fn servers(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HubQuery>,
) -> AppResult<Json<Vec<HubServer>>> {
    Ok(Json(
        mcp_hub::inventory(&state, query.workspace_id.as_deref()).await?,
    ))
}

pub async fn catalog() -> Json<Vec<CatalogEntry>> {
    Json(mcp_hub::catalog())
}

pub async fn install(
    State(state): State<Arc<AppState>>,
    Json(input): Json<InstallInput>,
) -> AppResult<Json<HubServer>> {
    Ok(Json(mcp_hub::install(&state, input).await?))
}

#[derive(Debug, Deserialize)]
pub struct ToggleBody {
    pub disabled: bool,
}

pub async fn toggle(
    State(state): State<Arc<AppState>>,
    Path((scope, name)): Path<(String, String)>,
    Query(query): Query<HubQuery>,
    Json(body): Json<ToggleBody>,
) -> AppResult<StatusCode> {
    mcp_hub::toggle(
        &state,
        &scope,
        &name,
        body.disabled,
        query.workspace_id.as_deref(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test(
    State(state): State<Arc<AppState>>,
    Path((scope, name)): Path<(String, String)>,
    Query(query): Query<HubQuery>,
) -> AppResult<Json<TestResult>> {
    Ok(Json(
        mcp_hub::test(&state, &scope, &name, query.workspace_id.as_deref()).await?,
    ))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path((scope, name)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    mcp_hub::delete(&state, &scope, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}
