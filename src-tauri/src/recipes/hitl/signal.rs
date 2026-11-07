use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    integrations::signal,
    recipes::hitl::{HitlRequestPayload, parse_reply, resolve},
    recipes::store,
};
use std::sync::Arc;

pub async fn dispatch(state: &Arc<AppState>, payload: &HitlRequestPayload) -> AppResult<String> {
    let config = signal::load(&state.db).await?;
    if !config.enabled || config.account.trim().is_empty() || config.allowlist.is_empty() {
        return Err(AppError::BadRequest(
            "Signal não está habilitado ou sem allowlist".into(),
        ));
    }
    let text = format!(
        "[OD-#{}] {}\n\n{}\n\nResponda APROVAR, REJEITAR ou escreva o feedback.",
        payload.token, payload.title, payload.message_body
    );
    for recipient in &config.allowlist {
        signal::send_with_attachments(
            &config.cli_path,
            &config.account,
            recipient,
            &text,
            &payload.attachments,
        )
        .await?;
    }
    Ok(format!("signal-{}", chrono::Utc::now().timestamp_millis()))
}

pub async fn handle_reply(
    state: &Arc<AppState>,
    sender: &str,
    text: &str,
    attachments: &[String],
) -> AppResult<bool> {
    let Some(token) = crate::recipes::hitl::extract_token(text) else {
        return Ok(false);
    };
    let Ok(request) = store::get_hitl(&state.db, &token).await else {
        return Ok(false);
    };
    if request.status != "PENDING" {
        return Ok(true);
    }
    let (kind, payload) = if let Some(path) = attachments.first() {
        (
            "ARTIFACT".to_string(),
            Some(serde_json::json!({ "path": path }).to_string()),
        )
    } else {
        parse_reply(&request.interaction_type, &[], text)
    };
    resolve(
        state,
        &token,
        "SIGNAL",
        Some(sender),
        &kind,
        payload.as_deref(),
    )
    .await?;
    let config = signal::load(&state.db).await?;
    let _ = signal::send_text(
        &config.cli_path,
        &config.account,
        sender,
        "OpenDesk: resposta recebida.",
    )
    .await;
    Ok(true)
}
