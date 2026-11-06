use crate::{
    app_state::AppState,
    config_sync::{
        agents::{self, AgentFile, AgentWriteInput},
        commands::{self, CommandFile, CommandWriteInput},
        jsonc_editor::{self, ConfigDocument},
        paths::StudioPaths,
        skills::{self, SkillFile, SkillWriteInput},
    },
    db::{settings, workspaces},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct StudioQuery {
    pub scope: Option<String>,
    pub workspace_id: Option<String>,
    pub id: Option<String>,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigWriteBody {
    pub content: String,
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct McpWriteBody {
    pub server: Value,
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

pub(crate) async fn context(
    state: &AppState,
    query: &StudioQuery,
) -> AppResult<(StudioPaths, String, String)> {
    let requested = query.scope.as_deref().unwrap_or("workspace");
    match requested {
        "global" => {
            let workspace_id = match &query.workspace_id {
                Some(id) => id.clone(),
                None => settings::active_workspace_id(&state.db)
                    .await?
                    .unwrap_or_else(|| "global".to_string()),
            };
            Ok((StudioPaths::new(None), "global".to_string(), workspace_id))
        }
        "workspace" => {
            let workspace_id = match &query.workspace_id {
                Some(id) => id.clone(),
                None => settings::active_workspace_id(&state.db)
                    .await?
                    .ok_or_else(|| {
                        AppError::BadRequest(
                            "no active workspace; pass workspace_id or use scope=global".into(),
                        )
                    })?,
            };
            let workspace = workspaces::get(&state.db, &workspace_id).await?;
            Ok((
                StudioPaths::new(Some(PathBuf::from(workspace.root_path))),
                "workspace".to_string(),
                workspace_id,
            ))
        }
        other => Err(AppError::BadRequest(format!(
            "invalid scope '{other}'; expected global or workspace"
        ))),
    }
}

pub(crate) fn require_id(query: &StudioQuery) -> AppResult<String> {
    query
        .id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("the id query parameter is required".into()))
}

// Agents -------------------------------------------------------------------

pub async fn list_agents(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Vec<AgentFile>>> {
    let (paths, scope, _) = context(&state, &query).await?;
    paths.ensure_scope_layout(&scope)?;
    Ok(Json(agents::list(&paths, &scope)?))
}

pub async fn get_agent(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<AgentFile>> {
    let (paths, scope, _) = context(&state, &query).await?;
    Ok(Json(agents::get(&paths, &scope, &require_id(&query)?)?))
}

pub async fn put_agent(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
    Json(input): Json<AgentWriteInput>,
) -> AppResult<Json<AgentFile>> {
    let (paths, scope, _) = context(&state, &query).await?;
    paths.ensure_scope_layout(&scope)?;
    Ok(Json(agents::write(
        &paths,
        &scope,
        &require_id(&query)?,
        input,
        &state.paths.backups_dir,
    )?))
}

pub async fn delete_agent(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<StatusCode> {
    let (paths, scope, _) = context(&state, &query).await?;
    agents::delete(&paths, &scope, &require_id(&query)?)?;
    Ok(StatusCode::NO_CONTENT)
}

// Skills -------------------------------------------------------------------

pub async fn list_skills(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Vec<SkillFile>>> {
    let (paths, scope, _) = context(&state, &query).await?;
    paths.ensure_scope_layout(&scope)?;
    Ok(Json(skills::list(&paths, &scope)?))
}

pub async fn get_skill(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<SkillFile>> {
    let (paths, scope, _) = context(&state, &query).await?;
    Ok(Json(skills::get(&paths, &scope, &require_id(&query)?)?))
}

pub async fn put_skill(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
    Json(input): Json<SkillWriteInput>,
) -> AppResult<Json<SkillFile>> {
    let (paths, scope, _) = context(&state, &query).await?;
    paths.ensure_scope_layout(&scope)?;
    Ok(Json(skills::write(
        &paths,
        &scope,
        &require_id(&query)?,
        input,
        &state.paths.backups_dir,
    )?))
}

pub async fn delete_skill(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<StatusCode> {
    let (paths, scope, _) = context(&state, &query).await?;
    skills::delete(&paths, &scope, &require_id(&query)?)?;
    Ok(StatusCode::NO_CONTENT)
}

// Commands -----------------------------------------------------------------

pub async fn list_commands(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Vec<CommandFile>>> {
    let (paths, scope, _) = context(&state, &query).await?;
    paths.ensure_scope_layout(&scope)?;
    Ok(Json(commands::list(&paths, &scope)?))
}

pub async fn get_command(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<CommandFile>> {
    let (paths, scope, _) = context(&state, &query).await?;
    Ok(Json(commands::get(&paths, &scope, &require_id(&query)?)?))
}

pub async fn put_command(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
    Json(input): Json<CommandWriteInput>,
) -> AppResult<Json<CommandFile>> {
    let (paths, scope, _) = context(&state, &query).await?;
    paths.ensure_scope_layout(&scope)?;
    Ok(Json(commands::write(
        &paths,
        &scope,
        &require_id(&query)?,
        input,
        &state.paths.backups_dir,
    )?))
}

pub async fn delete_command(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<StatusCode> {
    let (paths, scope, _) = context(&state, &query).await?;
    commands::delete(&paths, &scope, &require_id(&query)?)?;
    Ok(StatusCode::NO_CONTENT)
}

// JSONC config -------------------------------------------------------------

pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<ConfigDocument>> {
    let (paths, scope, _) = context(&state, &query).await?;
    Ok(Json(jsonc_editor::read_for_scope(&paths, &scope)?))
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<ConfigWriteBody>,
) -> AppResult<Json<ConfigDocument>> {
    let (paths, scope, _) = context(&state, &query).await?;
    Ok(Json(jsonc_editor::write_for_scope(
        &paths,
        &scope,
        &body.content,
        body.expected_sha256.as_deref(),
        &state.paths.backups_dir,
    )?))
}

// MCP ----------------------------------------------------------------------

pub async fn list_mcp(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let document = jsonc_editor::read_for_scope(&paths, &scope)?;
    let servers = jsonc_editor::list_mcp_servers(&document);
    Ok(Json(serde_json::json!({
        "document": document,
        "servers": servers,
    })))
}

pub async fn put_mcp(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<McpWriteBody>,
) -> AppResult<Json<ConfigDocument>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let path = paths.config_path(&scope)?;
    Ok(Json(jsonc_editor::upsert_mcp_server(
        &path,
        &name,
        &body.server,
        body.expected_sha256.as_deref(),
        &state.paths.backups_dir,
    )?))
}

pub async fn delete_mcp(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<ConfigDocument>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let path = paths.config_path(&scope)?;
    Ok(Json(jsonc_editor::delete_mcp_server(
        &path,
        &name,
        query.expected_sha256.as_deref(),
        &state.paths.backups_dir,
    )?))
}
