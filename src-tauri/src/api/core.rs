use crate::{
    app_state::AppState,
    db::{models::DashboardStats, settings, workspaces},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, Request, State},
    http::{
        Method, StatusCode,
        header::{AUTHORIZATION, RETRY_AFTER},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use std::{sync::Arc, time::Instant};

pub(crate) async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

pub(crate) async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let daemon = state.daemon.status().await;
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "started_at": state.started_at,
        "daemon": daemon,
    }))
}

pub(crate) async fn meta() -> impl IntoResponse {
    Json(json!({
        "name": "OpenDesk",
        "version": env!("CARGO_PKG_VERSION"),
        "phase": "resilience",
        "opencode_api": "v2",
        "features": [
            "daemon-manager",
            "sqlite-state",
            "local-api",
            "websocket-events",
            "system-tray",
            "workspace-registry",
            "studio-ssot",
            "dsh-router",
            "persistent-queue",
            "budget-guard",
            "opencode-sessions",
            "artifact-provenance",
            "kanban",
            "meta-builder",
            "cron-scheduler",
            "smtp-delivery",
            "encrypted-secrets",
            "google-workspace",
            "google-oauth",
            "mcp-bridge",
            "engine-resolver",
            "engine-shared-attach",
            "engine-update",
            "opendesk-self-update",
            "diagnostics"
        ]
    }))
}

pub(crate) async fn daemon_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.daemon.status().await)
}

pub(crate) async fn restart_daemon(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.daemon.restart().await;
    Json(json!({ "ok": true }))
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct DirectoryQuery {
    directory: Option<String>,
}

pub(crate) async fn models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DirectoryQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let models = state
        .daemon
        .models(query.directory.as_deref())
        .await
        .map_err(|error| AppError::Daemon(format!("{error:#}")))?;
    Ok(Json(json!({ "data": models })))
}

pub(crate) async fn agents(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DirectoryQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let agents = state
        .daemon
        .agents(query.directory.as_deref())
        .await
        .map_err(|error| AppError::Daemon(format!("{error:#}")))?;
    Ok(Json(json!({ "data": agents })))
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct WorkspaceQuery {
    workspace_id: Option<String>,
}

pub(crate) async fn dashboard(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> AppResult<Json<DashboardStats>> {
    let workspace_id = match query.workspace_id {
        Some(id) => Some(id),
        None => settings::active_workspace_id(&state.db).await?,
    };

    let Some(workspace_id) = workspace_id else {
        return Ok(Json(DashboardStats::default()));
    };

    let mut stats = sqlx::query_as::<_, DashboardStats>(
        "SELECT
            COUNT(*) AS runs_total,
            COALESCE(SUM(CASE WHEN status = 'SUCCESS' THEN 1 ELSE 0 END), 0) AS runs_success,
            COALESCE(SUM(CASE WHEN status = 'FAILED' THEN 1 ELSE 0 END), 0) AS runs_failed,
            COALESCE(SUM(CASE WHEN status = 'RUNNING' THEN 1 ELSE 0 END), 0) AS runs_running,
            COALESCE(SUM(tokens_input), 0) AS tokens_input,
            COALESCE(SUM(tokens_output), 0) AS tokens_output,
            COALESCE(SUM(tokens_cache_read), 0) AS tokens_cache_read,
            COALESCE(SUM(tokens_cache_write), 0) AS tokens_cache_write,
            CAST(COALESCE(SUM(estimated_cost_usd), 0.0) AS REAL) AS estimated_cost_usd,
            CAST(COALESCE(SUM(CASE WHEN started_at >= strftime('%Y-%m-%dT00:00:00Z', 'now')
                THEN estimated_cost_usd ELSE 0 END), 0.0) AS REAL) AS daily_cost_usd,
            0 AS queue_running,
            0 AS queue_waiting,
            0 AS artifacts_total
         FROM task_runs
         WHERE workspace_id = ?",
    )
    .bind(&workspace_id)
    .fetch_one(&state.db)
    .await?;

    let (queue_running, queue_waiting): (i64, i64) = sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN status = 'RUNNING' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status IN ('QUEUED', 'PAUSED') THEN 1 ELSE 0 END), 0)
         FROM execution_queue
         WHERE workspace_id = ?",
    )
    .bind(&workspace_id)
    .fetch_one(&state.db)
    .await?;

    let artifacts_total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM artifacts_provenance WHERE workspace_id = ?")
            .bind(&workspace_id)
            .fetch_one(&state.db)
            .await?;

    stats.queue_running = queue_running;
    stats.queue_waiting = queue_waiting;
    stats.artifacts_total = artifacts_total;

    Ok(Json(stats))
}

