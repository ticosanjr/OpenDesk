use crate::{
    app_state::AppState,
    error::AppResult,
    integrations::signal::{self, SignalConfig, SignalStatus},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use std::sync::Arc;

pub async fn status(State(state): State<Arc<AppState>>) -> Json<SignalStatus> {
    Json(signal::status(&state).await)
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(mut config): Json<SignalConfig>,
) -> AppResult<Json<SignalStatus>> {
    config.account = config.account.trim().to_string();
    config.cli_path = if config.cli_path.trim().is_empty() {
        signal::default_cli_path()
    } else {
        config.cli_path.trim().to_string()
    };
    config.agent = if config.agent.trim().is_empty() {
        "build".into()
    } else {
        config.agent.trim().to_string()
    };
    config.allowlist = config
        .allowlist
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    signal::save(&state.db, &config).await?;
    let _ = state.signal.start_if_enabled(state.clone()).await;
    Ok(Json(signal::status(&state).await))
}

#[derive(Debug, Deserialize)]
pub struct LinkBody {
    #[serde(default)]
    pub cli_path: Option<String>,
}

pub async fn link(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LinkBody>,
) -> AppResult<Json<serde_json::Value>> {
    let config = signal::load(&state.db).await?;
    let cli = body
        .cli_path
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if config.cli_path.trim().is_empty() {
                signal::default_cli_path()
            } else {
                config.cli_path.clone()
            }
        });
    let uri = state.signal.link(&cli).await?;
    Ok(Json(serde_json::json!({
        "uri": uri,
        "command": format!("{cli} link -n OpenDesk"),
    })))
}

pub async fn stop(State(state): State<Arc<AppState>>) -> Json<SignalStatus> {
    state.signal.stop().await;
    Json(signal::status(&state).await)
}

pub async fn start(State(state): State<Arc<AppState>>) -> Json<SignalStatus> {
    let _ = state.signal.start_if_enabled(state.clone()).await;
    Json(signal::status(&state).await)
}
