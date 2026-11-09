use crate::{app_state::AppState, error::AppResult, metrics};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct MetricsQuery {
    pub workspace_id: Option<String>,
    pub days: Option<i64>,
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MetricsQuery>,
) -> AppResult<Json<metrics::Metrics>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        metrics::collect(&state.db, &workspace_id, query.days.unwrap_or(30)).await?,
    ))
}
