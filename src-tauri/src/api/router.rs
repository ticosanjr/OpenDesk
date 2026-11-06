use crate::{
    app_state::AppState,
    db::{
        budget,
        router::{self, CreateRuleInput, RouterRule, UpdateRuleInput},
    },
    error::AppResult,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct RouterQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct RouterConfig {
    pub workspace_id: String,
    pub policy: String,
    pub rules: Vec<RouterRule>,
    pub budget: budget::BudgetStatus,
}

pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RouterQuery>,
) -> AppResult<Json<RouterConfig>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let policy = router::workspace_policy(&state.db, &workspace_id).await?;
    let rules = router::list(&state.db, &workspace_id).await?;
    let budget = budget::status(&state.db).await?;
    Ok(Json(RouterConfig {
        workspace_id,
        policy,
        rules,
        budget,
    }))
}

pub async fn list_rules(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RouterQuery>,
) -> AppResult<Json<Vec<RouterRule>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(router::list(&state.db, &workspace_id).await?))
}

pub async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateRuleInput>,
) -> AppResult<Json<RouterRule>> {
    Ok(Json(router::create(&state.db, input).await?))
}

pub async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateRuleInput>,
) -> AppResult<Json<RouterRule>> {
    Ok(Json(router::update(&state.db, &id, input).await?))
}

pub async fn delete_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    router::delete(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
