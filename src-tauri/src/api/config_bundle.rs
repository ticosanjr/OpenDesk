use crate::{
    app_state::AppState,
    config_bundle::{self, ConfigBundle, ExportRequest, ImportRequest, MAX_BUNDLE_BYTES},
    error::{AppError, AppResult},
};
use axum::{Json, extract::State};
use std::sync::Arc;

async fn workspace(state: &AppState, requested: Option<&str>) -> AppResult<String> {
    crate::api::tasks::resolve_workspace_id(state, requested).await
}

pub async fn export(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ExportRequest>,
) -> AppResult<Json<ConfigBundle>> {
    let workspace_id = workspace(&state, request.workspace_id.as_deref()).await?;
    Ok(Json(
        config_bundle::export(&state.db, &workspace_id, &request).await?,
    ))
}

pub async fn preview(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ImportRequest>,
) -> AppResult<Json<config_bundle::PreviewReport>> {
    let bytes = serde_json::to_vec(&request.bundle)
        .map_err(|error| AppError::BadRequest(format!("bundle inválido: {error}")))?;
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(AppError::BadRequest("bundle excede 10 MB".into()));
    }
    let workspace_id = workspace(&state, request.workspace_id.as_deref()).await?;
    Ok(Json(
        config_bundle::preview(
            &state.db,
            &workspace_id,
            &request.bundle,
            request.sections.as_deref(),
        )
        .await?,
    ))
}

pub async fn import(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ImportRequest>,
) -> AppResult<Json<config_bundle::PreviewReport>> {
    let bytes = serde_json::to_vec(&request.bundle)
        .map_err(|error| AppError::BadRequest(format!("bundle inválido: {error}")))?;
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(AppError::BadRequest("bundle excede 10 MB".into()));
    }
    let workspace_id = workspace(&state, request.workspace_id.as_deref()).await?;
    Ok(Json(
        config_bundle::import(&state.db, &workspace_id, &request).await?,
    ))
}
