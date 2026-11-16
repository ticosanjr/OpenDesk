use crate::{
    app_state::AppState,
    db::memory::{self, CreateNoteInput, MemoryAcl, MemoryNamespace, MemoryNote, UpdateNoteInput},
    error::AppResult,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct MemoryQuery {
    pub workspace_id: Option<String>,
    pub namespace: Option<String>,
    pub agent: Option<String>,
    pub limit: Option<i64>,
    pub q: Option<String>,
}

async fn workspace(state: &AppState, value: Option<&str>) -> AppResult<String> {
    crate::api::tasks::resolve_workspace_id(state, value).await
}

pub async fn list_namespaces(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MemoryQuery>,
) -> AppResult<Json<Vec<MemoryNamespace>>> {
    let workspace_id = workspace(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        memory::list_namespaces(&state.db, &workspace_id).await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct NamespaceInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

pub async fn create_namespace(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MemoryQuery>,
    Json(input): Json<NamespaceInput>,
) -> AppResult<Json<MemoryNamespace>> {
    let workspace_id = workspace(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        memory::ensure_namespace(
            &state.db,
            &workspace_id,
            &input.name,
            input.description.as_deref(),
        )
        .await?,
    ))
}

pub async fn list_acl(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<MemoryAcl>>> {
    Ok(Json(memory::list_acl(&state.db, &id).await?))
}

#[derive(Debug, Deserialize)]
pub struct AclInput {
    pub agent: String,
    #[serde(default = "default_true")]
    pub can_read: bool,
    #[serde(default)]
    pub can_write: bool,
}

fn default_true() -> bool {
    true
}

pub async fn put_acl(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<AclInput>,
) -> AppResult<Json<Value>> {
    memory::set_acl(
        &state.db,
        &id,
        &input.agent,
        input.can_read,
        input.can_write,
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_acl(
    State(state): State<Arc<AppState>>,
    Path((id, agent)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    memory::delete_acl(&state.db, &id, &agent).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_notes(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MemoryQuery>,
) -> AppResult<Json<Vec<MemoryNote>>> {
    let workspace_id = workspace(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        memory::list_notes(
            &state.db,
            &workspace_id,
            query.namespace.as_deref(),
            query.agent.as_deref(),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub async fn create_note(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MemoryQuery>,
    Json(input): Json<CreateNoteInput>,
) -> AppResult<Json<MemoryNote>> {
    let workspace_id = workspace(&state, query.workspace_id.as_deref()).await?;
    if let Some(agent) = query.agent.as_deref()
        && !memory::can_write(&state.db, &workspace_id, &input.namespace, agent).await?
    {
        return Err(crate::error::AppError::Conflict(format!(
            "agente {agent} não pode escrever no namespace {}",
            input.namespace
        )));
    }
    Ok(Json(
        memory::create_note(&state.db, &workspace_id, input).await?,
    ))
}

pub async fn update_note(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateNoteInput>,
) -> AppResult<Json<MemoryNote>> {
    Ok(Json(memory::update_note(&state.db, &id, input).await?))
}

pub async fn delete_note(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    memory::delete_note(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MemoryQuery>,
) -> AppResult<Json<Vec<MemoryNote>>> {
    let workspace_id = workspace(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        memory::search(
            &state.db,
            &workspace_id,
            query.q.as_deref().unwrap_or(""),
            query.agent.as_deref(),
            query.limit.unwrap_or(20),
        )
        .await?,
    ))
}

pub async fn context(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MemoryQuery>,
) -> AppResult<Json<Value>> {
    let workspace_id = workspace(&state, query.workspace_id.as_deref()).await?;
    let rendered = memory::render_context(
        &state.db,
        &workspace_id,
        query.agent.as_deref().unwrap_or("build"),
        query.q.as_deref().unwrap_or(""),
        query.limit.unwrap_or(8),
    )
    .await?;
    Ok(Json(json!({ "context": rendered })))
}
