//! Route definitions for health, metrics, engine/updater, attention and the WebSocket endpoint.

use crate::api::core::*;
use crate::api::*;
use crate::app_state::AppState;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(crate::api::core::readyz))
        .route("/api/health", get(health))
        .route("/api/onboarding", get(onboarding::status))
        .route(
            "/api/mcp",
            get(mcp::stream).post(mcp::rpc).delete(mcp::delete),
        )
        .route("/api/meta", get(meta))
        .route("/api/about", get(about::about))
        .route(
            "/api/about/desktop-integration",
            post(about::repair_desktop),
        )
        .route("/api/meta/build", post(meta::build))
        .route("/api/daemon/status", get(daemon_status))
        .route("/api/daemon/restart", post(restart_daemon))
        .route("/api/engine/status", get(engine::status))
        .route("/api/engine/detect", post(engine::detect))
        .route("/api/engine/binary", post(engine::set_binary))
        .route("/api/engine/restart", post(engine::restart))
        .route("/api/engine/diagnostics", get(engine::diagnostics))
        .route("/api/models/health", get(engine::model_health))
        .route("/api/models/test", post(engine::test_model))
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
        .route("/api/metrics", get(metrics::get))
        .route("/api/audit", get(audit::list))
        .route("/api/attention", get(attention::attention))
        .route("/api/attention/dismiss", post(attention::dismiss))
        .route("/api/config-bundle/export", post(config_bundle::export))
        .route(
            "/api/config-bundle/preview",
            post(config_bundle::preview).layer(DefaultBodyLimit::max(
                crate::config_bundle::MAX_BUNDLE_BYTES,
            )),
        )
        .route(
            "/api/config-bundle/import",
            post(config_bundle::import).layer(DefaultBodyLimit::max(
                crate::config_bundle::MAX_BUNDLE_BYTES,
            )),
        )
        .route(
            "/api/diagnostics/self-test",
            post(self_test::run).layer(DefaultBodyLimit::max(1024 * 1024)),
        )
        .route("/api/test/seed", post(seed::seed))
        .route(
            "/api/budget",
            get(budget::get_budget).put(budget::put_budget),
        )
        .route("/api/budget/status", get(budget::budget_status))
        .route("/ws", get(ws::ws_handler))
}
