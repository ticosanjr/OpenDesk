use super::{
    IncomingCallback, TelegramConfig, answer_callback_query, clear_buttons, send_chat_action,
};
use crate::{
    app_state::AppState,
    db::interactions,
    error::{AppError, AppResult},
};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub(super) async fn handle_callback(
    state: &std::sync::Arc<AppState>,
    config: &TelegramConfig,
    token: &str,
    callback: IncomingCallback,
) -> AppResult<()> {
    if !super::allowed(
        &config.allowlist,
        &callback.chat_id,
        callback.username.as_deref(),
    ) {
        let _ = answer_callback_query(token, &callback.callback_id, "Não autorizado.").await;
        return Ok(());
    }
    let result = if let Some(rest) = callback.data.strip_prefix("ti:") {
        resolve_interaction_callback(state, rest).await
    } else if let Some(rest) = callback.data.strip_prefix("pm:") {
        resolve_permission_callback(state, rest).await
    } else {
        Ok("Comando desconhecido.".to_string())
    };
    match result {
        Ok(message) => {
            let _ = answer_callback_query(token, &callback.callback_id, &message).await;
            let _ = clear_buttons(token, &callback.chat_id, callback.message_id).await;
        }
        Err(error) => {
            let _ = answer_callback_query(token, &callback.callback_id, &format!("{error}")).await;
        }
    }
    Ok(())
}

pub(super) async fn resolve_interaction_callback(
    state: &std::sync::Arc<AppState>,
    rest: &str,
) -> AppResult<String> {
    let (id, action) = rest
        .rsplit_once(':')
        .ok_or_else(|| AppError::BadRequest("callback de interação inválido".into()))?;
    let interaction = interactions::get(&state.db, id).await?;
    if interaction.status != "PENDING" {
        return Ok("Esta pergunta já foi respondida ou expirou.".into());
    }
    let (selected, decision) = match action {
        "approve" | "deny" => (Vec::new(), action.to_string()),
        _ => {
            let options = super::format::interaction_options(&interaction);
            let index = action
                .parse::<usize>()
                .ok()
                .filter(|index| *index < options.len())
                .ok_or_else(|| AppError::BadRequest("opção inexistente".into()))?;
            (vec![options[index].clone()], "answer".to_string())
        }
    };
    crate::api::interactions::resolve_internal(
        state,
        &interaction.id,
        selected,
        None,
        Some(decision),
    )
    .await?;
    Ok("Resposta registrada. A tarefa foi retomada.".into())
}

pub(super) async fn resolve_permission_callback(
    state: &std::sync::Arc<AppState>,
    rest: &str,
) -> AppResult<String> {
    let (id, decision) = rest
        .rsplit_once(':')
        .ok_or_else(|| AppError::BadRequest("callback de permissão inválido".into()))?;
    if !matches!(decision, "once" | "reject") {
        return Err(AppError::BadRequest("decisão inválida".into()));
    }
    crate::api::permissions::decide_internal(state, id, decision).await?;
    Ok(if decision == "once" {
        "Permissão aprovada.".to_string()
    } else {
        "Permissão negada.".to_string()
    })
}

pub(super) struct TypingGuard {
    cancel: CancellationToken,
    handle: JoinHandle<()>,
}

impl TypingGuard {
    pub(super) fn start(token: String, chat_id: String) -> Self {
        let cancel = CancellationToken::new();
        let handle = tokio::spawn({
            let cancel = cancel.clone();
            async move {
                loop {
                    let _ = send_chat_action(&token, &chat_id, "typing").await;
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        _ = tokio::time::sleep(Duration::from_secs(4)) => {}
                    }
                }
            }
        });
        Self { cancel, handle }
    }
}

impl Drop for TypingGuard {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.handle.abort();
    }
}
