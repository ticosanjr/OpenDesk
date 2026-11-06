use crate::{
    app_state::AppState,
    db::budget::{self, BudgetSettings, BudgetStatus, UpdateBudgetInput},
    error::AppResult,
};
use axum::{Json, extract::State};
use std::sync::Arc;

pub async fn get_budget(State(state): State<Arc<AppState>>) -> AppResult<Json<BudgetSettings>> {
    Ok(Json(budget::get(&state.db).await?))
}

pub async fn put_budget(
    State(state): State<Arc<AppState>>,
    Json(input): Json<UpdateBudgetInput>,
) -> AppResult<Json<BudgetSettings>> {
    Ok(Json(budget::update(&state.db, input).await?))
}

pub async fn budget_status(State(state): State<Arc<AppState>>) -> AppResult<Json<BudgetStatus>> {
    Ok(Json(budget::status(&state.db).await?))
}
