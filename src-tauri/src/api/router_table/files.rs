//! Route definitions for files, I/O bus, artifacts, attachments, backups, proposals, search and recipes.

use crate::api::*;
use crate::app_state::AppState;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, patch, post, put};
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
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
        .route("/api/search/recent", get(search::recent))
        .route("/api/search", get(search::search))
        .route("/api/files/tree", get(files::tree))
        .route("/api/files/search", get(files::search))
        .route("/api/files/content", get(files::content))
        .route("/api/files/archive", get(files::archive))
        .route("/api/files/save", axum::routing::post(files::save_file))
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
        .route("/api/recipes/validate", post(recipes::validate))
        .route("/api/recipes/runs", get(recipes::runs))
        .route("/api/recipes/runs/{id}", get(recipes::run_detail))
        .route("/api/recipes/runs/{id}/cancel", post(recipes::cancel_run))
        .route("/api/recipes/runs/{id}/retry", post(recipes::retry_run))
        .route("/api/recipes/runs/{id}/resume", post(recipes::resume_run))
        .route(
            "/api/recipes/runs/{id}/nodes/{*path}",
            post(recipes::retry_node),
        )
        .route("/api/recipes/hitl", get(recipes::hitl_pending))
        .route(
            "/api/recipes/hitl/{token}/resolve",
            post(recipes::resolve_hitl),
        )
        .route(
            "/api/recipes/{id}",
            get(recipes::get)
                .put(recipes::save)
                .delete(recipes::delete_recipe),
        )
        .route("/api/recipes/{id}/enabled", post(recipes::set_enabled))
        .route("/api/recipes/{id}/versions", get(recipes::versions))
        .route(
            "/api/recipes/{id}/versions/{version_id}",
            get(recipes::version),
        )
        .route(
            "/api/recipes/{id}/versions/{version_id}/diff",
            get(recipes::version_diff),
        )
        .route("/api/recipes/{id}/rollback", post(recipes::rollback))
        .route("/api/recipes/{id}/run", post(recipes::run))
        .route("/api/io/tools", get(io_bus::tools))
        .route("/api/io/tools/{name}/install", post(io_bus::install_tool))
        .route("/api/io/tools/{name}/path", put(io_bus::set_tool_path))
        .route(
            "/api/proposals",
            get(proposals::list).post(proposals::create),
        )
        .route("/api/proposals/{id}/resolve", post(proposals::resolve))
        .route("/api/backups", get(backups::list).post(backups::create))
        .route("/api/backups/{id}/download", get(backups::download))
        .route("/api/backups/{id}/restore", post(backups::restore))
        .route("/api/backups/{id}", axum::routing::delete(backups::delete))
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
        .route(
            "/api/artifacts/{id}/archive",
            get(artifacts::artifact_archive),
        )
        .route(
            "/api/artifacts/{id}/save",
            axum::routing::post(artifacts::save_artifact),
        )
        .route("/api/router/config", get(router::get_config))
        .route("/api/router/model", put(router::set_router_model))
        .route("/api/router/models", put(router::set_models))
        .route("/api/router/blend", put(router::set_blend))
        .route("/api/router/analyze", post(router::analyze))
        .route("/api/router/test", post(router::test_decision))
        .route(
            "/api/router/rules",
            get(router::list_rules).post(router::create_rule),
        )
        .route(
            "/api/router/rules/{id}",
            patch(router::update_rule).delete(router::delete_rule),
        )
}
