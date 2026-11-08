use crate::{
    app_state::AppState,
    db::workspaces,
    error::{AppError, AppResult},
    media::mime_for_path,
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

pub const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;
/// Base64 inflates the payload by ~33%; leave room for JSON framing too.
pub const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

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

pub async fn save(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AttachmentQuery>,
    Json(body): Json<SaveAttachment>,
) -> AppResult<Json<SavedAttachment>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    Ok(Json(save_base64(
        Path::new(&workspace.root_path),
        "input/attachments",
        &body.filename,
        &body.base64,
    )?))
}

pub fn save_base64(
    root: &Path,
    folder: &str,
    filename: &str,
    base64: &str,
) -> AppResult<SavedAttachment> {
    save_base64_labeled(root, folder, filename, base64, "anexo", "inválido")
}

pub fn save_image_base64(
    root: &Path,
    folder: &str,
    filename: &str,
    base64: &str,
) -> AppResult<SavedAttachment> {
    save_base64_labeled(root, folder, filename, base64, "imagem", "inválida")
}

fn save_base64_labeled(
    root: &Path,
    folder: &str,
    filename: &str,
    base64: &str,
    kind: &str,
    invalid_adjective: &str,
) -> AppResult<SavedAttachment> {
    let bytes = STANDARD
        .decode(
            base64
                .split_once(',')
                .map(|(_, payload)| payload)
                .unwrap_or(base64),
        )
        .map_err(|_| AppError::BadRequest(format!("{kind} {invalid_adjective}")))?;
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(AppError::BadRequest(format!(
            "{kind} maior que {} MB",
            MAX_UPLOAD_BYTES / (1024 * 1024)
        )));
    }
    save_bytes(root, folder, filename, &bytes)
}

fn save_bytes(
    root: &Path,
    folder: &str,
    filename: &str,
    bytes: &[u8],
) -> AppResult<SavedAttachment> {
    let name = safe_name(filename);
    let month = chrono::Utc::now().format("%Y-%m").to_string();
    let directory = root.join(folder).join(&month);
    std::fs::create_dir_all(&directory)?;
    let stored = format!(
        "{}-{}",
        Uuid::now_v7()
            .simple()
            .to_string()
            .get(..8)
            .unwrap_or("anexo"),
        name
    );
    std::fs::write(directory.join(&stored), bytes)?;
    Ok(SavedAttachment {
        name,
        path: format!("{folder}/{month}/{stored}"),
        mime: mime_for_path(Path::new(&stored)).to_string(),
        size: bytes.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_uploads_keep_image_specific_errors() {
        let error = save_image_base64(
            &std::env::temp_dir(),
            "input/captures",
            "capture.png",
            "not-base64",
        )
        .expect_err("invalid image");
        assert!(error.to_string().contains("imagem inválida"));
    }
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
