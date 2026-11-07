use crate::{
    app_state::AppState,
    chat,
    db::chat::{self as chat_db, ChatMessage, ChatThread, CreateThreadInput},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
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
    Ok(Json(chat_db::create_thread(&state.db, input).await?))
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
        let folder = std::path::Path::new(&workspace.root_path)
            .join("input")
            .join("chat")
            .join(chrono::Utc::now().format("%Y-%m").to_string());
        std::fs::create_dir_all(&folder)?;
        for attachment in body.attachments {
            let bytes = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                attachment
                    .base64
                    .split_once(',')
                    .map(|(_, payload)| payload)
                    .unwrap_or(&attachment.base64),
            )
            .map_err(|_| AppError::BadRequest("anexo inválido".into()))?;
            if bytes.len() > 20 * 1024 * 1024 {
                return Err(AppError::BadRequest("anexo maior que 20 MB".into()));
            }
            let name = attachment
                .filename
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                        character
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            let name = if name.trim_matches('_').is_empty() {
                "anexo".to_string()
            } else {
                name
            };
            let path = folder.join(format!(
                "{}-{name}",
                chrono::Utc::now().format("%Y%m%d-%H%M%S%3f")
            ));
            std::fs::write(&path, bytes)?;
            content.push_str(&format!(
                "\n\nArquivo anexado: {}",
                path.strip_prefix(&workspace.root_path)
                    .unwrap_or(&path)
                    .to_string_lossy()
            ));
        }
    }
    Ok(Json(chat::send_message(&state, thread, &content).await?))
}

pub async fn delete_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    chat_db::delete_thread(&state.db, &id).await?;
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
    Ok(Json(chat_db::get_thread(&state.db, &id).await?))
}

#[derive(Debug, Deserialize)]
pub struct UpdateThreadBody {
    pub title: Option<String>,
}
