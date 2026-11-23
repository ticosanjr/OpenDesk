use crate::{
    app_state::AppState,
    db::chat::{self as chat_db, ChatThread, CreateThreadInput},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize)]
pub struct ThreadDetail {
    pub thread: ChatThread,
    pub messages: Vec<crate::db::chat::ChatMessage>,
}

pub async fn get_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<ThreadDetail>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    let messages = chat_db::list_messages(&state.db, &id).await?;
    Ok(Json(ThreadDetail { thread, messages }))
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
