use crate::{app_state::AppState, error::AppResult};
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct McpQuery {
    pub workspace_id: Option<String>,
}

/// JSON-RPC over HTTP. This is the single-endpoint gateway slice: it reuses the
/// control-plane tools and dispatch path without the stdio transport. Streaming
/// HTTP/SSE and upstream MCP server pooling are later slices.
pub async fn rpc(
    State(state): State<Arc<AppState>>,
    Query(query): Query<McpQuery>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> AppResult<Json<Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let agent = headers
        .get("x-opendesk-agent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("build");
    let phase = headers
        .get("x-opendesk-mcp-phase")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("ALL");
    let role = headers
        .get("x-opendesk-mcp-role")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    Ok(Json(
        crate::mcp_control::handle_http(&workspace_id, agent, phase, role, request).await,
    ))
}
