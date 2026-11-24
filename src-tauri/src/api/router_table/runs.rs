//! Route definitions for queue, runs, A/B experiments and chat threads.

use crate::api::*;
use crate::app_state::AppState;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, patch, post, put};
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
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
        .route("/api/chat/threads/{id}/files", post(chat::send_file))
        .route(
            "/api/chat/threads/{id}/email-reply",
            post(chat::send_email_reply),
        )
        .route(
            "/api/chat/threads/{id}/interrupt",
            post(chat::interrupt_thread),
        )
        .route(
            "/api/chat/threads/{id}/turns/latest",
            get(chat::latest_turn),
        )
        .route(
            "/api/chat/threads/{id}/prompt-preview",
            get(chat::prompt_preview),
        )
        .route("/api/queue", get(queue::list_queue))
        .route("/api/queue/status", get(queue::queue_status))
        .route(
            "/api/queue/config",
            get(queue::queue_config).put(queue::update_queue_config),
        )
        .route("/api/queue/pause", post(queue::pause_queue))
        .route("/api/queue/resume", post(queue::resume_queue))
        .route("/api/queue/{id}/cancel", post(queue::cancel_job))
        .route("/api/queue/{id}/retry", post(queue::retry_job))
        .route("/api/queue/{id}/priority", patch(queue::update_priority))
        .route("/api/queue/concurrency", put(queue::set_concurrency))
        .route("/api/runs", get(runs::list_runs))
        .route("/api/runs/ab", get(ab::list).post(ab::create))
        .route("/api/runs/ab/{id}", get(ab::get))
        .route("/api/runs/ab/{id}/choose", post(ab::choose))
        .route("/api/runs/{id}", get(runs::get_run))
        .route("/api/runs/{id}/log", get(runs::run_log))
        .route("/api/runs/{id}/timeline", get(runs::run_timeline))
        .route("/api/runs/{id}/thread", get(runs::run_thread_timeline))
        .route("/api/runs/{id}/export", get(runs::export_run))
        .route(
            "/api/runs/{id}/retry-other-model",
            post(tasks::retry_other_model),
        )
        .route("/api/runs/{id}/fork", post(tasks::fork_from_run))
}
