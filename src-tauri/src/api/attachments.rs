use crate::{
    app_state::AppState,
    db::workspaces,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Query, State},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{
    path::{Component, Path},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct AttachmentQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveAttachment {
    pub filename: String,
    pub base64: String,
}

#[derive(Debug, Serialize)]
pub struct SavedAttachment {
    pub name: String,
    pub path: String,
    pub mime: String,
    pub size: usize,
}

fn safe_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let name = name.trim_matches('_');
    if name.is_empty() {
        "anexo".into()
    } else {
        name.chars().take(120).collect()
    }
}

fn mime_for(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_lowercase).as_deref() {
        Some("md") | Some("markdown") => "text/markdown",
        Some("json") => "application/json",
        Some("html") | Some("htm") => "text/html",
        Some("txt") | Some("log") => "text/plain",
        Some("csv") => "text/csv",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

pub async fn save(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AttachmentQuery>,
    Json(body): Json<SaveAttachment>,
) -> AppResult<Json<SavedAttachment>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let bytes = STANDARD
        .decode(
            body.base64
                .split_once(',')
                .map(|(_, payload)| payload)
                .unwrap_or(&body.base64),
        )
        .map_err(|_| AppError::BadRequest("anexo inválido".into()))?;
    if bytes.len() > 20 * 1024 * 1024 {
        return Err(AppError::BadRequest("anexo maior que 20 MB".into()));
    }
    let name = safe_name(&body.filename);
    let folder = Path::new(&workspace.root_path)
        .join("input")
        .join("attachments")
        .join(chrono::Utc::now().format("%Y-%m").to_string());
    std::fs::create_dir_all(&folder)?;
    let stored = format!(
        "{}-{}",
        Uuid::now_v7()
            .simple()
            .to_string()
            .get(..8)
            .unwrap_or("anexo"),
        name
    );
    let path = folder.join(&stored);
    std::fs::write(&path, &bytes)?;
    Ok(Json(SavedAttachment {
        name,
        path: format!(
            "input/attachments/{}/{}",
            chrono::Utc::now().format("%Y-%m"),
            stored
        ),
        mime: mime_for(&stored).to_string(),
        size: bytes.len(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct DeleteAttachment {
    pub workspace_id: Option<String>,
    pub path: String,
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DeleteAttachment>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let root = Path::new(&workspace.root_path);
    let relative = Path::new(&query.path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
        || !relative.starts_with("input/attachments")
    {
        return Err(AppError::BadRequest("caminho de anexo inválido".into()));
    }
    let file = root.join(relative);
    if !file.is_file() {
        return Err(AppError::NotFound("anexo não encontrado".into()));
    }
    std::fs::remove_file(file)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
