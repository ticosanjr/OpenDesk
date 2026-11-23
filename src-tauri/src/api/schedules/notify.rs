use crate::{
    app_state::AppState,
    db::schedules,
    error::{AppError, AppResult},
    integrations::{signal, smtp_mailer, telegram},
    secrets::SecretStore,
    text::split_recipients,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct NotifyTestBody {
    pub channel: String,
}

pub async fn test_notification(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<NotifyTestBody>,
) -> AppResult<Json<serde_json::Value>> {
    let schedule = schedules::get(&state.db, &id).await?;
    let subject = format!("[OpenDesk] Teste de notificação · {}", schedule.name);
    let html = format!(
        "<p>Este é um teste de notificação da rotina <strong>{}</strong>.</p>\
         <p>Próximo disparo: {}</p>",
        schedule.name,
        schedule.next_run_at.as_deref().unwrap_or("não agendado")
    );
    match body.channel.as_str() {
        "email" => {
            let recipients =
                split_recipients(schedule.notify_email_recipients.as_deref().unwrap_or(""));
            if recipients.is_empty() {
                return Err(AppError::BadRequest(
                    "a rotina não tem destinatários de e-mail".into(),
                ));
            }
            let secrets = SecretStore::new(&state.paths);
            let config = smtp_mailer::load_config(&state.db, &secrets, &schedule.workspace_id)
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest("SMTP não está configurado para este workspace".into())
                })?;
            for recipient in &recipients {
                smtp_mailer::send_email(&config, recipient, &subject, html.clone(), Vec::new())
                    .await
                    .map_err(|error| AppError::Daemon(format!("SMTP test failed: {error:#}")))?;
            }
            Ok(Json(serde_json::json!({
                "sent": recipients.len(),
                "channel": "email"
            })))
        }
        "signal" => {
            let config = signal::load(&state.db).await?;
            if config.account.trim().is_empty() {
                return Err(AppError::BadRequest(
                    "Signal não está vinculado neste OpenDesk".into(),
                ));
            }
            let mut recipients =
                split_recipients(schedule.notify_signal_recipients.as_deref().unwrap_or(""));
            if recipients.is_empty() {
                recipients = config.allowlist.clone();
            }
            if recipients.is_empty() {
                return Err(AppError::BadRequest(
                    "a rotina não tem destinatários Signal e a allowlist está vazia".into(),
                ));
            }
            let message = format!("[OpenDesk] Teste da rotina {}", schedule.name);
            for recipient in &recipients {
                signal::send_text(&config.cli_path, &config.account, recipient, &message).await?;
            }
            Ok(Json(serde_json::json!({
                "sent": recipients.len(),
                "channel": "signal"
            })))
        }
        "telegram" => {
            let config = telegram::load(&state.db).await?;
            if !config.enabled {
                return Err(AppError::BadRequest(
                    "Telegram não está habilitado neste OpenDesk".into(),
                ));
            }
            let token = telegram::token_for(&state).await?;
            let mut recipients =
                split_recipients(schedule.notify_telegram_recipients.as_deref().unwrap_or(""));
            if recipients.is_empty() {
                recipients = config.allowlist.clone();
            }
            let recipients = telegram::resolve_recipients(&state.db, &recipients).await;
            if recipients.is_empty() {
                return Err(AppError::BadRequest(
                    "a rotina não tem destinatários Telegram conhecidos".into(),
                ));
            }
            let message = format!("[OpenDesk] Teste da rotina {}", schedule.name);
            for recipient in &recipients {
                telegram::send_text(&token, recipient, &message).await?;
            }
            Ok(Json(serde_json::json!({
                "sent": recipients.len(),
                "channel": "telegram"
            })))
        }
        other => Err(AppError::BadRequest(format!(
            "invalid notification channel '{other}'; expected email, signal or telegram"
        ))),
    }
}
