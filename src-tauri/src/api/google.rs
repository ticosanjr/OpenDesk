use crate::{
    app_state::AppState,
    config_sync::{jsonc_editor, paths::StudioPaths},
    db::workspaces,
    error::{AppError, AppResult},
    integrations::google_oauth::{GOOGLE_SCOPES, GoogleStatus},
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Deserialize, Default)]
pub struct GoogleQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GoogleConfigInput {
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GoogleConfigDto {
    pub client_id_set: bool,
    pub client_secret_set: bool,
    pub scopes: &'static str,
    pub callback_url: String,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceBody {
    pub workspace_id: String,
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GoogleQuery>,
) -> AppResult<Json<GoogleStatus>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(state.google.status(&workspace_id).await?))
}

pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GoogleQuery>,
) -> AppResult<Json<GoogleConfigDto>> {
    let status = match crate::api::tasks::resolve_workspace_id(
        &state,
        query.workspace_id.as_deref(),
    )
    .await
    {
        Ok(workspace_id) => state.google.status(&workspace_id).await?,
        Err(_) => GoogleStatus {
            connected: false,
            email: None,
            scopes: None,
            callback_url: state.google.callback_url(),
            client_id_set: false,
            client_secret_set: false,
        },
    };
    Ok(Json(GoogleConfigDto {
        client_id_set: status.client_id_set,
        client_secret_set: status.client_secret_set,
        scopes: GOOGLE_SCOPES,
        callback_url: status.callback_url,
    }))
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(input): Json<GoogleConfigInput>,
) -> AppResult<Json<GoogleConfigDto>> {
    state
        .google
        .save_client_config(&input.client_id, input.client_secret.as_deref())
        .await?;
    get_config(State(state), Query(GoogleQuery { workspace_id: None })).await
}

pub async fn start_oauth(
    State(state): State<Arc<AppState>>,
    Json(body): Json<WorkspaceBody>,
) -> AppResult<Json<serde_json::Value>> {
    let auth_url = state.google.start_flow(&body.workspace_id).await?;
    Ok(Json(serde_json::json!({
        "auth_url": auth_url,
        "callback_url": state.google.callback_url(),
    })))
}

pub async fn disconnect(
    State(state): State<Arc<AppState>>,
    Json(body): Json<WorkspaceBody>,
) -> AppResult<Json<serde_json::Value>> {
    state.google.disconnect(&body.workspace_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn register_mcp(
    State(state): State<Arc<AppState>>,
    Json(body): Json<WorkspaceBody>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace = workspaces::get(&state.db, &body.workspace_id).await?;
    let studio = StudioPaths::new(Some(PathBuf::from(workspace.root_path)));
    let config_path = studio.config_path("workspace")?;
    let executable = std::env::current_exe().map_err(|error| {
        AppError::Internal(format!("could not resolve current executable: {error}"))
    })?;

    let server = serde_json::json!({
        "type": "local",
        "command": [executable.to_string_lossy(), "mcp-google"],
        "environment": {
            "OPENDESK_WORKSPACE_ID": body.workspace_id,
            "OPENDESK_DATA_DIR": state.paths.data_dir.to_string_lossy(),
            "OPENDESK_CONFIG_DIR": state.paths.config_dir.to_string_lossy(),
        },
        "disabled": false
    });

    jsonc_editor::upsert_mcp_server(
        &config_path,
        "opendesk-google",
        &server,
        None,
        &state.paths.backups_dir,
    )?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "config_path": config_path.to_string_lossy(),
        "server": "opendesk-google"
    })))
}
