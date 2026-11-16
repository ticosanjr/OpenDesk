use crate::{
    app_state::AppState,
    computer::{self, ComputerAction, ComputerCapabilities},
    db::workspaces,
    error::{AppError, AppResult},
    sandbox::{self, SandboxPolicy, SandboxStatus},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct WorkspaceQuery {
    pub workspace_id: Option<String>,
}

pub async fn get_sandbox(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> AppResult<Json<SandboxStatus>> {
    Ok(Json(
        sandbox::status(&state.db, query.workspace_id.as_deref()).await?,
    ))
}

pub async fn put_sandbox(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
    Json(policy): Json<SandboxPolicy>,
) -> AppResult<Json<SandboxPolicy>> {
    Ok(Json(
        sandbox::set_for(&state.db, query.workspace_id.as_deref(), policy).await?,
    ))
}

pub async fn computer_status() -> Json<ComputerCapabilities> {
    Json(computer::capabilities())
}

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> AppResult<Json<Vec<computer::ComputerSession>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(computer::list(&state.db, &workspace_id).await?))
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateSessionBody {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub require_approval: bool,
    #[serde(default)]
    pub isolated: bool,
}

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSessionBody>,
) -> AppResult<Json<computer::ComputerSession>> {
    let capabilities = computer::can_start(body.isolated);
    if !capabilities.available {
        return Err(AppError::Conflict(
            capabilities
                .reason
                .unwrap_or_else(|| "computer-use indisponível".into()),
        ));
    }
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    Ok(Json(
        computer::create(
            &state.db,
            &workspace_id,
            body.run_id.as_deref(),
            body.require_approval,
            body.isolated,
        )
        .await?,
    ))
}

pub async fn approve_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<computer::ComputerSession>> {
    Ok(Json(computer::approve(&state.db, &id).await?))
}

pub async fn end_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<computer::ComputerSession>> {
    Ok(Json(computer::end(&state.db, &id).await?))
}

pub async fn screenshot(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let session = computer::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &session.workspace_id).await?;
    let (path, bytes) = computer::screenshot(
        &state.db,
        &state.paths,
        std::path::Path::new(&workspace.root_path),
        &id,
    )
    .await?;
    Ok(Json(json!({
        "path": path,
        "mime": "image/png",
        "data": STANDARD.encode(bytes),
    })))
}

pub async fn action(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(action): Json<ComputerAction>,
) -> AppResult<Json<Value>> {
    Ok(Json(computer::act(&state.db, &id, action).await?))
}
