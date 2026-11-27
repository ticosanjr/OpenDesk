//! Multi-bot supervisor: one independent bridge per workspace bot.
//!
//! Each handle owns its own cancellation token and, for Signal, its own
//! `signal-cli` process. A failure only changes the status of that bot.

use crate::{
    app_state::AppState,
    db::{channel_bots, settings},
    error::{AppError, AppResult},
    integrations::{
        signal::{self as signal_bridge, SignalBridge},
        telegram,
    },
};
use opendesk_channels::{
    signal::SignalConfig,
    telegram::{TelegramConfig, TelegramUpdate, delete_webhook, get_me, get_updates},
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;

struct BotHandle {
    channel: String,
    cancel: CancellationToken,
    task: JoinHandle<()>,
    signal: Option<Arc<SignalBridge>>,
}

/// Live registry of running bots. Persisted state stays in `channel_bots`.
#[derive(Default)]
pub struct BotSupervisor {
    handles: Mutex<HashMap<String, BotHandle>>,
}

impl BotSupervisor {
    pub async fn is_running(&self, bot_id: &str) -> bool {
        self.handles
            .lock()
            .await
            .get(bot_id)
            .map(|handle| !handle.task.is_finished())
            .unwrap_or(false)
    }

    pub async fn has_running_channel(&self, channel: &str) -> bool {
        self.handles
            .lock()
            .await
            .values()
            .any(|handle| handle.channel == channel && !handle.task.is_finished())
    }

    /// Seeds legacy global Telegram/Signal settings into `channel_bots` once.
    pub async fn seed_legacy(state: &Arc<AppState>) -> AppResult<()> {
        if channel_bots::count_bots(&state.db).await? > 0 {
            return Ok(());
        }
        let workspace_id = settings::active_workspace_id(&state.db)
            .await?
            .unwrap_or_else(|| "default".into());

        let telegram_config = telegram::load(&state.db).await?;
        if telegram_config.enabled
            && let Some(token) = telegram::token(&state.secrets)
            && !telegram_config.allowlist.is_empty()
        {
            let workspace = telegram_config
                .workspace_id
                .clone()
                .unwrap_or(workspace_id.clone());
            let bot = channel_bots::create_bot(
                &state.db,
                &workspace,
                channel_bots::ChannelBotInput {
                    channel: "telegram".into(),
                    name: "Telegram principal".into(),
                    enabled: true,
                    allowlist: telegram_config.allowlist.clone(),
                    default_agent: Some(telegram_config.agent.clone()),
                    ..Default::default()
                },
                Some(String::new()),
            )
            .await?;
            state
                .secrets
                .set("channel_bot", &bot.id, &token)
                .map_err(AppError::from)?;
            channel_bots::set_secret_ref(&state.db, &bot.id, Some(&bot.id)).await?;
        }

        let signal_config = signal_bridge::load(&state.db).await?;
        if signal_config.enabled && !signal_config.account.trim().is_empty() {
            let workspace = signal_config.workspace_id.clone().unwrap_or(workspace_id);
            let config = serde_json::to_string(&signal_config)
                .map_err(|error| AppError::Internal(error.to_string()))?;
            channel_bots::create_bot(
                &state.db,
                &workspace,
                channel_bots::ChannelBotInput {
                    channel: "signal".into(),
                    name: "Signal principal".into(),
                    enabled: true,
                    config_json: Some(config),
                    allowlist: signal_config.allowlist.clone(),
                    default_agent: Some(signal_config.agent.clone()),
                    ..Default::default()
                },
                None,
            )
            .await?;
        }
        Ok(())
    }

    pub async fn start_all(self: &Arc<Self>, state: Arc<AppState>) {
        if let Err(error) = Self::seed_legacy(&state).await {
            tracing::warn!(error = %error, "legacy channel bots were not seeded");
        }
        let bots = match channel_bots::all_enabled(&state.db).await {
            Ok(bots) => bots,
            Err(error) => {
                tracing::warn!(error = %error, "could not list enabled channel bots");
                return;
            }
        };
        for bot in bots {
            if let Err(error) = self.start(state.clone(), bot.clone()).await {
                tracing::warn!(bot_id = %bot.id, error = %error, "channel bot was not started");
            }
        }
    }

    pub async fn start(
        self: &Arc<Self>,
        state: Arc<AppState>,
        mut bot: channel_bots::ChannelBot,
    ) -> AppResult<()> {
        self.stop(&bot.id).await;
        bot.enabled = true;
        channel_bots::set_enabled(&state.db, &bot.id, true).await?;
        match bot.channel.as_str() {
            "telegram" => self.start_telegram(state, bot).await,
            "signal" => self.start_signal(state, bot).await,
            other => Err(AppError::BadRequest(format!(
                "canal {other} ainda não suporta múltiplos bots"
            ))),
        }
    }

    async fn start_telegram(
        self: &Arc<Self>,
        state: Arc<AppState>,
        bot: channel_bots::ChannelBot,
    ) -> AppResult<()> {
        let token = match state
            .secrets
            .get("channel_bot", &bot.id)?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            Some(token) => token,
            None => {
                let message = "token do bot não configurado";
                channel_bots::set_status(&state.db, &bot.id, "error", Some(message)).await?;
                return Err(AppError::BadRequest(message.into()));
            }
        };
        if !telegram::valid_token_shape(&token) {
            let message = "token do Telegram inválido; use o formato 123456:ABC…";
            channel_bots::set_status(&state.db, &bot.id, "error", Some(message)).await?;
            return Err(AppError::BadRequest(message.into()));
        }
        let allowlist = parse_allowlist(&bot.allowlist_json)?;
        if allowlist.is_empty() {
            let message = "a allowlist do bot está vazia";
            channel_bots::set_status(&state.db, &bot.id, "error", Some(message)).await?;
            return Err(AppError::BadRequest(message.into()));
        }
        if let Err(error) = get_me(&token).await {
            let message = format!("token do Telegram inválido: {error}");
            channel_bots::set_status(&state.db, &bot.id, "error", Some(&message)).await?;
            return Err(AppError::BadRequest(message));
        }
        if let Err(error) = delete_webhook(&token).await {
            let message = format!("não foi possível liberar o long polling: {error}");
            channel_bots::set_status(&state.db, &bot.id, "error", Some(&message)).await?;
            return Err(AppError::Daemon(message));
        }

        let cancel = CancellationToken::new();
        let task = tokio::spawn(run_telegram(
            state.clone(),
            bot.clone(),
            token,
            cancel.clone(),
        ));
        self.handles.lock().await.insert(
            bot.id.clone(),
            BotHandle {
                channel: "telegram".into(),
                cancel,
                task,
                signal: None,
            },
        );
        channel_bots::set_status(&state.db, &bot.id, "running", None).await?;
        Ok(())
    }

    async fn start_signal(
        self: &Arc<Self>,
        state: Arc<AppState>,
        bot: channel_bots::ChannelBot,
    ) -> AppResult<()> {
        let mut config = serde_json::from_str::<SignalConfig>(&bot.config_json)
            .unwrap_or_else(|_| SignalConfig::default());
        config.enabled = true;
        config.workspace_id = Some(bot.workspace_id.clone());
        config.agent = bot.default_agent.clone();
        config.allowlist = parse_allowlist(&bot.allowlist_json)?;
        let bridge = Arc::new(SignalBridge::default());
        match bridge
            .start_for_bot(state.clone(), bot.clone(), config)
            .await
        {
            Ok(_) => {
                let cancel = CancellationToken::new();
                let task = tokio::spawn({
                    let cancel = cancel.clone();
                    async move {
                        cancel.cancelled().await;
                    }
                });
                self.handles.lock().await.insert(
                    bot.id.clone(),
                    BotHandle {
                        channel: "signal".into(),
                        cancel,
                        task,
                        signal: Some(bridge),
                    },
                );
                channel_bots::set_status(&state.db, &bot.id, "running", None).await?;
                Ok(())
            }
            Err(error) => {
                channel_bots::set_status(&state.db, &bot.id, "error", Some(&error.to_string()))
                    .await?;
                Err(error)
            }
        }
    }

    pub async fn stop(&self, bot_id: &str) {
        let handle = self.handles.lock().await.remove(bot_id);
        if let Some(handle) = handle {
            handle.cancel.cancel();
            if let Some(signal) = handle.signal {
                signal.stop().await;
            }
            handle.task.abort();
        }
    }

    pub async fn stop_all(&self) {
        let handles = std::mem::take(&mut *self.handles.lock().await);
        for (_, handle) in handles {
            handle.cancel.cancel();
            if let Some(signal) = handle.signal {
                signal.stop().await;
            }
            handle.task.abort();
        }
    }
}

