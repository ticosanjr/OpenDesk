use crate::{
    app_state::AppState,
    chat,
    db::{chat as chat_db, settings},
    error::{AppError, AppResult},
    secrets::SecretStore,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::sync::Mutex;

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
    pub chat_id: String,
    pub username: Option<String>,
    pub chat_title: Option<String>,
    pub text: String,
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
                        offset = offset.max(update.update_id + 1);
                        let config = load(&state.db).await?;
                        if let Err(error) = handle_incoming(&state, &config, &token, update).await {
                            tracing::warn!(error = %error, "Telegram message failed");
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
        send_text(token, &incoming.chat_id, &message).await?;
        return Ok(());
    }

    let workspace_id = match &config.workspace_id {
        Some(id) => id.clone(),
        None => settings::active_workspace_id(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("no active workspace for Telegram".into()))?,
    };
    let label = incoming
        .chat_title
        .clone()
        .or_else(|| incoming.username.clone())
        .unwrap_or_else(|| incoming.chat_id.clone());
    let thread =
        match chat_db::find_external(&state.db, &workspace_id, "telegram", &incoming.chat_id)
            .await?
        {
            Some(thread) => thread,
            None => {
                chat_db::create_thread(
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
                .await?
            }
        };

    match chat::send_message(state, thread, trimmed).await {
        Ok(reply) => {
            send_text(token, &incoming.chat_id, &reply.assistant.content).await?;
        }
        Err(error) => {
            let message = format!("Não consegui responder agora: {error}");
            let _ = send_text(token, &incoming.chat_id, &message).await;
            return Err(error);
        }
    }
    Ok(())
}

fn help_text(agent: &str) -> String {
    format!(
        "OpenDesk conectado.\n\n\
         Envie uma mensagem para conversar com @{agent}.\n\
         Comandos:\n\
         /help — esta ajuda\n\
         aprovar <prefixo> — aprova uma permissão pendente\n\
         negar <prefixo> — nega uma permissão pendente\n\
         Para aprovações de receita, responda com o token OD-… e APROVAR/REJEITAR."
    )
}

pub fn parse_updates(value: &Value) -> Vec<IncomingTelegram> {
    let items = value
        .as_array()
        .or_else(|| value.get("result").and_then(Value::as_array));
    let Some(items) = items else {
        return Vec::new();
    };
    items.iter().filter_map(parse_update).collect()
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
        chat_id,
        username,
        chat_title,
        text,
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
    let text = if text.trim().is_empty() {
        "(sem resposta)"
    } else {
        text
    };
    for chunk in chunks(text, MAX_MESSAGE_CHARS) {
        send_chunk(token, chat_id, &chunk).await?;
    }
    Ok(())
}

async fn send_chunk(token: &str, chat_id: &str, text: &str) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "sendMessage"))
        .json(&json!({
            "chat_id": chat_id,
            "text": text,
            "disable_web_page_preview": true,
        }))
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

async fn get_updates(token: &str, offset: i64) -> AppResult<Vec<IncomingTelegram>> {
    let response = client()
        .post(api_url(token, "getUpdates"))
        .json(&json!({
            "offset": offset,
            "timeout": 25,
            "allowed_updates": ["message"],
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
        assert_eq!(updates[0].chat_id, "42");
        assert_eq!(updates[0].text, "olá");
        assert_eq!(updates[1].chat_id, "-100123");
        assert_eq!(updates[1].chat_title.as_deref(), Some("Equipe"));
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
    fn token_shape_rejects_obvious_paste_errors() {
        assert!(valid_token_shape("123456:abcdefghijklmnopqrstuvwxyz"));
        assert!(!valid_token_shape("not-a-token"));
        assert!(!valid_token_shape("123456:short"));
        assert!(!valid_token_shape("abc:abcdefghijklmnopqrstuvwxyz"));
    }
}
