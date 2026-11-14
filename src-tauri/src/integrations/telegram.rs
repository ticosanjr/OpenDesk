use crate::{
    app_state::AppState,
    chat,
    db::{chat as chat_db, interactions, settings, tasks::Task},
    error::{AppError, AppResult},
    secrets::SecretStore,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;

const SETTING_KEY: &str = "telegram_config";
const TOKEN_NAMESPACE: &str = "telegram";
const TOKEN_KEY: &str = "bot_token";
const OFFSET_KEY: &str = "telegram_last_update_id";
const CHATS_KEY: &str = "telegram_chats";
const MAX_MESSAGE_CHARS: usize = 3800;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default = "default_agent")]
    pub agent: String,
}

fn default_agent() -> String {
    "build".into()
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowlist: Vec::new(),
            workspace_id: None,
            agent: default_agent(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TelegramStatus {
    pub config: TelegramConfig,
    pub token_configured: bool,
    pub bot_username: Option<String>,
    pub bot_name: Option<String>,
    pub bridge_running: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInfo {
    pub id: i64,
    pub is_bot: bool,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownChat {
    pub chat_id: String,
    pub username: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTelegram {
    pub update_id: i64,
    pub message_id: i64,
    pub chat_id: String,
    pub username: Option<String>,
    pub chat_title: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCallback {
    pub update_id: i64,
    pub callback_id: String,
    pub chat_id: String,
    pub username: Option<String>,
    pub message_id: i64,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramUpdate {
    Message(IncomingTelegram),
    Callback(IncomingCallback),
}

#[derive(Default)]
pub struct TelegramBridge {
    handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    last_error: Mutex<Option<String>>,
    bot: Mutex<Option<BotInfo>>,
}

pub async fn load(pool: &sqlx::SqlitePool) -> AppResult<TelegramConfig> {
    let value = settings::get(pool, SETTING_KEY).await?;
    Ok(value
        .and_then(|value| serde_json::from_value::<TelegramConfig>(value).ok())
        .unwrap_or_default())
}

pub async fn save(pool: &sqlx::SqlitePool, config: &TelegramConfig) -> AppResult<()> {
    settings::set(pool, SETTING_KEY, &serde_json::to_value(config)?).await
}

pub fn token(secrets: &SecretStore) -> Option<String> {
    secrets
        .get(TOKEN_NAMESPACE, TOKEN_KEY)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub async fn token_for(state: &AppState) -> AppResult<String> {
    token(&state.secrets).ok_or_else(|| {
        AppError::BadRequest("Telegram não está configurado: informe o token do bot.".into())
    })
}

/// Accepts the canonical `<bot_id>:<secret>` shape and rejects obvious paste
/// mistakes without being stricter than Telegram itself.
pub fn valid_token_shape(value: &str) -> bool {
    let value = value.trim();
    let Some((id, secret)) = value.split_once(':') else {
        return false;
    };
    id.chars().all(|character| character.is_ascii_digit())
        && !id.is_empty()
        && secret.len() >= 20
        && secret
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
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
        let options = interaction_options(&interaction);
        let selected = parse_selected_options(answer, &options, interaction.multiple);
        let text = if selected.is_empty() && interaction.allow_custom {
            Some(answer.trim().to_string()).filter(|value| !value.is_empty())
        } else {
            None
        };
        if selected.is_empty() && text.is_none() {
            send_reply(
                token,
                &incoming.chat_id,
                incoming.message_id,
                &format!(
                    "Responda com o código e uma opção válida: #{} 1",
                    short_id(&interaction.id)
                ),
            )
            .await?;
            return Ok(());
        }
        crate::api::interactions::resolve_internal(state, &interaction.id, selected, text).await?;
        send_reply(
            token,
            &incoming.chat_id,
            incoming.message_id,
            "Resposta registrada. A tarefa foi retomada.",
        )
        .await?;
        return Ok(());
    }

    if crate::recipes::hitl::telegram::handle_reply(state, &incoming.chat_id, trimmed, &[]).await? {
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

    let context = telegram_context(&thread.workspace_id);
    let typing = TypingGuard::start(token.to_string(), incoming.chat_id.clone());
    let result = chat::send_message_with_context(state, thread, trimmed, Some(&context)).await;
    drop(typing);
    match result {
        Ok(reply) => {
            send_reply(
                token,
                &incoming.chat_id,
                incoming.message_id,
                &reply.assistant.content,
            )
            .await?;
        }
        Err(error) => {
            let message = format!("Não consegui responder agora: {error}");
            let _ = send_reply(token, &incoming.chat_id, incoming.message_id, &message).await;
            return Err(error);
        }
    }
    Ok(())
}

fn telegram_context(workspace_id: &str) -> String {
    format!(
        "Contexto do canal Telegram (não repita ao usuário):\n         Você está operando o OpenDesk no workspace `{workspace_id}`.\n         Use as ferramentas MCP `opendesk-control` quando o pedido envolver o OpenDesk:\n         - tarefas: task_create, task_list, task_update, task_move, task_dispatch;\n         - execuções e estado: run_list, opendesk_status, attention_list;\n         - rotinas e receitas: routine_list, routine_run, recipe_list, recipe_run;\n         - fila, artefatos e resultados: queue_control, artifact_list, artifact_read;\n         - decisão humana: ask_user (a pergunta será enviada ao Telegram).\n         Consulte resultados de tarefas com artifact_list e artifact_read, e leia os\n         arquivos do workspace quando precisar do conteúdo completo.\n         Nunca ative rotina/receita nem aprove HITL/permissão sem pedido explícito.\n         Responda em português, de forma curta, citando IDs e caminhos relevantes."
    )
}

/// Parses `#abcdef12 2` into (short code, answer). The code is at least four
/// characters so it cannot collide with ordinary hashtags.
pub fn parse_interaction_reply(text: &str) -> Option<(&str, &str)> {
    let rest = text.trim().strip_prefix('#')?;
    let (short, answer) = rest.split_once(char::is_whitespace)?;
    let short = short.trim();
    if short.len() < 4 || !short.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return None;
    }
    Some((short, answer.trim()))
}

pub fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn interaction_options(interaction: &crate::db::interactions::TaskInteraction) -> Vec<String> {
    serde_json::from_str(&interaction.options_json).unwrap_or_default()
}

fn parse_selected_options(answer: &str, options: &[String], multiple: bool) -> Vec<String> {
    if options.is_empty() {
        return Vec::new();
    }
    let mut selected = Vec::new();
    for part in answer.split([',', ' ']).filter(|part| !part.is_empty()) {
        let Ok(index) = part.parse::<usize>() else {
            return Vec::new();
        };
        if index == 0 || index > options.len() {
            return Vec::new();
        }
        selected.push(options[index - 1].clone());
        if !multiple {
            break;
        }
    }
    selected
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
    let (id, index) = rest
        .rsplit_once(':')
        .ok_or_else(|| AppError::BadRequest("callback de interação inválido".into()))?;
    let interaction = interactions::get(&state.db, id).await?;
    if interaction.status != "PENDING" {
        return Ok("Esta pergunta já foi respondida ou expirou.".into());
    }
    let options = interaction_options(&interaction);
    let index = index
        .parse::<usize>()
        .ok()
        .filter(|index| *index < options.len())
        .ok_or_else(|| AppError::BadRequest("opção inexistente".into()))?;
    crate::api::interactions::resolve_internal(
        state,
        &interaction.id,
        vec![options[index].clone()],
        None,
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
    let short = short_id(&interaction.id);
    let mut message = format!(
        "[OpenDesk] Decisão necessária\nTarefa: {}\nStatus: {}\n\nPergunta: {}\n\nCódigo: #{short}",
        task.title, task.status, interaction.question
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
    if !options.is_empty() {
        message.push_str("\n\nOpções:");
        for (index, option) in options.iter().enumerate() {
            message.push_str(&format!("\n{}. {}", index + 1, option));
        }
        if interaction.multiple {
            message.push_str(&format!(
                "\n\nResponda com `#{short} 1,2` (separe as opções por vírgula)."
            ));
        } else {
            message.push_str(&format!(
                "\n\nResponda com os botões ou `#{short} <número>`."
            ));
        }
        if interaction.allow_custom {
            message.push_str(&format!("\nOu escreva `#{short} <texto livre>`."));
        }
    } else {
        message.push_str(&format!("\n\nResponda com `#{short} <sua resposta>`."));
    }
    message
}

fn interaction_markup(interaction: &crate::db::interactions::TaskInteraction) -> Option<Value> {
    if interaction.multiple {
        return None;
    }
    let options = interaction_options(interaction);
    if options.is_empty() {
        return None;
    }
    let rows = options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            json!([{
                "text": truncate(&option.replace(['\n', '\r'], " "), 48),
                "callback_data": format!("ti:{}:{index}", interaction.id),
            }])
        })
        .collect::<Vec<_>>();
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

fn truncate(value: &str, limit: usize) -> String {
    let mut output = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        output.push('…');
    }
    output
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

async fn send_chat_action(token: &str, chat_id: &str, action: &str) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "sendChatAction"))
        .json(&json!({ "chat_id": chat_id, "action": action }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede no typing".into()))?;
    parse_response("sendChatAction", response).await.map(|_| ())
}

async fn answer_callback_query(token: &str, callback_id: &str, text: &str) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "answerCallbackQuery"))
        .json(&json!({ "callback_query_id": callback_id, "text": text }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha ao responder callback".into()))?;
    parse_response("answerCallbackQuery", response)
        .await
        .map(|_| ())
}

async fn clear_buttons(token: &str, chat_id: &str, message_id: i64) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "editMessageReplyMarkup"))
        .json(&json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "reply_markup": { "inline_keyboard": [] },
        }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha ao limpar botões".into()))?;
    parse_response("editMessageReplyMarkup", response)
        .await
        .map(|_| ())
}

fn help_text(agent: &str) -> String {
    format!(
        "OpenDesk conectado.\n\n\
         Converse com @{agent} em linguagem natural. Exemplos:\n\
         - crie uma tarefa para revisar o relatório de outubro;\n\
         - mostre o status das execuções e da fila;\n\
         - execute a rotina <id> ou a receita <id>;\n\
         - qual foi o resultado da tarefa <id>?\n\n\
         Comandos:\n\
         /help — esta ajuda\n\
         aprovar <prefixo> — aprova uma permissão pendente\n\
         negar <prefixo> — nega uma permissão pendente\n\
         #<código> <opção> — responde uma pergunta HITL de tarefa\n\
         Para aprovações de receita, responda com o token OD-… e APROVAR/REJEITAR."
    )
}

pub fn parse_updates(value: &Value) -> Vec<TelegramUpdate> {
    let items = value
        .as_array()
        .or_else(|| value.get("result").and_then(Value::as_array));
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if let Some(message) = parse_update(item) {
                Some(TelegramUpdate::Message(message))
            } else {
                parse_callback_update(item).map(TelegramUpdate::Callback)
            }
        })
        .collect()
}

pub fn parse_update(value: &Value) -> Option<IncomingTelegram> {
    let update_id = value.get("update_id")?.as_i64()?;
    let message = value.get("message")?;
    let from = message.get("from");
    if from
        .and_then(|from| from.get("is_bot"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let message_id = message.get("message_id")?.as_i64()?;
    let chat = message.get("chat")?;
    let chat_id = chat.get("id")?.as_i64()?.to_string();
    let username = from
        .and_then(|from| from.get("username"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let chat_title = chat
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string);
    let text = message
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())?
        .to_string();
    Some(IncomingTelegram {
        update_id,
        message_id,
        chat_id,
        username,
        chat_title,
        text,
    })
}

pub fn parse_callback_update(value: &Value) -> Option<IncomingCallback> {
    let update_id = value.get("update_id")?.as_i64()?;
    let callback = value.get("callback_query")?;
    let callback_id = callback.get("id")?.as_str()?.to_string();
    let from = callback.get("from");
    if from
        .and_then(|from| from.get("is_bot"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let message = callback.get("message")?;
    let message_id = message.get("message_id")?.as_i64()?;
    let chat = message.get("chat")?;
    let chat_id = chat.get("id")?.as_i64()?.to_string();
    let username = from
        .and_then(|from| from.get("username"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let data = callback.get("data")?.as_str()?.to_string();
    Some(IncomingCallback {
        update_id,
        callback_id,
        chat_id,
        username,
        message_id,
        data,
    })
}

pub fn allowed(allowlist: &[String], chat_id: &str, username: Option<&str>) -> bool {
    let chat_id = chat_id.trim();
    let username = username.map(normalize_username);
    allowlist.iter().any(|entry| {
        let entry = entry.trim();
        if entry.is_empty() {
            return false;
        }
        if is_chat_id(entry) {
            entry == chat_id
        } else {
            username
                .as_deref()
                .map(|username| normalize_username(entry) == username)
                .unwrap_or(false)
        }
    })
}

fn is_chat_id(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
}

fn normalize_username(value: &str) -> String {
    value.trim().trim_start_matches('@').to_lowercase()
}

pub fn chunks(text: &str, limit: usize) -> Vec<String> {
    let limit = limit.max(200);
    if text.chars().count() <= limit {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if current.chars().count() + line.chars().count() + 1 > limit && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        if line.chars().count() > limit {
            for piece in split_hard(line, limit) {
                if !current.is_empty() {
                    chunks.push(std::mem::take(&mut current));
                }
                chunks.push(piece);
            }
            continue;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    if chunks.is_empty() {
        chunks.push(String::new());
    }
    chunks
}

fn split_hard(value: &str, limit: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if current.chars().count() >= limit {
            parts.push(std::mem::take(&mut current));
        }
        current.push(character);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

async fn record_chat(
    pool: &sqlx::SqlitePool,
    chat_id: &str,
    username: Option<&str>,
    title: Option<&str>,
) -> AppResult<()> {
    let mut chats = settings::get(pool, CHATS_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value::<Vec<KnownChat>>(value).ok())
        .unwrap_or_default();
    if let Some(chat) = chats.iter_mut().find(|chat| chat.chat_id == chat_id) {
        chat.username = username.map(str::to_string).or(chat.username.clone());
        chat.title = title.map(str::to_string).or(chat.title.clone());
    } else {
        chats.push(KnownChat {
            chat_id: chat_id.to_string(),
            username: username.map(str::to_string),
            title: title.map(str::to_string),
        });
    }
    settings::set(pool, CHATS_KEY, &serde_json::to_value(chats)?).await
}

pub async fn known_chats(pool: &sqlx::SqlitePool) -> Vec<KnownChat> {
    settings::get(pool, CHATS_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value::<Vec<KnownChat>>(value).ok())
        .unwrap_or_default()
}

/// Resolve numeric chat IDs and `@username` entries against chats the bot has
/// already seen. Telegram cannot send to a private username before the user
/// messages the bot.
pub async fn resolve_recipients(pool: &sqlx::SqlitePool, recipients: &[String]) -> Vec<String> {
    let chats = known_chats(pool).await;
    let mut seen = HashSet::new();
    let mut resolved = Vec::new();
    for entry in recipients {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let chat_id = if is_chat_id(entry) {
            Some(entry.to_string())
        } else {
            let username = normalize_username(entry);
            chats
                .iter()
                .find(|chat| {
                    chat.username
                        .as_deref()
                        .map(|value| normalize_username(value) == username)
                        .unwrap_or(false)
                })
                .map(|chat| chat.chat_id.clone())
        };
        if let Some(chat_id) = chat_id
            && seen.insert(chat_id.clone())
        {
            resolved.push(chat_id);
        }
    }
    resolved
}

pub async fn send_text(token: &str, chat_id: &str, text: &str) -> AppResult<()> {
    send_text_with_markup(token, chat_id, text, None, None).await
}

pub async fn send_reply(token: &str, chat_id: &str, message_id: i64, text: &str) -> AppResult<()> {
    send_text_with_markup(token, chat_id, text, Some(message_id), None).await
}

async fn send_text_with_markup(
    token: &str,
    chat_id: &str,
    text: &str,
    reply_to: Option<i64>,
    markup: Option<&Value>,
) -> AppResult<()> {
    let text = if text.trim().is_empty() {
        "(sem resposta)"
    } else {
        text
    };
    for (index, chunk) in chunks(text, MAX_MESSAGE_CHARS).into_iter().enumerate() {
        send_chunk(
            token,
            chat_id,
            &chunk,
            if index == 0 { reply_to } else { None },
            if index == 0 { markup } else { None },
        )
        .await?;
    }
    Ok(())
}

async fn send_chunk(
    token: &str,
    chat_id: &str,
    text: &str,
    reply_to: Option<i64>,
    markup: Option<&Value>,
) -> AppResult<()> {
    let mut payload = json!({
        "chat_id": chat_id,
        "text": text,
        "disable_web_page_preview": true,
    });
    if let Some(message_id) = reply_to {
        payload["reply_parameters"] = json!({
            "message_id": message_id,
            "allow_sending_without_reply": true,
        });
    }
    if let Some(markup) = markup {
        payload["reply_markup"] = markup.clone();
    }
    let response = client()
        .post(api_url(token, "sendMessage"))
        .json(&payload)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede ao enviar mensagem".into()))?;
    parse_response("sendMessage", response).await.map(|_| ())
}

/// Send to every allowlist target that already has a numeric chat ID.
pub async fn send_to_allowlist(state: &AppState, text: &str) -> AppResult<usize> {
    let config = load(&state.db).await?;
    if !config.enabled {
        return Err(AppError::BadRequest("Telegram não está habilitado".into()));
    }
    let token = token_for(state).await?;
    let targets = resolve_recipients(&state.db, &config.allowlist).await;
    if targets.is_empty() {
        return Err(AppError::BadRequest(
            "a allowlist do Telegram não tem chats conhecidos; envie /start para o bot primeiro"
                .into(),
        ));
    }
    for target in &targets {
        send_text(&token, target, text).await?;
    }
    Ok(targets.len())
}

pub async fn get_me(token: &str) -> AppResult<BotInfo> {
    let response = client()
        .get(api_url(token, "getMe"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede ao validar o token".into()))?;
    let value = parse_response("getMe", response).await?;
    serde_json::from_value(value).map_err(|error| {
        AppError::Daemon(format!("Telegram getMe retornou dados inválidos: {error}"))
    })
}

async fn delete_webhook(token: &str) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "deleteWebhook"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede ao liberar o polling".into()))?;
    parse_response("deleteWebhook", response).await.map(|_| ())
}

async fn get_updates(token: &str, offset: i64) -> AppResult<Vec<TelegramUpdate>> {
    let response = client()
        .post(api_url(token, "getUpdates"))
        .json(&json!({
            "offset": offset,
            "timeout": 25,
            "allowed_updates": ["message", "callback_query"],
        }))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede no long polling".into()))?;
    let value = parse_response("getUpdates", response).await?;
    Ok(parse_updates(&value))
}

async fn parse_response(method: &str, response: reqwest::Response) -> AppResult<Value> {
    let status = response.status();
    let value: Value = response
        .json()
        .await
        .map_err(|_| AppError::Daemon(format!("Telegram {method}: resposta inválida")))?;
    if !status.is_success() || value.get("ok").and_then(Value::as_bool) != Some(true) {
        let description = value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("erro desconhecido");
        return Err(AppError::Daemon(format!(
            "Telegram {method} falhou ({status}): {description}"
        )));
    }
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

fn api_url(token: &str, method: &str) -> String {
    format!("https://api.telegram.org/bot{token}/{method}")
}

fn client() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(45))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Telegram HTTP client")
        })
        .clone()
}

pub async fn clear_offset(pool: &sqlx::SqlitePool) {
    let _ = sqlx::query("DELETE FROM app_settings WHERE key = ?")
        .bind(OFFSET_KEY)
        .execute(pool)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_private_and_group_updates() {
        let value = json!({
            "ok": true,
            "result": [
                {
                    "update_id": 10,
                    "message": {
                        "message_id": 1,
                        "from": { "id": 42, "is_bot": false, "username": "rafa" },
                        "chat": { "id": 42, "type": "private" },
                        "text": "  olá  "
                    }
                },
                {
                    "update_id": 11,
                    "message": {
                        "message_id": 2,
                        "from": { "id": 7, "is_bot": true },
                        "chat": { "id": 7, "type": "private" },
                        "text": "bot"
                    }
                },
                {
                    "update_id": 12,
                    "message": {
                        "message_id": 3,
                        "from": { "id": 42, "is_bot": false },
                        "chat": { "id": -100123, "type": "group", "title": "Equipe" },
                        "text": "/help"
                    }
                }
            ]
        });
        let updates = parse_updates(&value);
        assert_eq!(updates.len(), 2);
        assert_eq!(parse_updates(&value["result"]).len(), 2);
        let TelegramUpdate::Message(first) = &updates[0] else {
            panic!("expected message");
        };
        let TelegramUpdate::Message(second) = &updates[1] else {
            panic!("expected message");
        };
        assert_eq!(first.message_id, 1);
        assert_eq!(first.chat_id, "42");
        assert_eq!(first.text, "olá");
        assert_eq!(second.message_id, 3);
        assert_eq!(second.chat_id, "-100123");
        assert_eq!(second.chat_title.as_deref(), Some("Equipe"));
    }

    #[test]
    fn parses_callback_queries() {
        let value = json!({
            "update_id": 20,
            "callback_query": {
                "id": "cb-1",
                "from": { "id": 42, "is_bot": false, "username": "rafa" },
                "message": {
                    "message_id": 9,
                    "chat": { "id": 42, "type": "private" }
                },
                "data": "ti:abc:1"
            }
        });
        let callback = parse_callback_update(&value).expect("callback");
        assert_eq!(callback.callback_id, "cb-1");
        assert_eq!(callback.chat_id, "42");
        assert_eq!(callback.message_id, 9);
        assert_eq!(callback.data, "ti:abc:1");
    }

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
    fn ignores_messages_without_text() {
        let value = json!({
            "update_id": 1,
            "message": {
                "from": { "id": 1, "is_bot": false },
                "chat": { "id": 1, "type": "private" },
                "photo": []
            }
        });
        assert!(parse_update(&value).is_none());
    }

    #[test]
    fn allowlist_accepts_numeric_ids_and_usernames() {
        let allowlist = vec!["42".into(), "-100123".into(), "@Rafa".into()];
        assert!(allowed(&allowlist, "42", Some("other")));
        assert!(allowed(&allowlist, "-100123", None));
        assert!(allowed(&allowlist, "99", Some("rafa")));
        assert!(!allowed(&allowlist, "99", Some("someone")));
        assert!(!allowed(&[], "42", Some("rafa")));
    }

    #[test]
    fn chunks_long_messages_at_line_boundaries() {
        let text = (1..=200)
            .map(|index| format!("linha {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let parts = chunks(&text, 200);
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|part| part.chars().count() <= 200));
        assert_eq!(parts.concat().replace('\n', ""), text.replace('\n', ""));
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
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0][0]["callback_data"],
            "ti:abcdef12-0000-0000-0000-000000000000:0"
        );
        assert!(rows[0][0]["callback_data"].as_str().unwrap().len() <= 64);
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
        sqlx::query(
            "INSERT INTO task_interactions
                (id, workspace_id, task_id, question, options_json, expires_at)
             VALUES ('cb-abcdef12-0000-0000-0000-000000000000', ?, 'task-cb', 'Aprovar?', '[\"Aprovar\",\"Rejeitar\"]', '2999-01-01T00:00:00Z')",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("interaction");

        let message =
            resolve_interaction_callback(&ctx.state, "cb-abcdef12-0000-0000-0000-000000000000:0")
                .await
                .expect("resolve");
        assert!(message.contains("Resposta registrada"));
        let interaction =
            interactions::get(&ctx.state.db, "cb-abcdef12-0000-0000-0000-000000000000")
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

    #[test]
    fn token_shape_rejects_obvious_paste_errors() {
        assert!(valid_token_shape("123456:abcdefghijklmnopqrstuvwxyz"));
        assert!(!valid_token_shape("not-a-token"));
        assert!(!valid_token_shape("123456:short"));
        assert!(!valid_token_shape("abc:abcdefghijklmnopqrstuvwxyz"));
    }
}
