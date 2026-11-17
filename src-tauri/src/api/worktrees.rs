use crate::{
    app_state::AppState,
    db::{self, worktrees},
    error::{AppError, AppResult},
    worktree,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Deserialize, Default)]
pub struct WorktreeQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PrepareBody {
    pub workspace_id: Option<String>,
    pub run_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MergeBody {
    #[serde(default)]
    pub approved: bool,
}

#[derive(Debug, Serialize)]
pub struct WorktreeEntry {
    #[serde(flatten)]
    pub worktree: worktree::RunWorktree,
    pub task_title: Option<String>,
    pub run_status: Option<String>,
    pub diff_stat: worktree::DiffStat,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorktreeQuery>,
) -> AppResult<Json<Vec<WorktreeEntry>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = db::workspaces::get(&state.db, &workspace_id).await?;
    let root = PathBuf::from(workspace.root_path);
    let mut entries = Vec::new();
    for row in worktrees::list_with_context(&state.db, &workspace_id).await? {
        let stat = if row.status == "ACTIVE" {
            let worktree = row.clone().into_model();
            let root = root.clone();
            tokio::task::spawn_blocking(move || worktree::diff_stat(&worktree, &root))
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default()
        } else {
            worktree::DiffStat::default()
        };
        entries.push(WorktreeEntry {
            worktree: row.clone().into_model(),
            task_title: row.task_title,
            run_status: row.run_status,
            diff_stat: stat,
        });
    }
    Ok(Json(entries))
}

pub async fn prepare(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PrepareBody>,
) -> AppResult<Json<worktree::RunWorktree>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    let workspace = db::workspaces::get(&state.db, &workspace_id).await?;
    let root = PathBuf::from(&workspace.root_path);
    let data = state.paths.data_dir.clone();
    let run_id = body.run_id.clone();
    let prepared = tokio::task::spawn_blocking(move || {
        worktree::prepare(&workspace_id, &root, &data, run_id.as_deref())
    })
    .await
    .map_err(|error| AppError::Internal(format!("worktree task: {error}")))??;
    worktrees::insert(&state.db, &prepared).await?;
    Ok(Json(prepared))
}

pub async fn diff(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let worktree = worktrees::get(&state.db, &id).await?;
    let workspace = db::workspaces::get(&state.db, &worktree.workspace_id).await?;
    let root = PathBuf::from(workspace.root_path);
    let diff = tokio::task::spawn_blocking(move || worktree::diff(&worktree, &root))
        .await
        .map_err(|error| AppError::Internal(format!("worktree task: {error}")))??;
    Ok(Json(json!({ "id": id, "diff": diff })))
}

pub async fn merge(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<MergeBody>,
) -> AppResult<Json<serde_json::Value>> {
    let worktree = worktrees::get(&state.db, &id).await?;
    let workspace = db::workspaces::get(&state.db, &worktree.workspace_id).await?;
    let root = PathBuf::from(workspace.root_path);
    let approved = body.approved;
    let report = tokio::task::spawn_blocking(move || worktree::merge(&worktree, &root, approved))
        .await
        .map_err(|error| AppError::Internal(format!("worktree task: {error}")))??;
    worktrees::set_status(
        &state.db,
        &report.id,
        if report.status == "MERGED" {
            "MERGED"
        } else {
            "ACTIVE"
        },
    )
    .await?;
    Ok(Json(json!(report)))
}

pub async fn cleanup(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let worktree = worktrees::get(&state.db, &id).await?;
    let workspace = db::workspaces::get(&state.db, &worktree.workspace_id).await?;
    let root = PathBuf::from(workspace.root_path);
    tokio::task::spawn_blocking(move || worktree::cleanup(&worktree, &root))
        .await
        .map_err(|error| AppError::Internal(format!("worktree task: {error}")))??;
    worktrees::set_status(&state.db, &id, "REMOVED").await?;
    Ok(Json(json!({ "id": id, "status": "REMOVED" })))
}
