use crate::{
    app_state::AppState,
    db::auth_sessions,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header::AUTHORIZATION},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct CreateSessionBody {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub ttl_days: Option<i64>,
}

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSessionBody>,
) -> AppResult<Json<serde_json::Value>> {
    let label = body
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("remote");
    let (token, session) =
        auth_sessions::create(&state.db, label, body.ttl_days.unwrap_or(30)).await?;
    Ok(Json(json!({
        "token": token,
        "session": session,
    })))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::BadRequest("token ausente".into()))?;
    if token == state.api_token {
        return Err(AppError::BadRequest(
            "o token local não pode ser revogado por esta rota".into(),
        ));
    }
    let revoked = auth_sessions::revoke(&state.db, token).await?;
    Ok(Json(json!({ "revoked": revoked })))
}

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<auth_sessions::AuthSession>>> {
    Ok(Json(auth_sessions::list(&state.db).await?))
}
