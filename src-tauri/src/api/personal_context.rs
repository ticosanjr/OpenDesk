use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    personal_context::{self, PersonalContextGroup, PersonalContextInput},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct PersonalContextQuery {
    pub workspace_id: Option<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PersonalContextQuery>,
) -> AppResult<Json<Vec<PersonalContextGroup>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        personal_context::list(&state.db, &workspace_id).await?,
    ))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PersonalContextQuery>,
    Json(mut input): Json<PersonalContextInput>,
) -> AppResult<Json<PersonalContextGroup>> {
    if input.scope.trim().eq_ignore_ascii_case("workspace") && input.workspace_id.is_none() {
        input.workspace_id = Some(
            crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?,
        );
    }
    if !input.scope.trim().eq_ignore_ascii_case("workspace")
        && !input.scope.trim().eq_ignore_ascii_case("global")
    {
        return Err(AppError::BadRequest(
            "scope deve ser 'workspace' ou 'global'".into(),
        ));
    }
    Ok(Json(personal_context::create(&state.db, input).await?))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut input): Json<PersonalContextInput>,
) -> AppResult<Json<PersonalContextGroup>> {
    let current = personal_context::get(&state.db, &id).await?;
    if input.scope.trim().eq_ignore_ascii_case("workspace") && input.workspace_id.is_none() {
        input.workspace_id = current.workspace_id;
    }
    Ok(Json(personal_context::update(&state.db, &id, input).await?))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    personal_context::delete(&state.db, &id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
