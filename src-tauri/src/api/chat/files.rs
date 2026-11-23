use crate::{
    app_state::AppState,
    db::{chat as chat_db, workspaces},
    error::{AppError, AppResult},
    integrations::{signal, telegram},
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct SendFileBody {
    pub path: String,
    #[serde(default)]
    pub caption: Option<String>,
}

/// Sends a workspace file through the channel that owns the thread. Chat file
/// delivery is path-based on purpose: shell-created files are not always
/// registered as artifacts, but they still exist in the workspace.
pub async fn send_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SendFileBody>,
) -> AppResult<Json<serde_json::Value>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &thread.workspace_id).await?;
    let root = std::path::Path::new(&workspace.root_path);
    let file = resolve_workspace_file(root, &body.path)?;
    let relative = file
        .strip_prefix(root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| file.to_string_lossy().to_string());
    let caption = body
        .caption
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let external = thread
        .external_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match thread.source.as_str() {
        "telegram" => {
            let chat_id = external.ok_or_else(|| {
                AppError::BadRequest("thread do Telegram sem chat_id externo".into())
            })?;
            let token = telegram::token_for(&state).await?;
            telegram::send_document(&token, chat_id, &file, caption).await?;
        }
        "signal" => {
            let recipient = external.ok_or_else(|| {
                AppError::BadRequest("thread do Signal sem destinatário externo".into())
            })?;
            let config = signal::load(&state.db).await?;
            if !config.enabled || config.account.trim().is_empty() {
                return Err(AppError::BadRequest("Signal não está habilitado".into()));
            }
            let attachment = file.to_string_lossy().to_string();
            signal::send_with_attachments(
                &config.cli_path,
                &config.account,
                recipient,
                caption.unwrap_or("Arquivo enviado pelo OpenDesk"),
                &[attachment],
            )
            .await?;
        }
        _ => {
            return Ok(Json(serde_json::json!({
                "sent": false,
                "path": relative,
                "reason": "canal sem envio de arquivo; cite o caminho na resposta",
            })));
        }
    }
    Ok(Json(serde_json::json!({ "sent": true, "path": relative })))
}

/// Resolves a user/agent supplied path inside the workspace. Symlinks are
/// followed and rejected when they escape the workspace root.
pub(crate) fn resolve_workspace_file(
    root: &std::path::Path,
    requested: &str,
) -> AppResult<PathBuf> {
    let requested = requested.trim();
    if requested.is_empty() {
        return Err(AppError::BadRequest(
            "caminho do arquivo é obrigatório".into(),
        ));
    }
    let raw = std::path::Path::new(requested);
    let absolute = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        root.join(raw)
    };
    let canonical_root = root
        .canonicalize()
        .map_err(|error| AppError::Internal(format!("workspace inacessível: {error}")))?;
    let canonical = absolute.canonicalize().map_err(|_| {
        AppError::NotFound(format!("arquivo {requested} não encontrado no workspace"))
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(AppError::BadRequest(
            "o arquivo está fora do workspace".into(),
        ));
    }
    if !canonical.is_file() {
        return Err(AppError::BadRequest(
            "o caminho não aponta para um arquivo".into(),
        ));
    }
    let size = std::fs::metadata(&canonical)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if size > 50 * 1024 * 1024 {
        return Err(AppError::BadRequest(
            "arquivo excede o limite de 50 MB do Telegram".into(),
        ));
    }
    Ok(canonical)
}
