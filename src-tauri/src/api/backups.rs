use crate::{app_state::AppState, backup, error::AppResult};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct BackupQuery {
    pub workspace_id: Option<String>,
}

async fn workspace_id(state: &AppState, query: &BackupQuery) -> AppResult<String> {
    crate::api::tasks::resolve_workspace_id(state, query.workspace_id.as_deref()).await
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BackupQuery>,
) -> AppResult<Json<Vec<backup::WorkspaceBackup>>> {
    let workspace_id = workspace_id(&state, &query).await?;
    Ok(Json(backup::list(&state, &workspace_id)?))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BackupQuery>,
) -> AppResult<Json<backup::WorkspaceBackup>> {
    let workspace_id = workspace_id(&state, &query).await?;
    Ok(Json(backup::create(&state, &workspace_id).await?))
}

pub async fn download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<BackupQuery>,
) -> AppResult<impl IntoResponse> {
    let workspace_id = workspace_id(&state, &query).await?;
    let (path, backup) = backup::get(&state, &workspace_id, &id)?;
    let bytes = std::fs::read(&path)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", backup.file_name),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        bytes,
    ))
}

pub async fn restore(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<BackupQuery>,
    Json(body): Json<backup::RestoreInput>,
) -> AppResult<Json<crate::db::workspaces::Workspace>> {
    let workspace_id = workspace_id(&state, &query).await?;
    Ok(Json(
        backup::restore_as_new(&state, &workspace_id, &id, body).await?,
    ))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<BackupQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id = workspace_id(&state, &query).await?;
    backup::delete(&state, &workspace_id, &id)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
