//! Route definitions for tasks, interactions, permissions and prompt templates.

use crate::api::*;
use crate::app_state::AppState;
use axum::Router;
use axum::routing::{get, post};
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/permissions", get(permissions::list_permissions))
        .route(
            "/api/permissions/{request_id}/reply",
            post(permissions::reply_permission),
        )
        .route("/api/interactions/pending", get(interactions::list_pending))
        .route("/api/interactions/{id}", get(interactions::get_interaction))
        .route(
            "/api/interactions/{id}/resolve",
            post(interactions::resolve),
        )
        .route(
            "/api/tasks/{id}/interactions",
            get(interactions::list_for_task).post(interactions::ask),
        )
        .route("/api/agents/states", get(permissions::agent_states))
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
        .route("/api/tasks/{id}/session-packet", get(tasks::session_packet))
        .route("/api/tasks/{id}/dispatch", post(tasks::dispatch_task))
        .route("/api/tasks/{id}/approve-plan", post(tasks::approve_plan))
        .route("/api/tasks/{id}/move", post(tasks::move_task))
        .route("/api/tasks/{id}/transition", post(tasks::transition_task))
        .route("/api/tasks/{id}/hold", post(tasks::hold_task))
        .route("/api/tasks/{id}/diagnostics", get(tasks::task_diagnostics))
        .route("/api/tasks/{id}/fork", post(tasks::fork_task))
        .route("/api/tasks/{id}/swarm", post(tasks::swarm_task))
        .route(
            "/api/tasks/{id}/prompt-preview",
            get(prompt_preview::preview),
        )
        .route(
            "/api/prompt/templates",
            get(prompt_templates::get).put(prompt_templates::put),
        )
        .route("/api/prompt/templates/reset", post(prompt_templates::reset))
        .route(
            "/api/prompt-templates/export",
            get(prompt_templates::export),
        )
        .route(
            "/api/prompt-templates/import",
            post(prompt_templates::import).layer(axum::extract::DefaultBodyLimit::max(
                crate::config_bundle::MAX_BUNDLE_BYTES,
            )),
        )
        .route(
            "/api/tasks/{id}/watchers",
            get(task_watchers::list).put(task_watchers::replace),
        )
        .route("/api/tasks/{id}/context", get(permissions::context))
        .route("/api/tasks/{id}/compact", post(permissions::compact))
        .route(
            "/api/tasks/{id}/memory",
            get(tasks::get_task_memory)
                .post(tasks::post_task_memory)
                .delete(tasks::delete_task_memory),
        )
        .route(
            "/api/tasks/{id}/messages",
            get(tasks::list_task_messages).post(tasks::post_task_message),
        )
        .route(
            "/api/tasks/{id}/sinks",
            get(io_bus::task_sinks).post(io_bus::set_task_sinks),
        )
}
