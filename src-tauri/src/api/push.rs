use crate::{
    app_state::AppState,
    db::push,
    error::{AppError, AppResult},
};
use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct SubscribeBody {
    pub endpoint: String,
    #[serde(default)]
    pub label: Option<String>,
    pub keys: SubscribeKeys,
}

#[derive(Debug, Deserialize)]
pub struct SubscribeKeys {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeBody {
    pub endpoint: String,
}

/// Public half of the VAPID pair. The browser passes it as `applicationServerKey`.
pub async fn vapid_key(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({ "public_key": state.push_keys.public_key }))
}

pub async fn subscribe(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SubscribeBody>,
) -> AppResult<Json<push::PushSubscription>> {
    let label = body.label.as_deref().unwrap_or("device");
    let subscription = push::upsert(
        &state.db,
        &body.endpoint,
        &body.keys.p256dh,
        &body.keys.auth,
        label,
    )
    .await?;
    Ok(Json(subscription))
}

pub async fn unsubscribe(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UnsubscribeBody>,
) -> AppResult<Json<serde_json::Value>> {
    let removed = push::remove(&state.db, body.endpoint.trim()).await?;
    Ok(Json(json!({ "removed": removed })))
}

pub async fn subscriptions(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<push::PushSubscription>>> {
    Ok(Json(push::list(&state.db).await?))
}

pub async fn test(
    State(state): State<Arc<AppState>>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let payload = crate::push::PushPayload::new(
        "OpenDesk",
        "Notificação de teste do control plane.",
        "/#/settings",
    );
    let delivered = crate::push::send_to_all(&state.db, &state.paths, &payload).await?;
    if delivered == 0 {
        return Err(AppError::BadRequest(
            "nenhum dispositivo com push ativo aceitou a notificação".into(),
        ));
    }
    Ok((StatusCode::OK, Json(json!({ "delivered": delivered }))))
}