pub(crate) async fn list_workspaces(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<workspaces::Workspace>>> {
    Ok(Json(workspaces::list(&state.db).await?))
}

pub(crate) async fn create_workspace(
    State(state): State<Arc<AppState>>,
    Json(input): Json<workspaces::CreateWorkspaceInput>,
) -> AppResult<Json<workspaces::Workspace>> {
    let workspace = workspaces::create(&state.db, &state.paths, input).await?;
    settings::set_active_workspace(&state.db, &workspace.id).await?;
    if let Err(error) = crate::proposals::ensure_workspace_bridge(&state, &workspace.id).await {
        tracing::warn!(error = %error, "could not register MCP bridges for workspace");
    }
    if let Err(error) = crate::starter_packs::install_base_pack(&state, &workspace.id).await {
        tracing::warn!(workspace = %workspace.id, error = %error, "could not install operational base pack");
    }
    state.refresh_watcher().await?;
    Ok(Json(workspace))
}

pub(crate) async fn get_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<workspaces::Workspace>> {
    Ok(Json(workspaces::get(&state.db, &id).await?))
}

pub(crate) async fn update_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<workspaces::UpdateWorkspaceInput>,
) -> AppResult<Json<workspaces::Workspace>> {
    let workspace = workspaces::update(&state.db, &state.paths, &id, input).await?;
    state.refresh_watcher().await?;
    Ok(Json(workspace))
}

pub(crate) async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    workspaces::delete(&state.db, &id).await?;
    state.refresh_watcher().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn activate_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<workspaces::Workspace>> {
    workspaces::touch(&state.db, &id).await?;
    settings::set_active_workspace(&state.db, &id).await?;
    state.refresh_watcher().await?;
    Ok(Json(workspaces::get(&state.db, &id).await?))
}

pub(crate) async fn list_settings(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<crate::db::models::AppSetting>>> {
    Ok(Json(settings::all(&state.db).await?))
}

pub(crate) async fn get_setting(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    settings::get(&state.db, &key)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("setting {key}")))
}

pub(crate) async fn app_defaults(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    let workspaces_root = settings::get(&state.db, "workspaces_root")
        .await?
        .unwrap_or_else(|| json!(crate::paths::default_workspaces_root().to_string_lossy()));

    Ok(Json(json!({
        "workspaces_root": workspaces_root,
        "default_policy": "COST_BENEFIT",
        "default_timezone": "UTC",
    })))
}

pub(crate) async fn put_setting(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(value): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let value = if key == "workspaces_root" {
        let raw = value
            .as_str()
            .ok_or_else(|| AppError::BadRequest("workspaces_root must be a string".into()))?;
        let path = std::path::Path::new(raw.trim());
        if raw.trim().is_empty() {
            return Err(AppError::BadRequest(
                "workspaces_root cannot be empty".into(),
            ));
        }
        std::fs::create_dir_all(path)?;
        let canonical = std::fs::canonicalize(path)?;
        json!(canonical.to_string_lossy())
    } else {
        value
    };

    settings::set(&state.db, &key, &value).await?;
    Ok(Json(value))
}

pub(crate) async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "code": "not_found",
            "message": "route not found"
        })),
    )
}

pub(crate) async fn guard_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    if let Some(class) = crate::audit::rate_class(&method, uri.path())
        && let Err(retry_after) = state.rate_limiter.check(class)
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(RETRY_AFTER, retry_after.to_string())],
            Json(json!({
                "code": "rate_limited",
                "message": format!(
                    "limite local de requisições excedido; tente novamente em {retry_after}s"
                ),
            })),
        )
            .into_response();
    }
    let started = Instant::now();
    let response = next.run(request).await;
    let status = response.status().as_u16();
    if crate::audit::should_audit(&method, uri.path()) {
        let pool = state.db.clone();
        let duration_ms = started.elapsed().as_millis() as i64;
        tokio::spawn(async move {
            if let Err(error) =
                crate::audit::record(&pool, &method, &uri, status, duration_ms).await
            {
                tracing::warn!(error = %error, "api audit write failed");
            }
        });
    }
    response
}

pub(crate) async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    if request.method() == Method::OPTIONS
        || path == "/healthz"
        || path == "/api/health"
        || path.starts_with("/api/webhooks/")
    {
        return next.run(request).await;
    }

    let header_token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let query_token = if path == "/ws" {
        query_token(request.uri())
    } else {
        None
    };

    let authorized = header_token
        .map(|token| token == state.api_token)
        .unwrap_or(false)
        || query_token
            .map(|token| token == state.api_token)
            .unwrap_or(false);

    if authorized {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "code": "unauthorized",
                "message": "missing or invalid OpenDesk API token"
            })),
        )
            .into_response()
    }
}

pub(crate) fn query_token(uri: &axum::http::Uri) -> Option<String> {
    uri.query()?.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key != "token" {
            return None;
        }
        Some(
            urlencoding::decode(value)
                .map(|value| value.into_owned())
                .unwrap_or_else(|_| value.to_string()),
        )
    })
}
