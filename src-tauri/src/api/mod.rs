pub mod about;
pub mod artifacts;
pub mod attachments;
pub mod attention;
pub mod blueprints;
pub mod budget;
pub mod calendar;
pub mod capture;
pub mod chat;
pub mod commands;
pub mod cost;
pub mod dedupe;
pub mod engine;
pub mod files;
pub mod google;
pub mod governance;
pub mod integrations;
pub mod io_bus;
pub mod learning;
pub mod meta;
pub mod permissions;
pub mod proposals;
pub mod queue;
pub mod recipes;
pub mod router;
pub mod runs;
pub mod schedules;
pub mod search;
pub mod signal;
pub mod starter_packs;
pub mod studio;
pub mod studio_ops;
pub mod tasks;
pub mod updates;
pub mod ws;

use crate::{
    app_state::AppState,
    db::{models::DashboardStats, settings, workspaces},
    error::{AppError, AppResult},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderValue, Method, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
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

pub(crate) fn router(state: Arc<AppState>) -> Router {
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
        .route("/api/about", get(about::about))
        .route("/api/meta/build", post(meta::build))
        .route("/api/daemon/status", get(daemon_status))
        .route("/api/daemon/restart", post(restart_daemon))
        .route("/api/engine/status", get(engine::status))
        .route("/api/engine/detect", post(engine::detect))
        .route("/api/engine/binary", post(engine::set_binary))
        .route("/api/engine/restart", post(engine::restart))
        .route("/api/engine/diagnostics", get(engine::diagnostics))
        .route("/api/models/health", get(engine::model_health))
        .route("/api/models/health/clear", post(engine::clear_model_health))
        .route("/api/engine/update/check", post(engine::update_check))
        .route("/api/engine/update", post(engine::update))
        .route("/api/engine/rollback", post(engine::rollback))
        .route("/api/updates/status", get(updates::status))
        .route("/api/updates/check", post(updates::check))
        .route(
            "/api/updates/token",
            axum::routing::put(updates::put_token).delete(updates::delete_token),
        )
        .route("/api/updates/download", post(updates::download))
        .route("/api/updates/rollback", post(updates::rollback))
        .route("/api/models", get(models))
        .route("/api/agents", get(agents))
        .route("/api/dashboard", get(dashboard))
        .route("/api/attention", get(attention::attention))
        .route("/api/attention/dismiss", post(attention::dismiss))
        .route("/api/starter-packs", get(starter_packs::list_packs))
        .route(
            "/api/starter-packs/templates",
            get(starter_packs::list_templates),
        )
        .route(
            "/api/starter-packs/templates/create",
            post(starter_packs::create_template),
        )
        .route(
            "/api/starter-packs/{id}",
            get(starter_packs::get_pack).delete(starter_packs::uninstall_pack),
        )
        .route(
            "/api/starter-packs/{id}/preview",
            post(starter_packs::preview_pack),
        )
        .route(
            "/api/starter-packs/{id}/items",
            get(starter_packs::list_items),
        )
        .route(
            "/api/starter-packs/{id}/items/install",
            post(starter_packs::install_item),
        )
        .route(
            "/api/starter-packs/{id}/install",
            post(starter_packs::install_pack),
        )
        .route(
            "/api/attachments",
            post(attachments::save)
                .delete(attachments::delete)
                .layer(DefaultBodyLimit::max(attachments::MAX_BODY_BYTES)),
        )
        .route(
            "/api/capture",
            post(capture::capture).layer(DefaultBodyLimit::max(attachments::MAX_BODY_BYTES)),
        )
        .route("/api/blueprints", get(blueprints::list_blueprints))
        .route(
            "/api/blueprints/{id}/apply",
            post(blueprints::apply_blueprint),
        )
        .route("/api/cost/mix", get(cost::model_mix))
        .route("/api/permissions", get(permissions::list_permissions))
        .route(
            "/api/permissions/{request_id}/reply",
            post(permissions::reply_permission),
        )
        .route("/api/agents/states", get(permissions::agent_states))
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
        .route("/api/settings/dedupe", get(dedupe::get).put(dedupe::put))
        .route("/api/settings/{key}", get(get_setting).put(put_setting))
        .route("/api/app/defaults", get(app_defaults))
        .route("/api/studio/agents", get(studio::list_agents))
        .route(
            "/api/studio/agent",
            get(studio::get_agent)
                .put(studio::put_agent)
                .delete(studio::delete_agent),
        )
        .route("/api/studio/skills", get(studio::list_skills))
        .route(
            "/api/studio/skill",
            get(studio::get_skill)
                .put(studio::put_skill)
                .delete(studio::delete_skill),
        )
        .route("/api/studio/commands", get(studio::list_commands))
        .route(
            "/api/studio/command",
            get(studio::get_command)
                .put(studio::put_command)
                .delete(studio::delete_command),
        )
        .route(
            "/api/studio/config",
            get(studio::get_config).put(studio::put_config),
        )
        .route("/api/studio/effective", get(studio_ops::effective))
        .route("/api/studio/catalog", get(studio_ops::catalog))
        .route("/api/studio/runtime", get(studio_ops::runtime))
        .route(
            "/api/studio/runtime/import",
            post(studio_ops::import_runtime),
        )
        .route(
            "/api/studio/install/preview",
            post(studio_ops::install_preview),
        )
        .route("/api/studio/install/apply", post(studio_ops::install_apply))
        .route(
            "/api/studio/{kind}/{id}/duplicate",
            post(studio_ops::duplicate),
        )
        .route("/api/studio/{kind}/{id}/copy", post(studio_ops::copy))
        .route("/api/studio/{kind}/{id}/move", post(studio_ops::move_item))
        .route("/api/studio/{kind}/{id}/toggle", post(studio_ops::toggle))
        .route(
            "/api/studio/{kind}/{id}/backups",
            get(studio_ops::list_backups),
        )
        .route(
            "/api/studio/{kind}/{id}/restore",
            post(studio_ops::restore_backup),
        )
        .route(
            "/api/studio/{kind}/{id}",
            axum::routing::delete(studio_ops::remove_item),
        )
        .route("/api/studio/mcp", get(studio::list_mcp))
        .route(
            "/api/studio/mcp/{name}",
            put(studio::put_mcp).delete(studio::delete_mcp),
        )
        .route(
            "/api/tasks",
            get(tasks::list_tasks).post(tasks::create_task),
        )
        .route(
            "/api/tasks/{id}",
            get(tasks::get_task)
                .patch(tasks::update_task)
                .delete(tasks::delete_task),
        )
        .route("/api/tasks/{id}/dispatch", post(tasks::dispatch_task))
        .route("/api/tasks/{id}/approve-plan", post(tasks::approve_plan))
        .route("/api/tasks/{id}/move", post(tasks::move_task))
        .route("/api/tasks/{id}/transition", post(tasks::transition_task))
        .route("/api/tasks/{id}/diagnostics", get(tasks::task_diagnostics))
        .route("/api/tasks/{id}/fork", post(tasks::fork_task))
        .route("/api/tasks/{id}/swarm", post(tasks::swarm_task))
        .route("/api/tasks/{id}/context", get(permissions::context))
        .route("/api/tasks/{id}/compact", post(permissions::compact))
        .route(
            "/api/tasks/{id}/memory",
            get(tasks::get_task_memory).post(tasks::post_task_memory),
        )
        .route(
            "/api/tasks/{id}/messages",
            get(tasks::list_task_messages).post(tasks::post_task_message),
        )
        .route(
            "/api/chat/threads",
            get(chat::list_threads).post(chat::create_thread),
        )
        .route(
            "/api/chat/threads/{id}",
            get(chat::get_thread)
                .patch(chat::update_thread)
                .delete(chat::delete_thread),
        )
        .route(
            "/api/chat/threads/{id}/messages",
            post(chat::send_message).layer(DefaultBodyLimit::max(attachments::MAX_BODY_BYTES)),
        )
        .route("/api/queue", get(queue::list_queue))
        .route("/api/queue/status", get(queue::queue_status))
        .route("/api/queue/pause", post(queue::pause_queue))
        .route("/api/queue/resume", post(queue::resume_queue))
        .route("/api/queue/{id}/cancel", post(queue::cancel_job))
        .route("/api/queue/{id}/retry", post(queue::retry_job))
        .route("/api/queue/{id}/priority", patch(queue::update_priority))
        .route("/api/queue/concurrency", put(queue::set_concurrency))
        .route(
            "/api/calendar/events",
            get(calendar::list_events).post(calendar::create_event),
        )
        .route(
            "/api/calendar/events/{id}",
            get(calendar::get_event)
                .patch(calendar::update_event)
                .delete(calendar::delete_event),
        )
        .route("/api/calendar/sources", get(calendar::list_sources))
        .route(
            "/api/calendar/sources/{id}",
            axum::routing::patch(calendar::update_source).delete(calendar::delete_source),
        )
        .route(
            "/api/calendar/sources/{id}/sync",
            post(calendar::sync_source),
        )
        .route(
            "/api/calendar/google/calendars",
            get(calendar::google_calendars),
        )
        .route("/api/calendar/google/import", post(calendar::import_google))
        .route("/api/calendar/sync", post(calendar::sync_calendars))
        .route("/api/calendar/agenda", get(calendar::agenda))
        .route("/api/runs", get(runs::list_runs))
        .route("/api/runs/{id}", get(runs::get_run))
        .route("/api/runs/{id}/log", get(runs::run_log))
        .route("/api/runs/{id}/timeline", get(runs::run_timeline))
        .route("/api/runs/{id}/export", get(runs::export_run))
        .route(
            "/api/runs/{id}/retry-other-model",
            post(tasks::retry_other_model),
        )
        .route("/api/runs/{id}/fork", post(tasks::fork_from_run))
        .route("/api/search", get(search::search))
        .route("/api/files/tree", get(files::tree))
        .route("/api/files/search", get(files::search))
        .route("/api/files/content", get(files::content))
        .route("/api/files/raw", get(files::raw))
        .route("/api/files/media", get(files::media))
        .route(
            "/api/io/inbound",
            get(io_bus::list_inbound).post(io_bus::create_inbound),
        )
        .route(
            "/api/io/inbound/{id}",
            axum::routing::patch(io_bus::update_inbound).delete(io_bus::delete_inbound),
        )
        .route(
            "/api/io/outbound",
            get(io_bus::list_outbound).post(io_bus::create_outbound),
        )
        .route(
            "/api/io/outbound/{id}",
            axum::routing::patch(io_bus::update_outbound).delete(io_bus::delete_outbound),
        )
        .route("/api/io/secrets", post(io_bus::put_secret))
        .route(
            "/api/io/secrets/{key}",
            axum::routing::delete(io_bus::delete_secret),
        )
        .route("/api/recipes", get(recipes::list).post(recipes::sync))
        .route("/api/recipes/sync", post(recipes::sync))
        .route("/api/recipes/runs", get(recipes::runs))
        .route("/api/recipes/runs/{id}", get(recipes::run_detail))
        .route("/api/recipes/hitl", get(recipes::hitl_pending))
        .route(
            "/api/recipes/hitl/{token}/resolve",
            post(recipes::resolve_hitl),
        )
        .route("/api/recipes/{id}", get(recipes::get).put(recipes::save))
        .route("/api/recipes/{id}/enabled", post(recipes::set_enabled))
        .route("/api/recipes/{id}/run", post(recipes::run))
        .route("/api/io/tools", get(io_bus::tools))
        .route("/api/io/tools/{name}/install", post(io_bus::install_tool))
        .route("/api/io/tools/{name}/path", put(io_bus::set_tool_path))
        .route(
            "/api/tasks/{id}/sinks",
            get(io_bus::task_sinks).post(io_bus::set_task_sinks),
        )
        .route(
            "/api/schedules/{id}/sinks",
            get(io_bus::schedule_sinks).post(io_bus::set_schedule_sinks),
        )
        .route("/api/webhooks/{source_id}", post(io_bus::webhook))
        .route(
            "/api/proposals",
            get(proposals::list).post(proposals::create),
        )
        .route("/api/proposals/{id}/resolve", post(proposals::resolve))
        .route("/api/governance", get(governance::overview))
        .route("/api/governance/agents/{id}", put(governance::put_agent))
        .route(
            "/api/governance/agents/{id}/heartbeat",
            post(governance::heartbeat_now),
        )
        .route("/api/governance/goals", post(governance::create_goal))
        .route(
            "/api/governance/goals/{id}",
            put(governance::update_goal).delete(governance::delete_goal),
        )
        .route(
            "/api/governance/delegations",
            post(governance::create_delegation),
        )
        .route(
            "/api/governance/delegations/{id}/resolve",
            post(governance::resolve_delegation),
        )
        .route("/api/learning", get(learning::overview))
        .route("/api/learning/settings", put(learning::update_settings))
        .route("/api/learning/scan", post(learning::scan))
        .route(
            "/api/learning/lessons/{id}/dismiss",
            post(learning::dismiss),
        )
        .route(
            "/api/learning/lessons/{id}/promote",
            post(learning::promote),
        )
        .route(
            "/api/learning/lessons/{id}/remember",
            post(learning::remember),
        )
        .route("/api/artifacts", get(artifacts::list_artifacts))
        .route("/api/artifacts/pinned", get(artifacts::pinned_artifacts))
        .route("/api/artifacts/{id}", get(artifacts::get_artifact))
        .route("/api/artifacts/{id}/raw", get(artifacts::artifact_raw))
        .route("/api/artifacts/{id}/pin", post(artifacts::pin_artifact))
        .route(
            "/api/artifacts/{id}/rollback",
            post(artifacts::rollback_artifact),
        )
        .route(
            "/api/artifacts/{id}/content",
            get(artifacts::artifact_content),
        )
        .route("/api/router/config", get(router::get_config))
        .route("/api/router/model", put(router::set_router_model))
        .route("/api/router/blend", put(router::set_blend))
        .route("/api/router/analyze", post(router::analyze))
        .route(
            "/api/router/rules",
            get(router::list_rules).post(router::create_rule),
        )
        .route(
            "/api/router/rules/{id}",
            patch(router::update_rule).delete(router::delete_rule),
        )
        .route(
            "/api/budget",
            get(budget::get_budget).put(budget::put_budget),
        )
        .route("/api/budget/status", get(budget::budget_status))
        .route(
            "/api/schedules",
            get(schedules::list_schedules).post(schedules::create_schedule),
        )
        .route("/api/schedules/tick", post(schedules::tick_scheduler))
        .route("/api/schedules/preview", post(schedules::preview_schedule))
        .route("/api/schedules/{id}/runs", get(schedules::schedule_runs))
        .route("/api/schedules/{id}/dry-run", post(schedules::dry_run))
        .route(
            "/api/schedules/{id}/notify-test",
            post(schedules::test_notification),
        )
        .route(
            "/api/schedules/{id}",
            get(schedules::get_schedule)
                .patch(schedules::update_schedule)
                .delete(schedules::delete_schedule),
        )
        .route("/api/schedules/{id}/run", post(schedules::run_schedule_now))
        .route(
            "/api/schedules/{id}/active",
            post(schedules::set_schedule_active),
        )
        .route(
            "/api/integrations/smtp",
            get(integrations::get_smtp).put(integrations::put_smtp),
        )
        .route("/api/integrations/smtp/test", post(integrations::test_smtp))
        .route(
            "/api/integrations/deliveries",
            get(integrations::list_deliveries),
        )
        .route(
            "/api/integrations/deliveries/{id}/retry",
            post(integrations::retry_delivery),
        )
        .route("/api/integrations/google/status", get(google::status))
        .route(
            "/api/integrations/google/config",
            get(google::get_config).put(google::put_config),
        )
        .route(
            "/api/integrations/google/oauth/start",
            post(google::start_oauth),
        )
        .route(
            "/api/integrations/google/disconnect",
            post(google::disconnect),
        )
        .route(
            "/api/integrations/google/mcp/register",
            post(google::register_mcp),
        )
        .route(
            "/api/integrations/signal",
            get(signal::status).put(signal::put_config),
        )
        .route("/api/integrations/signal/link", post(signal::link))
        .route("/api/integrations/signal/start", post(signal::start))
        .route("/api/integrations/signal/stop", post(signal::stop))
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
    if let Err(error) = crate::proposals::ensure_workspace_bridge(&state, &workspace.id).await {
        tracing::warn!(error = %error, "could not register config MCP bridge for workspace");
    }
    state.refresh_watcher().await?;
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
    let workspace = workspaces::update(&state.db, &state.paths, &id, input).await?;
    state.refresh_watcher().await?;
    Ok(Json(workspace))
}

async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    workspaces::delete(&state.db, &id).await?;
    state.refresh_watcher().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn activate_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<workspaces::Workspace>> {
    workspaces::touch(&state.db, &id).await?;
    settings::set_active_workspace(&state.db, &id).await?;
    state.refresh_watcher().await?;
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

async fn app_defaults(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let workspaces_root = settings::get(&state.db, "workspaces_root")
        .await?
        .unwrap_or_else(|| json!(crate::paths::default_workspaces_root().to_string_lossy()));

    Ok(Json(json!({
        "workspaces_root": workspaces_root,
        "default_policy": "COST_BENEFIT",
        "default_timezone": "UTC",
    })))
}

async fn put_setting(
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

#[cfg(test)]
mod tests;
