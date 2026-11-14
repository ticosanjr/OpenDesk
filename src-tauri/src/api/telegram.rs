use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    integrations::telegram::{self, TelegramConfig, TelegramStatus},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use std::sync::Arc;

pub async fn status(State(state): State<Arc<AppState>>) -> Json<TelegramStatus> {
    Json(telegram::status(&state).await)
}

#[derive(Debug, Deserialize)]
pub struct TelegramConfigInput {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub clear_token: bool,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub agent: String,
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(input): Json<TelegramConfigInput>,
) -> AppResult<Json<TelegramStatus>> {
    if input.clear_token {
        state.secrets.delete("telegram", "bot_token")?;
        state.telegram.clear_bot().await;
        telegram::clear_offset(&state.db).await;
    } else if let Some(token) = input.token.as_deref().map(str::trim)
        && !token.is_empty()
    {
        if !telegram::valid_token_shape(token) {
            return Err(AppError::BadRequest(
                "token do Telegram inválido; use o formato 123456:ABC…".into(),
            ));
        }
        state.secrets.set("telegram", "bot_token", token)?;
        state.telegram.clear_bot().await;
        telegram::clear_offset(&state.db).await;
    }

    let mut allowlist = Vec::new();
    for entry in input.allowlist {
        let entry = entry.trim().to_string();
        if !entry.is_empty() && !allowlist.contains(&entry) {
            allowlist.push(entry);
        }
    }
    let config = TelegramConfig {
        enabled: input.enabled,
        allowlist,
        workspace_id: input
            .workspace_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        agent: if input.agent.trim().is_empty() {
            "build".into()
        } else {
            input.agent.trim().to_string()
        },
    };
    telegram::save(&state.db, &config).await?;
    let _ = state.telegram.start_if_enabled(state.clone()).await;
    Ok(Json(telegram::status(&state).await))
}

pub async fn start(State(state): State<Arc<AppState>>) -> Json<TelegramStatus> {
    let _ = state.telegram.start_if_enabled(state.clone()).await;
    Json(telegram::status(&state).await)
}

pub async fn stop(State(state): State<Arc<AppState>>) -> Json<TelegramStatus> {
    state.telegram.stop().await;
    Json(telegram::status(&state).await)
}

pub async fn test(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let token = telegram::token_for(&state).await?;
    let bot = telegram::get_me(&token).await?;
    let config = telegram::load(&state.db).await?;
    let targets = telegram::resolve_recipients(&state.db, &config.allowlist).await;
    if targets.is_empty() {
        return Err(AppError::BadRequest(
            "nenhum chat conhecido na allowlist; envie /start para o bot pelo celular primeiro"
                .into(),
        ));
    }
    let message = "OpenDesk conectado. Envie uma mensagem para conversar com o agente.";
    for target in &targets {
        telegram::send_text(&token, target, message).await?;
    }
    Ok(Json(serde_json::json!({
        "sent": targets.len(),
        "bot": bot,
    })))
}
