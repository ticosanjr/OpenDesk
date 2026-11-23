use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Serialize;
use std::sync::Arc;

use super::paths::{FileQuery, relative_to, resolve_path, workspace_root};

const MAX_PREVIEW_BYTES: usize = 512 * 1024;

#[derive(Debug, Serialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub truncated: bool,
    pub binary: bool,
    pub mime: String,
    pub size: u64,
}

pub async fn content(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> AppResult<Json<FileContent>> {
    let root = workspace_root(&state, query.workspace_id.as_deref()).await?;
    let path = resolve_path(&root, query.path.as_deref())?;
    let metadata = std::fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(AppError::BadRequest("path is not a file".into()));
    }
    let bytes = std::fs::read(&path)?;
    let truncated = bytes.len() > MAX_PREVIEW_BYTES;
    let slice = &bytes[..bytes.len().min(MAX_PREVIEW_BYTES)];
    let binary =
        slice.iter().take(8_000).any(|byte| *byte == 0) || std::str::from_utf8(slice).is_err();
    let content = if binary {
        String::new()
    } else {
        String::from_utf8_lossy(slice).to_string()
    };
    Ok(Json(FileContent {
        path: relative_to(&root, &path),
        content,
        truncated,
        binary,
        mime: crate::media::mime_for_path(&path).to_string(),
        size: metadata.len(),
    }))
}
