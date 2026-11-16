use crate::{
    app_state::AppState,
    db::workspaces,
    error::{AppError, AppResult},
    workspace_sync,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Deserialize, Default)]
pub struct BundleBody {
    /// Manifest known by the node; omitted means "send everything".
    #[serde(default)]
    pub base: Vec<workspace_sync::FileEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyBody {
    pub bundle_path: String,
    #[serde(default)]
    pub dry_run: bool,
}

pub async fn manifest(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace = workspaces::get(&state.db, &id).await?;
    let root = PathBuf::from(workspace.root_path);
    let entries = tokio::task::spawn_blocking(move || workspace_sync::manifest(&root))
        .await
        .map_err(|error| AppError::Internal(format!("sync task: {error}")))??;
    Ok(Json(json!({ "workspace_id": id, "files": entries })))
}

pub async fn bundle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<BundleBody>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace = workspaces::get(&state.db, &id).await?;
    let root = PathBuf::from(workspace.root_path);
    let destination = state
        .paths
        .data_dir
        .join("sync")
        .join(format!("{}.tar.zst", uuid::Uuid::now_v7()));
    let base = body.base;
    let destination_for_task = destination.clone();
    let (current, delta, report) = tokio::task::spawn_blocking(move || {
        let current = workspace_sync::manifest(&root)?;
        let delta = workspace_sync::delta(&base, &current);
        let files = delta
            .added
            .iter()
            .chain(delta.changed.iter())
            .cloned()
            .collect::<Vec<_>>();
        let report = workspace_sync::bundle(&root, &files, &destination_for_task)?;
        AppResult::Ok((current, delta, report))
    })
    .await
    .map_err(|error| AppError::Internal(format!("sync task: {error}")))??;
    Ok(Json(json!({
        "workspace_id": id,
        "bundle": report,
        "delta": delta,
        "manifest": current,
    })))
}

pub async fn apply(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ApplyBody>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace = workspaces::get(&state.db, &id).await?;
    let root = PathBuf::from(workspace.root_path);
    let data_dir = state.paths.data_dir.clone();
    let bundle = PathBuf::from(&body.bundle_path);
    let dry_run = body.dry_run;
    let report = tokio::task::spawn_blocking(move || {
        let bundle = workspace_sync::validate_bundle_path(&bundle, &data_dir)?;
        workspace_sync::apply(&bundle, &root, dry_run)
    })
    .await
    .map_err(|error| AppError::Internal(format!("sync task: {error}")))??;
    Ok(Json(json!({ "workspace_id": id, "report": report })))
}
