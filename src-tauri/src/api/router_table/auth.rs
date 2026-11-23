//! Route definitions for authentication, passkeys and push subscriptions.

use crate::api::*;
use crate::app_state::AppState;
use axum::Router;
use axum::routing::{delete, get, post};
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/session", post(auth::create_session))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/sessions", get(auth::list_sessions))
        .route("/api/auth/passkey/status", get(passkey::status))
        .route("/api/auth/passkeys", get(passkey::list))
        .route("/api/auth/passkeys/{id}", delete(passkey::delete))
        .route(
            "/api/auth/passkey/register/start",
            post(passkey::register_start),
        )
        .route(
            "/api/auth/passkey/register/finish",
            post(passkey::register_finish),
        )
        .route("/api/auth/passkey/login/start", post(passkey::login_start))
        .route(
            "/api/auth/passkey/login/finish",
            post(passkey::login_finish),
        )
        .route("/api/push/vapid", get(push::vapid_key))
        .route("/api/push/subscribe", post(push::subscribe))
        .route("/api/push/unsubscribe", post(push::unsubscribe))
        .route("/api/push/subscriptions", get(push::subscriptions))
        .route("/api/push/test", post(push::test))
}
