use crate::{
    app_state::AppState,
    chat,
    db::{
        chat::{self as chat_db, ChatMessage, ChatThread, CreateThreadInput},
        workspaces,
    },
    error::{AppError, AppResult},
    integrations::{signal, telegram},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct ThreadQuery {
    pub workspace_id: Option<String>,
    pub source: Option<String>,
}

pub async fn list_threads(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ThreadQuery>,
) -> AppResult<Json<Vec<ChatThread>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        chat_db::list_threads(&state.db, &workspace_id, query.source.as_deref()).await?,
    ))
}

pub async fn create_thread(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateThreadInput>,
) -> AppResult<Json<ChatThread>> {
    if input.agent.trim().is_empty() {
        return Err(AppError::BadRequest("agent is required".into()));
    }
    if let Some(model) = input
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && crate::opencode_client::rest_client::ModelRefDto::parse(model).is_none()
    {
        return Err(AppError::BadRequest(format!(
            "modelo inválido '{model}'; use provider/modelo ou provider/modelo#variant"
        )));
    }
    let thread = chat_db::create_thread(&state.db, input).await?;
    state
        .events
        .publish(crate::events::OpenDeskWsEvent::ChatUpdated {
            thread_id: thread.id.clone(),
            workspace_id: thread.workspace_id.clone(),
        });
    Ok(Json(thread))
}

#[derive(Debug, serde::Serialize)]
pub struct ThreadDetail {
    pub thread: ChatThread,
    pub messages: Vec<ChatMessage>,
}

pub async fn get_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<ThreadDetail>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    let messages = chat_db::list_messages(&state.db, &id).await?;
    Ok(Json(ThreadDetail { thread, messages }))
}

#[derive(Debug, Deserialize)]
pub struct AttachmentBody {
    pub filename: String,
    pub base64: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageBody {
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<AttachmentBody>,
}

pub async fn send_message(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<MessageBody>,
) -> AppResult<Json<chat::ChatReply>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    let mut content = body.content;
    if !body.attachments.is_empty() {
        let workspace = crate::db::workspaces::get(&state.db, &thread.workspace_id).await?;
        for attachment in body.attachments {
            let saved = crate::api::attachments::save_base64(
                std::path::Path::new(&workspace.root_path),
                "input/chat",
                &attachment.filename,
                &attachment.base64,
            )?;
            content.push_str(&format!("\n\nArquivo anexado: {}", saved.path));
        }
    }
    let context = chat::channel_context("Desktop", &thread.workspace_id, &thread.id);
    // Detach the turn so a browser reload does not abort the OpenCode work. The
    // handler still waits for the reply in the normal case, while the task keeps
    // running (and persisting messages) if the request is dropped.
    let handle = tokio::spawn(async move {
        chat::send_message_with_context(&state, thread, &content, Some(&context)).await
    });
    match handle.await {
        Ok(result) => Ok(Json(result?)),
        Err(error) => Err(AppError::Internal(format!(
            "chat task terminated unexpectedly: {error}"
        ))),
    }
}

pub async fn latest_turn(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Option<crate::db::chat_turns::ChatTurn>>> {
    let _ = chat_db::get_thread(&state.db, &id).await?;
    Ok(Json(
        crate::db::chat_turns::latest_for_thread(&state.db, &id).await?,
    ))
}

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

/// Stops the active chat turn. The wait loop sees the cancellation token and
/// marks the chat turn as CANCELLED; OpenCode is interrupted as well.
pub async fn interrupt_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    let (turn_id, session_id) = match state.cancel_chat_turn(&thread.id) {
        Some(value) => value,
        None => {
            let Some(active) = crate::db::chat_turns::active_for_thread(&state.db, &id).await?
            else {
                return Err(AppError::Conflict(
                    "nenhuma execução em andamento nesta conversa".into(),
                ));
            };
            let _ = crate::db::chat_turns::finish(
                &state.db,
                &active.id,
                "CANCELLED",
                active.assistant_content.as_deref(),
                &crate::db::runs::RunUsage::default(),
                active.duration_ms,
                Some("cancelled"),
                Some("interrompido pelo usuário"),
            )
            .await;
            (active.id, thread.opencode_session_id.clone())
        }
    };
    let session = session_id.or(thread.opencode_session_id);
    if let Some(session) = session
        && let Some(client) = state.daemon.client().await
    {
        let _ = client.interrupt(&session).await;
    }
    Ok(Json(serde_json::json!({ "ok": true, "turn_id": turn_id })))
}

pub async fn delete_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    chat_db::delete_thread(&state.db, &id).await?;
    state
        .events
        .publish(crate::events::OpenDeskWsEvent::ChatUpdated {
            thread_id: thread.id,
            workspace_id: thread.workspace_id,
        });
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn update_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateThreadBody>,
) -> AppResult<Json<ChatThread>> {
    if let Some(title) = body.title.filter(|value| !value.trim().is_empty()) {
        chat_db::update_title(&state.db, &id, title.trim()).await?;
    }
    let thread = chat_db::get_thread(&state.db, &id).await?;
    state
        .events
        .publish(crate::events::OpenDeskWsEvent::ChatUpdated {
            thread_id: thread.id.clone(),
            workspace_id: thread.workspace_id.clone(),
        });
    Ok(Json(thread))
}

#[derive(Debug, Deserialize)]
pub struct UpdateThreadBody {
    pub title: Option<String>,
}
