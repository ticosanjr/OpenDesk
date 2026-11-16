use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    passkey::{
        self, FinishLogin, FinishRegistration, LoginResult, LoginStart, PasskeyRecord,
        RegistrationStart,
    },
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;

pub async fn status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let origin = passkey::origin_from_headers(&headers);
    let (origin_value, origin_error, origin_available) = match origin {
        Ok((rp_id, url)) => (Some(format!("{rp_id} @ {url}")), None, true),
        Err(error) => (None, Some(error.to_string()), false),
    };
    let value = passkey::status(&state.db, origin_value.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "registered": value.registered,
        "enabled": value.enabled,
        "origin": value.origin,
        "origin_available": origin_available,
        "origin_error": origin_error,
    })))
}

pub async fn register_start(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<Json<RegistrationStart>> {
    Ok(Json(
        passkey::start_registration(&state.db, &headers).await?,
    ))
}

pub async fn register_finish(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<FinishRegistration>,
) -> AppResult<Json<PasskeyRecord>> {
    Ok(Json(
        passkey::finish_registration(&state.db, &headers, body).await?,
    ))
}

pub async fn login_start(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<Json<LoginStart>> {
    Ok(Json(passkey::start_login(&state.db, &headers).await?))
}

pub async fn login_finish(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<FinishLogin>,
) -> AppResult<Json<LoginResult>> {
    Ok(Json(
        passkey::finish_login(&state.db, &headers, body).await?,
    ))
}

pub async fn list(State(state): State<Arc<AppState>>) -> AppResult<Json<Vec<PasskeyRecord>>> {
    Ok(Json(passkey::list(&state.db).await?))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    if !passkey::delete(&state.db, &id).await? {
        return Err(AppError::NotFound(format!("passkey {id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}
