use crate::{
    app_state::{AppState, OPENCODE_BIN_SETTING},
    db::settings,
    error::{AppError, AppResult},
    opencode_client::{binary::OpenCodeBinaryInfo, daemon_manager::DaemonStatus},
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc, time::Duration};

const RESTART_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Serialize)]
pub struct EngineStatus {
    pub daemon: DaemonStatus,
    pub configured_binary: Option<String>,
    pub resolved: Option<OpenCodeBinaryInfo>,
    pub resolve_error: Option<String>,
    pub candidates: Vec<String>,
    pub app_version: &'static str,
}

#[derive(Debug, Serialize)]
pub struct DetectResult {
    pub ok: bool,
    pub binary: Option<OpenCodeBinaryInfo>,
    pub error: Option<String>,
    pub candidates: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct BinaryBody {
    pub path: String,
}

/// Return an actionable 503 when the engine is offline so dispatch and
/// "run now" never enqueue work that is guaranteed to fail.
pub async fn require_engine(state: &AppState) -> AppResult<()> {
    // Headless test mode deliberately disables the engine; the UI/queue tests
    // still need to enqueue work. Only honor the bypass in debug builds so a
    // stray environment variable can never make a release enqueue doomed jobs.
    if cfg!(debug_assertions) && std::env::var("OPENDESK_SKIP_DAEMON").is_ok() {
        return Ok(());
    }
    require_engine_strict(state).await
}

/// Same check without the headless-test bypass. Used by Kanban transitions,
/// where the test must prove that a failed dispatch keeps the chosen column.
pub async fn require_engine_strict(state: &AppState) -> AppResult<()> {
    let status = state.daemon.status().await;
    if status.online {
        return Ok(());
    }

    let mut message = String::from(
        "OpenCode is offline, so this action was not queued. Open Settings → OpenCode Engine to fix it.",
    );
    if let Some(reason) = status.message {
        message.push_str(" Reason: ");
        message.push_str(&reason);
    } else {
        message.push_str(" No OpenCode binary was detected and no shared service is reachable.");
    }
    Err(AppError::Daemon(message))
}

pub async fn status(State(state): State<Arc<AppState>>) -> Json<EngineStatus> {
    let daemon = state.daemon.status().await;
    let locator = state.locator.clone();
    let configured_binary = state
        .locator
        .configured()
        .map(|path| path.to_string_lossy().to_string());
    let (resolved, resolve_error) = match locator.resolve() {
        Ok(binary) => (Some(binary), None),
        Err(error) => (None, Some(format!("{error:#}"))),
    };
    let candidates = locator
        .candidates()
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    Json(EngineStatus {
        daemon,
        configured_binary,
        resolved,
        resolve_error,
        candidates,
        app_version: env!("CARGO_PKG_VERSION"),
    })
}

pub async fn detect(State(state): State<Arc<AppState>>) -> Json<DetectResult> {
    let locator = state.locator.clone();
    let candidates = locator
        .candidates()
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    match locator.resolve() {
        Ok(binary) => Json(DetectResult {
            ok: true,
            binary: Some(binary),
            error: None,
            candidates,
        }),
        Err(error) => Json(DetectResult {
            ok: false,
            binary: None,
            error: Some(format!("{error:#}")),
            candidates,
        }),
    }
}

pub async fn set_binary(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BinaryBody>,
) -> AppResult<Json<EngineStatus>> {
    let raw = body.path.trim();
    if raw.is_empty() {
        return Err(AppError::BadRequest(
            "Informe o caminho completo do binário opencode.".into(),
        ));
    }
    let path = PathBuf::from(raw);
    let info = state.locator.resolve_path(&path)?;

    settings::set(
        &state.db,
        OPENCODE_BIN_SETTING,
        &Value::String(info.path.clone()),
    )
    .await?;
    state.locator.set_configured(Some(path));
    state.daemon.restart().await;
    let _ = state.daemon.wait_for_settled(RESTART_TIMEOUT).await;

    Ok(status(State(state)).await)
}

pub async fn restart(State(state): State<Arc<AppState>>) -> Json<EngineStatus> {
    state.daemon.restart().await;
    let _ = state.daemon.wait_for_settled(RESTART_TIMEOUT).await;
    status(State(state)).await
}