async fn run_telegram(
    state: Arc<AppState>,
    bot: channel_bots::ChannelBot,
    token: String,
    cancel: CancellationToken,
) {
    let config = TelegramConfig {
        enabled: true,
        allowlist: parse_allowlist(&bot.allowlist_json).unwrap_or_default(),
        workspace_id: Some(bot.workspace_id.clone()),
        agent: bot.default_agent.clone(),
    };
    let offset_key = format!("telegram_bot_offset:{}", bot.id);
    let mut offset = settings::get(&state.db, &offset_key)
        .await
        .ok()
        .flatten()
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let mut backoff = 2u64;
    loop {
        let result = tokio::select! {
            _ = cancel.cancelled() => return,
            result = get_updates(&token, offset) => result,
        };
        match result {
            Ok(updates) => {
                backoff = 2;
                let _ = channel_bots::set_status(&state.db, &bot.id, "running", None).await;
                for update in updates {
                    let update_id = match &update {
                        TelegramUpdate::Message(message) => message.update_id,
                        TelegramUpdate::Callback(callback) => callback.update_id,
                    };
                    offset = offset.max(update_id + 1);
                    if let Err(error) =
                        telegram::handle_bot_update(&state, &bot, &config, &token, update).await
                    {
                        tracing::warn!(bot_id = %bot.id, error = %error, "Telegram bot update failed");
                    }
                    let _ = settings::set(&state.db, &offset_key, &json!(offset)).await;
                }
            }
            Err(error) => {
                let _ =
                    channel_bots::set_status(&state.db, &bot.id, "error", Some(&error.to_string()))
                        .await;
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                }
                backoff = (backoff * 2).min(60);
            }
        }
    }
}

fn parse_allowlist(raw: &str) -> AppResult<Vec<String>> {
    serde_json::from_str(raw)
        .map_err(|error| AppError::BadRequest(format!("allowlist inválida: {error}")))
}
