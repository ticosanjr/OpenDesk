use super::http::{TypingGuard, handle_callback};
use super::{
    BotInfo, IncomingTelegram, IncomingTelegramFile, MAX_FILE_BYTES, OFFSET_KEY, TelegramConfig,
    TelegramUpdate, allowed, delete_webhook, download_file, get_me, get_updates, help_text, load,
    parse_interaction_reply, record_chat, safe_file_name, send_reply, send_text, token,
};
use crate::{
    app_state::AppState,
    chat,
    db::{chat as chat_db, interactions, settings, workspaces},
    error::{AppError, AppResult},
};
use serde_json::json;
use std::{path::Path, sync::Arc, time::Duration};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct TelegramBridge {
    handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    last_error: Mutex<Option<String>>,
    bot: Mutex<Option<BotInfo>>,
}

impl TelegramBridge {
    pub async fn is_running(&self) -> bool {
        self.handle
            .lock()
            .await
            .as_ref()
            .map(|handle| !handle.is_finished())
            .unwrap_or(false)
    }

    pub async fn last_error(&self) -> Option<String> {
        self.last_error.lock().await.clone()
    }

    pub async fn bot(&self) -> Option<BotInfo> {
        self.bot.lock().await.clone()
    }

    pub async fn clear_bot(&self) {
        *self.bot.lock().await = None;
    }

    async fn set_error(&self, error: Option<String>) {
        *self.last_error.lock().await = error;
    }

    pub async fn stop(&self) {
        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
    }

    pub async fn start_if_enabled(self: &Arc<Self>, state: Arc<AppState>) -> AppResult<bool> {
        self.stop().await;
        let config = load(&state.db).await?;
        if !config.enabled {
            self.set_error(None).await;
            return Ok(false);
        }
        let token = match token(&state.secrets) {
            Some(token) => token,
            None => {
                let message = "Telegram está habilitado, mas o token do bot não foi configurado.";
                self.set_error(Some(message.into())).await;
                return Err(AppError::BadRequest(message.into()));
            }
        };
        if config.allowlist.is_empty() {
            let message = "Telegram está habilitado, mas a allowlist está vazia.";
            self.set_error(Some(message.into())).await;
            return Err(AppError::BadRequest(message.into()));
        }
        let bot = match get_me(&token).await {
            Ok(bot) => bot,
            Err(error) => {
                let message = format!("token do Telegram inválido: {error}");
                self.set_error(Some(message.clone())).await;
                return Err(AppError::BadRequest(message));
            }
        };
        *self.bot.lock().await = Some(bot);
        if let Err(error) = delete_webhook(&token).await {
            let message = format!("não foi possível liberar o long polling: {error}");
            self.set_error(Some(message.clone())).await;
            return Err(AppError::Daemon(message));
        }

        let bridge = Arc::clone(self);
        let handle = tokio::spawn(async move {
            if let Err(error) = Arc::clone(&bridge).run(state).await {
                bridge.set_error(Some(format!("{error:#}"))).await;
                tracing::error!(error = %error, "Telegram bridge stopped");
            }
        });
        *self.handle.lock().await = Some(handle);
        self.set_error(None).await;
        Ok(true)
    }

