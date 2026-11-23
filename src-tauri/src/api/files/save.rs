use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use super::paths::{resolve_path, workspace_root};

#[derive(Debug, Deserialize)]
pub struct SaveFileBody {
    pub workspace_id: Option<String>,
    pub path: String,
    pub destination: String,
}

/// Copy a file to a user-chosen absolute destination (native save dialog).
pub fn save_to_destination(source: &Path, destination: &str) -> AppResult<()> {
    let destination = PathBuf::from(destination.trim());
    if destination.as_os_str().is_empty() || !destination.is_absolute() {
        return Err(AppError::BadRequest(
            "o destino do download deve ser um caminho absoluto".into(),
        ));
    }
    if let Some(parent) = destination.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            AppError::Internal(format!(
                "não foi possível criar a pasta de destino: {error}"
            ))
        })?;
    }
    if std::fs::canonicalize(source).ok() == std::fs::canonicalize(&destination).ok() {
        return Err(AppError::BadRequest(
            "origem e destino do download são o mesmo arquivo".into(),
        ));
    }
    std::fs::copy(source, &destination).map_err(|error| {
        AppError::Internal(format!("não foi possível salvar o download: {error}"))
    })?;
    Ok(())
}

pub async fn save_file(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SaveFileBody>,
) -> AppResult<Json<serde_json::Value>> {
    let root = workspace_root(&state, body.workspace_id.as_deref()).await?;
    let source = resolve_path(&root, Some(body.path.as_str()))?;
    if !source.is_file() {
        return Err(AppError::BadRequest("path is not a file".into()));
    }
    save_to_destination(&source, &body.destination)?;
    Ok(Json(json!({ "path": body.destination })))
}
