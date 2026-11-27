use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{Json, extract::State};
use base64::{Engine as _, engine::general_purpose::STANDARD};
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

fn destination_allowed(destination: &Path) -> bool {
    if !destination.is_absolute() {
        return false;
    }
    let candidate = if destination.exists() {
        std::fs::canonicalize(destination).unwrap_or_else(|_| destination.to_path_buf())
    } else {
        destination
            .parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .map(|parent| parent.join(destination.file_name().unwrap_or_default()))
            .unwrap_or_else(|| destination.to_path_buf())
    };
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let temp = std::env::temp_dir();
    home.as_ref()
        .is_some_and(|home| candidate.starts_with(home))
        || candidate.starts_with(temp)
}

#[derive(Debug, Deserialize)]
pub struct SaveBytesBody {
    pub destination: String,
    pub content_base64: String,
}

pub async fn save_bytes(Json(body): Json<SaveBytesBody>) -> AppResult<Json<serde_json::Value>> {
    let destination = PathBuf::from(body.destination.trim());
    if body.content_base64.len() > 180 * 1024 * 1024 {
        return Err(AppError::BadRequest(
            "conteúdo exportado excede o limite de 128 MB".into(),
        ));
    }
    if !destination_allowed(&destination) {
        return Err(AppError::BadRequest(
            "destino fora de $HOME ou da pasta temporária".into(),
        ));
    }
    let bytes = STANDARD
        .decode(body.content_base64.trim())
        .map_err(|_| AppError::BadRequest("conteúdo base64 inválido".into()))?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(AppError::BadRequest(
            "conteúdo exportado excede o limite de 128 MB".into(),
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
    let temp = destination.with_extension(format!(
        "{}.tmp",
        destination
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("tmp")
    ));
    std::fs::write(&temp, &bytes)
        .map_err(|error| AppError::Internal(format!("não foi possível gravar: {error}")))?;
    std::fs::rename(&temp, &destination)
        .map_err(|error| AppError::Internal(format!("não foi possível concluir: {error}")))?;
    Ok(Json(json!({ "path": body.destination })))
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
