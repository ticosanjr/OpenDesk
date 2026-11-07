use crate::{
    app_state::AppState,
    db::{
        artifacts::{self, ArtifactProvenance},
        workspaces,
    },
    error::{AppError, AppResult},
};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::header,
    response::Response,
};
use serde::Deserialize;
use std::path::Path as FsPath;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct ArtifactQuery {
    pub workspace_id: Option<String>,
    pub agent: Option<String>,
    pub task_id: Option<String>,
    pub action: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct ArtifactContent {
    pub path: String,
    pub content: String,
    pub truncated: bool,
    pub binary: bool,
    pub mime: String,
    pub size: u64,
}

pub async fn list_artifacts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ArtifactQuery>,
) -> AppResult<Json<Vec<ArtifactProvenance>>> {
    let workspace_id = match query.workspace_id {
        Some(id) => id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    Ok(Json(
        artifacts::list(
            &state.db,
            &workspace_id,
            query.agent.as_deref(),
            query.task_id.as_deref(),
            query.action.as_deref(),
            query.limit.unwrap_or(200),
        )
        .await?,
    ))
}

pub async fn get_artifact(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<ArtifactProvenance>> {
    Ok(Json(artifacts::get(&state.db, &id).await?))
}

pub async fn artifact_content(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<ArtifactContent>> {
    let artifact = artifacts::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &artifact.workspace_id).await?;
    let root = std::fs::canonicalize(&workspace.root_path)
        .map_err(|error| AppError::Internal(format!("workspace path: {error}")))?;
    let file = std::fs::canonicalize(&artifact.absolute_path)
        .map_err(|error| AppError::NotFound(format!("artifact file: {error}")))?;

    if !file.starts_with(&root) {
        return Err(AppError::BadRequest(
            "artifact path is outside the workspace".into(),
        ));
    }

    let bytes = std::fs::read(&file)
        .map_err(|error| AppError::Internal(format!("could not read artifact: {error}")))?;
    let truncated = bytes.len() > 2 * 1024 * 1024;
    let slice = if truncated {
        &bytes[..2 * 1024 * 1024]
    } else {
        &bytes
    };
    let binary =
        slice.iter().take(8_000).any(|byte| *byte == 0) || std::str::from_utf8(slice).is_err();
    let content = if binary {
        String::new()
    } else {
        String::from_utf8_lossy(slice).to_string()
    };

    Ok(Json(ArtifactContent {
        path: FsPath::new(&artifact.absolute_path)
            .to_string_lossy()
            .to_string(),
        content,
        truncated,
        binary,
        mime: crate::api::files::mime_for(&file).to_string(),
        size: bytes.len() as u64,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PinBody {
    pub pinned: bool,
}

pub async fn pin_artifact(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PinBody>,
) -> AppResult<Json<ArtifactProvenance>> {
    Ok(Json(
        artifacts::set_pinned(&state.db, &id, body.pinned).await?,
    ))
}

pub async fn artifact_raw(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let artifact = artifacts::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &artifact.workspace_id).await?;
    let root = std::fs::canonicalize(&workspace.root_path)
        .map_err(|error| AppError::Internal(format!("workspace path: {error}")))?;
    let file = std::fs::canonicalize(&artifact.absolute_path)
        .map_err(|error| AppError::NotFound(format!("artifact file: {error}")))?;
    if !file.starts_with(&root) {
        return Err(AppError::BadRequest(
            "artifact path is outside the workspace".into(),
        ));
    }
    let metadata = std::fs::metadata(&file)?;
    if metadata.len() > 250 * 1024 * 1024 {
        return Err(AppError::BadRequest(
            "artifact is too large for the in-app viewer".into(),
        ));
    }
    let bytes = std::fs::read(&file)?;
    let filename = file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "artifact".into());
    Response::builder()
        .header(header::CONTENT_TYPE, crate::api::files::mime_for(&file))
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{filename}\""),
        )
        .header(header::CONTENT_LENGTH, bytes.len())
        .body(Body::from(bytes))
        .map_err(|error| AppError::Internal(format!("could not build response: {error}")))
}