    async fn run(self: Arc<Self>, state: Arc<AppState>) -> anyhow::Result<()> {
        let token =
            token(&state.secrets).ok_or_else(|| anyhow::anyhow!("Telegram token missing"))?;
        let mut offset = settings::get(&state.db, OFFSET_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let mut backoff = 2u64;
        loop {
            match get_updates(&token, offset).await {
                Ok(updates) => {
                    backoff = 2;
                    self.set_error(None).await;
                    for update in updates {
                        let update_id = match &update {
                            TelegramUpdate::Message(message) => message.update_id,
                            TelegramUpdate::Callback(callback) => callback.update_id,
                        };
                        offset = offset.max(update_id + 1);
                        let config = load(&state.db).await?;
                        let result = match update {
                            TelegramUpdate::Message(message) => {
                                handle_incoming(&state, &config, &token, message).await
                            }
                            TelegramUpdate::Callback(callback) => {
                                handle_callback(&state, &config, &token, callback).await
                            }
                        };
                        if let Err(error) = result {
                            tracing::warn!(error = %error, "Telegram update failed");
                        }
                        let _ = settings::set(&state.db, OFFSET_KEY, &json!(offset)).await;
                    }
                }
                Err(error) => {
                    self.set_error(Some(error.to_string())).await;
                    tracing::warn!(error = %error, "Telegram getUpdates failed");
                    tokio::time::sleep(Duration::from_secs(backoff)).await;
                    backoff = (backoff * 2).min(60);
                }
            }
        }
    }
}

async fn handle_incoming(
    state: &Arc<AppState>,
    config: &TelegramConfig,
    token: &str,
    incoming: IncomingTelegram,
) -> AppResult<()> {
    if !allowed(
        &config.allowlist,
        &incoming.chat_id,
        incoming.username.as_deref(),
    ) {
        tracing::warn!(
            chat_id = %incoming.chat_id,
            username = ?incoming.username,
            "Telegram message rejected by allowlist"
        );
        return Ok(());
    }
    let _ = record_chat(
        &state.db,
        &incoming.chat_id,
        incoming.username.as_deref(),
        incoming.chat_title.as_deref(),
    )
    .await;

    let trimmed = incoming.text.trim();
    let lower = trimmed.to_lowercase();
    if matches!(lower.as_str(), "/start" | "/help" | "start" | "ajuda") {
        send_text(token, &incoming.chat_id, &help_text(&config.agent)).await?;
        return Ok(());
    }

    // Short code for task interactions: #1a2b3c4d 2
    if let Some((short, answer)) = parse_interaction_reply(trimmed)
        && let Some(interaction) = interactions::find_pending_by_prefix(&state.db, short).await?
    {
        let (decision, selected, text) =
            crate::api::interactions::parse_remote_answer(&interaction, answer);
        let message = match crate::api::interactions::resolve_internal(
            state,
            &interaction.id,
            selected,
            text,
            Some(decision),
        )
        .await
        {
            Ok(_) => "Resposta registrada. A tarefa foi retomada.".to_string(),
            Err(error) => format!("Não consegui registrar a resposta: {error}"),
        };
        send_reply(token, &incoming.chat_id, incoming.message_id, &message).await?;
        return Ok(());
    }

    if crate::recipes::hitl::telegram::handle_reply(
        &crate::app_state::recipe_context(state),
        &incoming.chat_id,
        trimmed,
        &[],
    )
    .await?
    {
        return Ok(());
    }
    if let Some(rest) = lower
        .strip_prefix("aprovar ")
        .or_else(|| lower.strip_prefix("negar "))
    {
        let approve = lower.starts_with("aprovar ");
        let prefix = rest.split_whitespace().next().unwrap_or_default();
        let decision = crate::db::permissions::decide_prefix(
            &state.db,
            prefix,
            if approve { "once" } else { "reject" },
        )
        .await?;
        let message = match decision {
            Some(_) if approve => format!("Permissão {prefix} aprovada."),
            Some(_) => format!("Permissão {prefix} negada."),
            None => format!("Nenhum pedido pendente com o prefixo {prefix}."),
        };
        send_reply(token, &incoming.chat_id, incoming.message_id, &message).await?;
        return Ok(());
    }

    if matches!(lower.as_str(), "/parar" | "parar" | "/stop" | "stop") {
        let thread =
            chat_db::find_external_any_workspace(&state.db, "telegram", &incoming.chat_id).await?;
        let Some(thread) = thread else {
            send_reply(
                token,
                &incoming.chat_id,
                incoming.message_id,
                "Nenhuma execução em andamento.",
            )
            .await?;
            return Ok(());
        };
        if let Some((_turn_id, session_id)) = state.cancel_chat_turn(&thread.id) {
            let session = session_id.or(thread.opencode_session_id);
            if let Some(session) = session
                && let Some(client) = state.daemon.client().await
            {
                let _ = client.interrupt(&session).await;
            }
            send_reply(
                token,
                &incoming.chat_id,
                incoming.message_id,
                "Interrompendo a execução…",
            )
            .await?;
        } else {
            send_reply(
                token,
                &incoming.chat_id,
                incoming.message_id,
                "Nenhuma execução em andamento.",
            )
            .await?;
        }
        return Ok(());
    }

    let configured_workspace = config.workspace_id.clone();
    let workspace_id = match &configured_workspace {
        Some(id) => id.clone(),
        None => settings::active_workspace_id(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("no active workspace for Telegram".into()))?,
    };
    let mut thread =
        chat_db::find_external(&state.db, &workspace_id, "telegram", &incoming.chat_id).await?;
    if thread.is_none() && configured_workspace.is_none() {
        thread =
            chat_db::find_external_any_workspace(&state.db, "telegram", &incoming.chat_id).await?;
    }
    let label = incoming
        .chat_title
        .clone()
        .or_else(|| incoming.username.clone())
        .unwrap_or_else(|| incoming.chat_id.clone());
    let thread = match thread {
        Some(thread) => thread,
        None => {
            let thread = chat_db::create_thread(
                &state.db,
                chat_db::CreateThreadInput {
                    workspace_id: workspace_id.clone(),
                    title: Some(format!("Telegram · {label}")),
                    agent: config.agent.clone(),
                    model: None,
                    source: Some("telegram".into()),
                    external_id: Some(incoming.chat_id.clone()),
                },
            )
            .await?;
            state
                .events
                .publish(crate::events::OpenDeskWsEvent::ChatUpdated {
                    thread_id: thread.id.clone(),
                    workspace_id: thread.workspace_id.clone(),
                });
            thread
        }
    };

    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &state.db,
        Some(&thread.workspace_id),
    )
    .await
    .unwrap_or_default();
    let context =
        chat::channel_context_with(&templates, "Telegram", &thread.workspace_id, &thread.id);
    let mut content = trimmed.to_string();
    if let Some(file) = &incoming.file {
        match save_incoming_file(state, &thread.workspace_id, &incoming, file, token).await {
            Ok(path) => {
                if content.is_empty() {
                    content = format!(
                        "Arquivo recebido:\n- `{path}`\n\nConfirme o recebimento em uma linha e diga o que pode fazer com ele."
                    );
                } else {
                    content = format!("{content}\n\nArquivo recebido:\n- `{path}`");
                }
            }
            Err(error) => {
                let message = format!("Não consegui salvar o arquivo: {error}");
                let _ = send_reply(token, &incoming.chat_id, incoming.message_id, &message).await;
                return Ok(());
            }
        }
    }
    if state.active_chat_turn(&thread.id).is_some() {
        send_reply(
            token,
            &incoming.chat_id,
            incoming.message_id,
            "Ainda estou trabalhando nesta conversa. Envie /parar para interromper.",
        )
        .await?;
        return Ok(());
    }

