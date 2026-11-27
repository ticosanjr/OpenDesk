use super::http::handle_callback;
use super::{
    BotInfo, IncomingTelegram, OFFSET_KEY, TelegramConfig, TelegramUpdate, allowed, delete_webhook,
    get_me, get_updates, help_text, load, parse_interaction_reply, record_chat, send_reply,
    send_text, token,
};
use crate::{
    app_state::AppState,
    chat,
    db::{channel_bots, chat as chat_db, interactions, settings},
    error::{AppError, AppResult},
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
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
                                handle_incoming(&state, &config, &token, message, None).await
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

/// Per-bot routing data resolved from `channel_chat_contexts`.
pub(crate) struct BotScope {
    pub bot_id: String,
    pub workspace_id: String,
    pub agent: String,
    pub system_context: String,
    pub memory_scope: String,
    pub context_id: String,
    pub opencode_session_id: Option<String>,
}

/// Routes one update received by a workspace bot, keeping every chat isolated
/// by `(bot_id, external_chat_id)`.
pub async fn handle_bot_update(
    state: &Arc<AppState>,
    bot: &channel_bots::ChannelBot,
    config: &TelegramConfig,
    token: &str,
    update: TelegramUpdate,
) -> AppResult<()> {
    match update {
        TelegramUpdate::Message(message) => {
            let title = message
                .chat_title
                .clone()
                .or_else(|| message.username.clone())
                .unwrap_or_else(|| message.chat_id.clone());
            let context = channel_bots::upsert_seen_context(
                &state.db,
                bot,
                &message.chat_id,
                "group",
                &title,
            )
            .await?;
            if !context.enabled {
                tracing::debug!(
                    bot_id = %bot.id,
                    chat_id = %message.chat_id,
                    "contexto desabilitado: mensagem ignorada"
                );
                return Ok(());
            }
            let scope = BotScope {
                bot_id: bot.id.clone(),
                workspace_id: bot.workspace_id.clone(),
                agent: context
                    .agent_override
                    .clone()
                    .unwrap_or_else(|| bot.default_agent.clone()),
                system_context: context.system_context.clone(),
                memory_scope: context.memory_scope.clone(),
                context_id: context.id.clone(),
                opencode_session_id: context.opencode_session_id.clone(),
            };
            handle_incoming(state, config, token, message, Some(&scope)).await
        }
        TelegramUpdate::Callback(callback) => {
            super::http::handle_callback(state, config, token, callback).await
        }
    }
}

async fn handle_incoming(
    state: &Arc<AppState>,
    config: &TelegramConfig,
    token: &str,
    incoming: IncomingTelegram,
    scope: Option<&BotScope>,
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

    let external_id = scope
        .map(|scope| format!("telegram:{}:{}", scope.bot_id, incoming.chat_id))
        .unwrap_or_else(|| incoming.chat_id.clone());
    if matches!(lower.as_str(), "/parar" | "parar" | "/stop" | "stop") {
        let thread = if scope.is_some() {
            chat_db::find_external(
                &state.db,
                &scope
                    .map(|scope| scope.workspace_id.clone())
                    .unwrap_or_default(),
                "telegram",
                &external_id,
            )
            .await?
        } else {
            chat_db::find_external_any_workspace(&state.db, "telegram", &incoming.chat_id).await?
        };
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

    let configured_workspace = scope
        .map(|scope| scope.workspace_id.clone())
        .or_else(|| config.workspace_id.clone());
    let workspace_id = match &configured_workspace {
        Some(id) => id.clone(),
        None => settings::active_workspace_id(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("no active workspace for Telegram".into()))?,
    };
    let mut thread =
        chat_db::find_external(&state.db, &workspace_id, "telegram", &external_id).await?;
    if thread.is_none() && configured_workspace.is_none() {
        thread = chat_db::find_external_any_workspace(&state.db, "telegram", &external_id).await?;
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
                    agent: scope
                        .map(|scope| scope.agent.clone())
                        .unwrap_or_else(|| config.agent.clone()),
                    model: None,
                    source: Some("telegram".into()),
                    external_id: Some(external_id.clone()),
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

    if let Some(scope) = scope
        && let Some(session) = scope.opencode_session_id.as_deref()
        && thread.opencode_session_id.as_deref() != Some(session)
    {
        chat_db::update_session(&state.db, &thread.id, session).await?;
    }
    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &state.db,
        Some(&thread.workspace_id),
    )
    .await
    .unwrap_or_default();
    let mut context =
        chat::channel_context_with(&templates, "Telegram", &thread.workspace_id, &thread.id);
    if let Some(scope) = scope {
        if !scope.system_context.trim().is_empty() {
            context.push_str("\n\n## Contexto deste grupo\n");
            context.push_str(scope.system_context.trim());
        }
        if scope.memory_scope == "none" {
            context.push_str("\n\nEsta conversa não deve usar memória pessoal ou estruturada.");
        }
    }
    let mut content = trimmed.to_string();
    if let Some(file) = &incoming.file {
        match super::reply::save_incoming_file(state, &thread.workspace_id, &incoming, file, token)
            .await
        {
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
    let context_id = scope.map(|scope| scope.context_id.clone());
    let chat_id = incoming.chat_id.clone();
    let message_id = incoming.message_id;
    tokio::spawn(async move {
        super::reply::run_chat_reply(
            background_state,
            super::reply::ReplyRequest {
                token: bot_token,
                chat_id,
                message_id,
                thread,
                content,
                context,
                context_id,
            },
        )
        .await;
    });
    Ok(())
}
