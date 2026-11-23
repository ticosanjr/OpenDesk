//! Axum route table for the local OpenDesk API.
//!
//! The routes themselves live in per-domain submodules; this file only merges
//! them and applies the shared fallback, middleware and static assets. Every
//! route path, handler name and layer order is unchanged.

use crate::api::core::{auth_middleware, guard_middleware};
use crate::app_state::AppState;
use axum::{Router, http::HeaderValue, middleware};
use std::sync::Arc;
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

mod auth;
mod files;
mod integrations;
mod runs;
mod schedules;
mod studio;
mod system;
mod tasks;

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

    let router = Router::new()
        .merge(auth::routes())
        .merge(files::routes())
        .merge(integrations::routes())
        .merge(runs::routes())
        .merge(schedules::routes())
        .merge(studio::routes())
        .merge(system::routes())
        .merge(tasks::routes())
        .fallback(crate::api::core::not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            guard_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let web_dir = std::env::var("OPENDESK_WEB_DIR").unwrap_or_else(|_| "dist".to_string());
    let index = std::path::Path::new(&web_dir).join("index.html");
    let router = if index.is_file() {
        router.fallback_service(ServeDir::new(&web_dir).fallback(ServeFile::new(index)))
    } else {
        router
    };
    router.layer(middleware::from_fn(crate::api::core::security_headers))
}