    let background_state = state.clone();
    let bot_token = token.to_string();
    let chat_id = incoming.chat_id.clone();
    let message_id = incoming.message_id;
    tokio::spawn(async move {
        run_chat_reply(
            background_state,
            bot_token,
            chat_id,
            message_id,
            thread,
            content,
            context,
        )
        .await;
    });
    Ok(())
}

async fn run_chat_reply(
    state: Arc<AppState>,
    token: String,
    chat_id: String,
    message_id: i64,
    thread: crate::db::chat::ChatThread,
    content: String,
    context: String,
) {
    let typing = TypingGuard::start(token.clone(), chat_id.clone());
    let progress = tokio::spawn({
        let token = token.clone();
        let chat_id = chat_id.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_secs(120)).await;
                let _ = send_text(&token, &chat_id, "Ainda estou trabalhando…").await;
            }
        }
    });
    let result = chat::send_message_with_context(&state, thread, &content, Some(&context)).await;
    progress.abort();
    drop(typing);
    match result {
        Ok(reply) => {
            let _ = send_reply(&token, &chat_id, message_id, &reply.assistant.content).await;
        }
        Err(error) => {
            let _ = send_reply(
                &token,
                &chat_id,
                message_id,
                &format!("Não consegui responder agora: {error}"),
            )
            .await;
        }
    }
}

async fn save_incoming_file(
    state: &Arc<AppState>,
    workspace_id: &str,
    incoming: &IncomingTelegram,
    file: &IncomingTelegramFile,
    token: &str,
) -> AppResult<String> {
    if file.size.is_some_and(|size| size > MAX_FILE_BYTES as i64) {
        return Err(AppError::BadRequest(format!(
            "o arquivo excede o limite de {} MB do Bot API",
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let relative_dir = Path::new("input").join("telegram").join(&day);
    let file_name = safe_file_name(&format!(
        "{}-{}-{}",
        incoming.chat_id, incoming.message_id, file.file_name
    ));
    let relative = relative_dir.join(file_name);
    let destination = Path::new(&workspace.root_path).join(&relative);
    download_file(token, &file.file_id, &destination).await?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}
