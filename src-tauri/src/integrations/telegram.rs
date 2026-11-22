use crate::{
    app_state::AppState,
    chat,
    db::{chat as chat_db, interactions, settings, tasks::Task, workspaces},
    error::{AppError, AppResult},
    hitl::HitlKind,
    secrets::SecretStore,
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;

#[allow(unused_imports)]
pub use opendesk_channels::telegram::{
    BotInfo, IncomingCallback, IncomingTelegram, IncomingTelegramFile, KnownChat, MAX_FILE_BYTES,
    OFFSET_KEY, TelegramConfig, TelegramStatus, TelegramUpdate, allowed, answer_callback_query,
    chunks, clear_buttons, clear_offset, delete_webhook, download_file, get_me, get_updates,
    help_text, known_chats, load, parse_callback_update, parse_interaction_reply, parse_update,
    parse_updates, record_chat, resolve_recipients, safe_file_name, save, send_chat_action,
    send_document, send_reply, send_text, send_text_with_markup, send_to_allowlist_parts, short_id,
    token, token_for_parts, truncate, valid_token_shape,
};

pub async fn token_for(state: &AppState) -> AppResult<String> {
    token_for_parts(&state.secrets).map_err(Into::into)
}

pub async fn status(state: &AppState) -> TelegramStatus {
    let config = load(&state.db).await.unwrap_or_default();
    let token = token(&state.secrets);
    let bot = state.telegram.bot().await;
    TelegramStatus {
        config,
        token_configured: token.is_some(),
        bot_username: bot.as_ref().and_then(|bot| bot.username.clone()),
        bot_name: bot.map(|bot| bot.first_name),
        bridge_running: state.telegram.is_running().await,
        last_error: state.telegram.last_error().await,
    }
}

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

fn interaction_options(interaction: &crate::db::interactions::TaskInteraction) -> Vec<String> {
    serde_json::from_str(&interaction.options_json).unwrap_or_default()
}

#[cfg(test)]
fn parse_selected_options(answer: &str, options: &[String], multiple: bool) -> Vec<String> {
    crate::api::interactions::parse_option_indices(answer, options, multiple)
}

async fn handle_callback(
    state: &Arc<AppState>,
    config: &TelegramConfig,
    token: &str,
    callback: IncomingCallback,
) -> AppResult<()> {
    if !allowed(
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

async fn resolve_interaction_callback(state: &Arc<AppState>, rest: &str) -> AppResult<String> {
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
            let options = interaction_options(&interaction);
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

async fn resolve_permission_callback(state: &Arc<AppState>, rest: &str) -> AppResult<String> {
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

/// Sends a task interaction to every allowlist chat that is already known.
pub async fn notify_task_interaction(
    state: &Arc<AppState>,
    interaction: &crate::db::interactions::TaskInteraction,
    task: &Task,
) -> AppResult<usize> {
    let config = load(&state.db).await?;
    if !config.enabled {
        return Ok(0);
    }
    let token = token_for(state).await?;
    let targets = resolve_recipients(&state.db, &config.allowlist).await;
    if targets.is_empty() {
        return Ok(0);
    }
    let text = format_task_interaction(interaction, task);
    let markup = interaction_markup(interaction);
    for target in &targets {
        send_text_with_markup(&token, target, &text, None, markup.as_ref()).await?;
    }
    Ok(targets.len())
}

fn format_task_interaction(
    interaction: &crate::db::interactions::TaskInteraction,
    task: &Task,
) -> String {
    let options = interaction_options(interaction);
    let kind = HitlKind::effective(&interaction.kind, &options, interaction.multiple);
    let short = short_id(&interaction.id);
    let mut message = format!(
        "[OpenDesk] Decisão necessária\nTarefa: {}\nStatus: {}\nTipo: {}\n\nPergunta: {}\n\nCódigo: #{short}",
        task.title,
        task.status,
        kind.as_str(),
        interaction.question
    );
    if let Some(context) = interaction
        .context_markdown
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        message.push_str("\n\nContexto:\n");
        message.push_str(&truncate(context, 1800));
    }
    match kind {
        HitlKind::Approval => {
            message.push_str(&format!(
                "\n\nAprove ou negue pelos botões, ou responda `#{short} aprovar <payload>` / `#{short} negar <payload>`."
            ));
        }
        HitlKind::SingleChoice | HitlKind::MultiChoice => {
            message.push_str("\n\nOpções:");
            for (index, option) in options.iter().enumerate() {
                message.push_str(&format!("\n{}. {}", index + 1, option));
            }
            if kind == HitlKind::MultiChoice {
                message.push_str(&format!(
                    "\n\nResponda com `#{short} 1,2` (separe as opções por vírgula) ou `#{short} negar`."
                ));
            } else {
                message.push_str(&format!(
                    "\n\nResponda com os botões ou `#{short} <número>`; use `#{short} negar` para recusar."
                ));
            }
            message.push_str(&format!("\nPayload opcional: `#{short} <número> <texto>`."));
        }
        HitlKind::Text => {
            message.push_str(&format!(
                "\n\nResponda com `#{short} <sua resposta>` ou `#{short} negar` para encerrar."
            ));
        }
    }
    message
}

fn interaction_markup(interaction: &crate::db::interactions::TaskInteraction) -> Option<Value> {
    let options = interaction_options(interaction);
    let kind = HitlKind::effective(&interaction.kind, &options, interaction.multiple);
    if kind == HitlKind::Approval {
        return Some(json!({
            "inline_keyboard": [[
                { "text": "Aprovar", "callback_data": format!("ti:{}:approve", interaction.id) },
                { "text": "Negar", "callback_data": format!("ti:{}:deny", interaction.id) },
            ]]
        }));
    }
    if interaction.multiple {
        return None;
    }
    if options.is_empty() {
        return None;
    }
    let mut rows = options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            json!([{
                "text": truncate(&option.replace(['\n', '\r'], " "), 48),
                "callback_data": format!("ti:{}:{index}", interaction.id),
            }])
        })
        .collect::<Vec<_>>();
    rows.push(json!([{
        "text": "Negar",
        "callback_data": format!("ti:{}:deny", interaction.id),
    }]));
    Some(json!({ "inline_keyboard": rows }))
}

/// Permission prompts can be approved or rejected directly from Telegram.
pub async fn notify_permission(
    pool: &sqlx::SqlitePool,
    secrets: &SecretStore,
    action: &str,
    resources: &[String],
    request_id: &str,
) -> AppResult<usize> {
    let config = load(pool).await?;
    if !config.enabled {
        return Ok(0);
    }
    let Some(token) = token(secrets) else {
        return Ok(0);
    };
    let targets = resolve_recipients(pool, &config.allowlist).await;
    if targets.is_empty() {
        return Ok(0);
    }
    let short = short_id(request_id);
    let text = format!(
        "OpenDesk aguardando decisão\nAção: {action}\nRecursos: {}\n\nAprove ou negue pelos botões, ou responda `APROVAR {short}` / `NEGAR {short}`.",
        resources.join(", ")
    );
    let markup = json!({
        "inline_keyboard": [[
            { "text": "Aprovar", "callback_data": format!("pm:{request_id}:once") },
            { "text": "Negar", "callback_data": format!("pm:{request_id}:reject") },
        ]]
    });
    for target in &targets {
        send_text_with_markup(&token, target, &text, None, Some(&markup)).await?;
    }
    Ok(targets.len())
}

struct TypingGuard {
    cancel: CancellationToken,
    handle: JoinHandle<()>,
}

impl TypingGuard {
    fn start(token: String, chat_id: String) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_interaction_replies_and_options() {
        assert_eq!(
            parse_interaction_reply("#1a2b3c4d 2"),
            Some(("1a2b3c4d", "2"))
        );
        assert_eq!(parse_interaction_reply("texto normal"), None);
        assert_eq!(parse_interaction_reply("#abc 1"), None);
        let options = vec!["Aprovar".to_string(), "Rejeitar".to_string()];
        assert_eq!(
            parse_selected_options("2", &options, false),
            vec!["Rejeitar".to_string()]
        );
        assert_eq!(
            parse_selected_options("1,2", &options, true),
            vec!["Aprovar".to_string(), "Rejeitar".to_string()]
        );
        assert!(parse_selected_options("9", &options, false).is_empty());
        assert_eq!(short_id("1234567890"), "12345678");
    }

    #[test]
    fn builds_inline_keyboards_for_single_choice_interactions() {
        let interaction = crate::db::interactions::TaskInteraction {
            id: "abcdef12-0000-0000-0000-000000000000".into(),
            workspace_id: "ws".into(),
            task_id: "task".into(),
            run_id: None,
            question: "Aprovar?".into(),
            context_markdown: None,
            options_json: r#"["Aprovar","Rejeitar"]"#.into(),
            kind: "SINGLE_CHOICE".into(),
            multiple: false,
            allow_custom: true,
            status: "PENDING".into(),
            answer_json: None,
            expires_at: "2999-01-01T00:00:00Z".into(),
            created_at: String::new(),
            resolved_at: None,
        };
        let markup = interaction_markup(&interaction).expect("markup");
        let rows = markup["inline_keyboard"].as_array().expect("rows");
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0][0]["callback_data"],
            "ti:abcdef12-0000-0000-0000-000000000000:0"
        );
        assert_eq!(
            rows[2][0]["callback_data"],
            "ti:abcdef12-0000-0000-0000-000000000000:deny"
        );
        assert!(rows[0][0]["callback_data"].as_str().unwrap().len() <= 64);

        let approval = crate::db::interactions::TaskInteraction {
            kind: "APPROVAL".into(),
            multiple: false,
            options_json: "[]".into(),
            ..interaction
        };
        let markup = interaction_markup(&approval).expect("approval markup");
        let rows = markup["inline_keyboard"].as_array().expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0][1]["callback_data"],
            "ti:abcdef12-0000-0000-0000-000000000000:deny"
        );
    }

    #[tokio::test]
    async fn resolves_task_interaction_callbacks() {
        let ctx = crate::test_support::context().await;
        sqlx::query(
            "INSERT INTO tasks (id, workspace_id, title, description)
             VALUES ('task-cb', ?, 'Revisar', 'Descrição')",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("task");
        let created = interactions::create(
            &ctx.state.db,
            interactions::CreateInteractionInput {
                workspace_id: ctx.workspace_id.clone(),
                task_id: "task-cb".into(),
                run_id: None,
                question: "Aprovar?".into(),
                context_markdown: None,
                options: vec!["Aprovar".into(), "Rejeitar".into()],
                kind: "SINGLE_CHOICE".into(),
                multiple: false,
                allow_custom: true,
                expires_at: "2999-01-01T00:00:00Z".into(),
            },
        )
        .await
        .expect("interaction");
        let callback = format!("{}:0", created.id);
        let message = resolve_interaction_callback(&ctx.state, &callback)
            .await
            .expect("resolve");
        assert!(message.contains("Resposta registrada"));
        let interaction = interactions::get(&ctx.state.db, &created.id)
            .await
            .expect("interaction");
        assert_eq!(interaction.status, "RESOLVED");
        assert!(
            interaction
                .answer_json
                .unwrap_or_default()
                .contains("Aprovar")
        );
    }

    #[tokio::test]
    async fn resolves_permission_callbacks() {
        let ctx = crate::test_support::context().await;
        sqlx::query(
            "INSERT INTO permission_requests
                (id, workspace_id, session_id, request_id, permission_action)
             VALUES ('perm-row', ?, 'ses-1', 'req-1', 'bash')",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("permission");

        let message = resolve_permission_callback(&ctx.state, "req-1:once")
            .await
            .expect("resolve");
        assert!(message.contains("aprovada"));
        let decision = crate::db::permissions::get_by_request(&ctx.state.db, "req-1")
            .await
            .expect("get")
            .decision;
        assert_eq!(decision.as_deref(), Some("once"));
    }
}
