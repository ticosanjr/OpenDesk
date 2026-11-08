use crate::{
    app_state::AppState,
    db::io_bus::{self, InboundInput, OutboundInput},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct WorkspaceQuery {
    pub workspace_id: Option<String>,
}

pub async fn list_inbound(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "sources": io_bus::list_inbound(&state.db, &workspace_id).await?,
    })))
}

pub async fn create_inbound(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
    Json(input): Json<InboundInput>,
) -> AppResult<Json<io_bus::InboundSource>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let source = io_bus::create_inbound(&state.db, &workspace_id, input).await?;
    state.io_bus.reload(state.clone()).await?;
    Ok(Json(source))
}

pub async fn update_inbound(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<InboundInput>,
) -> AppResult<Json<io_bus::InboundSource>> {
    let source = io_bus::update_inbound(&state.db, &id, input).await?;
    state.io_bus.reload(state.clone()).await?;
    Ok(Json(source))
}

pub async fn delete_inbound(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    io_bus::delete_inbound(&state.db, &id).await?;
    state.io_bus.reload(state.clone()).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn list_outbound(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "sinks": io_bus::list_outbound(&state.db, &workspace_id).await?,
    })))
}

pub async fn create_outbound(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
    Json(input): Json<OutboundInput>,
) -> AppResult<Json<io_bus::OutboundSink>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        io_bus::create_outbound(&state.db, &workspace_id, input).await?,
    ))
}

pub async fn update_outbound(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<OutboundInput>,
) -> AppResult<Json<io_bus::OutboundSink>> {
    Ok(Json(io_bus::update_outbound(&state.db, &id, input).await?))
}

pub async fn delete_outbound(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    io_bus::delete_outbound(&state.db, &id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct BindingInput {
    pub sink_ids: Vec<String>,
}

pub async fn task_sinks(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "bindings": io_bus::bindings_for_task(&state.db, &id).await?,
    })))
}

pub async fn set_task_sinks(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<BindingInput>,
) -> AppResult<Json<serde_json::Value>> {
    io_bus::set_task_bindings(&state.db, &id, &input.sink_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn schedule_sinks(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "bindings": io_bus::bindings_for_schedule(&state.db, &id).await?,
    })))
}

pub async fn set_schedule_sinks(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<BindingInput>,
) -> AppResult<Json<serde_json::Value>> {
    io_bus::set_schedule_bindings(&state.db, &id, &input.sink_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct SecretInput {
    pub key: String,
    pub value: String,
}

pub async fn put_secret(
    State(state): State<Arc<AppState>>,
    Json(input): Json<SecretInput>,
) -> AppResult<Json<serde_json::Value>> {
    let key = input.key.trim();
    if key.is_empty() || key.len() > 120 {
        return Err(crate::error::AppError::BadRequest("chave inválida".into()));
    }
    crate::secrets::SecretStore::new(&state.paths).set("io_bus", key, input.value.trim())?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_secret(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    crate::secrets::SecretStore::new(&state.paths).delete("io_bus", &key)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize, Default)]
pub struct ToolQuery {
    #[serde(default)]
    pub refresh: bool,
}

pub async fn tools(Query(query): Query<ToolQuery>) -> Json<Vec<crate::io_bus::tools::ToolInfo>> {
    let result = tokio::task::spawn_blocking(move || {
        if query.refresh {
            crate::io_bus::tools::list_fresh()
        } else {
            crate::io_bus::tools::list()
        }
    })
    .await;
    Json(result.unwrap_or_default())
}

pub async fn install_tool(
    Path(name): Path<String>,
) -> AppResult<Json<crate::io_bus::tools::ToolInfo>> {
    Ok(Json(crate::io_bus::tools::install(&name).await?))
}

#[derive(Debug, Deserialize)]
pub struct ToolPathBody {
    pub path: Option<String>,
}

pub async fn set_tool_path(
    Path(name): Path<String>,
    Json(body): Json<ToolPathBody>,
) -> AppResult<Json<crate::io_bus::tools::ToolInfo>> {
    crate::io_bus::tools::set_manual_path(&name, body.path.as_deref())?;
    Ok(Json(
        crate::io_bus::tools::list()
            .into_iter()
            .find(|tool| tool.name == name)
            .ok_or_else(|| AppError::NotFound(format!("ferramenta {name}")))?,
    ))
}

pub async fn webhook(
    State(state): State<Arc<AppState>>,
    Path(source_id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> AppResult<Json<serde_json::Value>> {
    if body.len() > 2 * 1024 * 1024 {
        return Err(crate::error::AppError::BadRequest(
            "payload maior que 2 MB".into(),
        ));
    }
    let source = io_bus::get_inbound(&state.db, &source_id).await?;
    Ok(Json(
        crate::io_bus::inbound::webhook::handle(&state, source, &headers, &body).await?,
    ))
}
