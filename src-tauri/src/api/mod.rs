pub mod commands;
pub mod ws;

use crate::{
    app_state::AppState,
    db::{models::DashboardStats, settings, workspaces},
    error::{AppError, AppResult},
};
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderValue, Method, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    trace::TraceLayer,
};

pub async fn spawn_server(state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = state
        .take_listener()
        .ok_or_else(|| anyhow::anyhow!("OpenDesk API listener was already taken"))?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    let address = listener.local_addr()?;
    let app = router(state);

    tracing::info!(%address, "OpenDesk local API listening");

    tauri::async_runtime::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            tracing::error!(error = %error, "OpenDesk API stopped");
        }
    });

    Ok(())
}

fn router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("tauri://localhost"),
            HeaderValue::from_static("http://tauri.localhost"),
            HeaderValue::from_static("http://localhost:1420"),
            HeaderValue::from_static("http://127.0.0.1:1420"),
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
        ]))
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/health", get(health))
        .route("/api/meta", get(meta))
        .route("/api/daemon/status", get(daemon_status))
        .route("/api/daemon/restart", post(restart_daemon))
        .route("/api/models", get(models))
        .route("/api/agents", get(agents))
        .route("/api/dashboard", get(dashboard))
        .route(
            "/api/workspaces",
            get(list_workspaces).post(create_workspace),
        )
        .route(
            "/api/workspaces/{id}",
            get(get_workspace)
                .patch(update_workspace)
                .delete(delete_workspace),
        )
        .route("/api/workspaces/{id}/activate", post(activate_workspace))
        .route("/api/settings", get(list_settings))
        .route("/api/settings/{key}", get(get_setting).put(put_setting))
        .route("/ws", get(ws::ws_handler))
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let daemon = state.daemon.status().await;
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "started_at": state.started_at,
        "daemon": daemon,
    }))
}

async fn meta() -> impl IntoResponse {
    Json(json!({
        "name": "OpenDesk",
        "version": env!("CARGO_PKG_VERSION"),
        "phase": "foundation",
        "opencode_api": "v2",
        "features": [
            "daemon-manager",
            "sqlite-state",
            "local-api",
            "websocket-events",
            "system-tray",
            "workspace-registry"
        ]
    }))
}

async fn daemon_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.daemon.status().await)
}

async fn restart_daemon(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.daemon.restart().await;
    Json(json!({ "ok": true }))
}

#[derive(Debug, Deserialize, Default)]
struct DirectoryQuery {
    directory: Option<String>,
}

async fn models(
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

async fn agents(
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
struct WorkspaceQuery {
    workspace_id: Option<String>,
}

async fn dashboard(
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
            COALESCE(SUM(estimated_cost_usd), 0.0) AS estimated_cost_usd,
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

async fn list_workspaces(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<workspaces::Workspace>>> {
    Ok(Json(workspaces::list(&state.db).await?))
}

async fn create_workspace(
    State(state): State<Arc<AppState>>,
    Json(input): Json<workspaces::CreateWorkspaceInput>,
) -> AppResult<Json<workspaces::Workspace>> {
    let workspace = workspaces::create(&state.db, &state.paths, input).await?;
    settings::set_active_workspace(&state.db, &workspace.id).await?;
    Ok(Json(workspace))
}

async fn get_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<workspaces::Workspace>> {
    Ok(Json(workspaces::get(&state.db, &id).await?))
}

async fn update_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<workspaces::UpdateWorkspaceInput>,
) -> AppResult<Json<workspaces::Workspace>> {
    Ok(Json(
        workspaces::update(&state.db, &state.paths, &id, input).await?,
    ))
}

async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    workspaces::delete(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn activate_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<workspaces::Workspace>> {
    workspaces::touch(&state.db, &id).await?;
    settings::set_active_workspace(&state.db, &id).await?;
    Ok(Json(workspaces::get(&state.db, &id).await?))
}

async fn list_settings(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<crate::db::models::AppSetting>>> {
    Ok(Json(settings::all(&state.db).await?))
}

async fn get_setting(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    settings::get(&state.db, &key)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("setting {key}")))
}

async fn put_setting(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(value): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    settings::set(&state.db, &key, &value).await?;
    Ok(Json(value))
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "code": "not_found",
            "message": "route not found"
        })),
    )
}

async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    if request.method() == Method::OPTIONS || path == "/healthz" || path == "/api/health" {
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

fn query_token(uri: &axum::http::Uri) -> Option<String> {
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
