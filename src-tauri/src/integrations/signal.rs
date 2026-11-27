//! Signal host orchestration.
//!
//! Transport, protocol parsing and `signal-cli` process management live in
//! `opendesk-channels::signal`. This module keeps only the callbacks that need
//! `Arc<AppState>`: status reporting, inbound routing (routines, HITL, chat)
//! and task interaction notifications.

use crate::{
    app_state::AppState,
    chat,
    db::{channel_bots, chat as chat_db, interactions, settings, tasks::Task},
    error::{AppError, AppResult},
    hitl::HitlKind,
};
use opendesk_channels::signal::{SignalBridge as SignalBridgeInner, SignalEventHandler};
use std::{future::Future, pin::Pin, sync::Arc};

// Host code keeps calling `crate::integrations::signal::X`; the modules below
// re-export the channels implementation verbatim. `allowed`, `parse_incoming`
// and `signal_path` are only used inside the channels bridge now.
#[allow(unused_imports)]
pub use opendesk_channels::signal::{
    IncomingSignal, SignalConfig, SignalStatus, allowed, default_cli_path, detect_cli, load,
    parse_incoming, save, send_text, send_with_attachments, signal_path,
};

pub async fn status(state: &AppState) -> SignalStatus {
    let config = load(&state.db).await.unwrap_or_default();
    let cli_version = detect_cli(&config.cli_path).await;
    SignalStatus {
        cli_available: cli_version.is_some(),
        cli_version,
        bridge_running: state.signal.is_running().await,
        link_running: state.signal.is_linking().await,
        config,
    }
}

/// Shell facade over the channels bridge.
///
/// It forwards process control to `opendesk-channels` and injects the inbound
/// callback that routes a message through `AppState`.
#[derive(Default)]
pub struct SignalBridge {
    inner: Arc<SignalBridgeInner>,
}

impl SignalBridge {
    pub async fn is_running(&self) -> bool {
        self.inner.is_running().await
    }

    pub async fn is_linking(&self) -> bool {
        self.inner.is_linking().await
    }

    pub async fn stop(&self) {
        self.inner.stop().await;
    }

    pub async fn start_if_enabled(self: &Arc<Self>, state: Arc<AppState>) -> AppResult<bool> {
        let handler: Arc<dyn SignalEventHandler> = Arc::new(AppSignalHandler {
            state: Arc::clone(&state),
            bot: None,
        });
        self.inner
            .start_if_enabled(&state.db, handler)
            .await
            .map_err(Into::into)
    }

    /// Starts an isolated `signal-cli` process for a workspace bot.
    pub async fn start_for_bot(
        self: &Arc<Self>,
        state: Arc<AppState>,
        bot: channel_bots::ChannelBot,
        config: SignalConfig,
    ) -> AppResult<bool> {
        let handler: Arc<dyn SignalEventHandler> = Arc::new(AppSignalHandler {
            state: Arc::clone(&state),
            bot: Some(bot),
        });
        self.inner
            .start_with_config(config, handler)
            .await
            .map_err(Into::into)
    }

    pub async fn link(&self, cli_path: &str) -> AppResult<String> {
        self.inner.link(cli_path).await.map_err(Into::into)
    }
}

struct AppSignalHandler {
    state: Arc<AppState>,
    bot: Option<channel_bots::ChannelBot>,
}

/// Per-bot routing data resolved from `channel_chat_contexts`.
pub(crate) struct SignalBotScope {
    pub bot_id: String,
    pub workspace_id: String,
    pub agent: String,
    pub system_context: String,
    pub memory_scope: String,
}

impl SignalEventHandler for AppSignalHandler {
    fn handle<'a>(
        &'a self,
        config: SignalConfig,
        incoming: IncomingSignal,
    ) -> Pin<Box<dyn Future<Output = opendesk_channels::error::AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            match &self.bot {
                Some(bot) => handle_bot_incoming(&self.state, bot, &config, incoming)
                    .await
                    .map_err(Into::into),
                None => handle_incoming(&self.state, &config, incoming, None)
                    .await
                    .map_err(Into::into),
            }
        })
    }
}

/// Resolves the chat context and routes a message from a workspace bot.
async fn handle_bot_incoming(
    state: &Arc<AppState>,
    bot: &channel_bots::ChannelBot,
    config: &SignalConfig,
    incoming: IncomingSignal,
) -> AppResult<()> {
    let context = channel_bots::upsert_seen_context(
        &state.db,
        bot,
        &incoming.source,
        "private",
        &incoming.source,
    )
    .await?;
    if !context.enabled {
        tracing::debug!(bot_id = %bot.id, "contexto de Signal desabilitado: mensagem ignorada");
        return Ok(());
    }
    let scope = SignalBotScope {
        bot_id: bot.id.clone(),
        workspace_id: bot.workspace_id.clone(),
        agent: context
            .agent_override
            .clone()
            .unwrap_or_else(|| bot.default_agent.clone()),
        system_context: context.system_context.clone(),
        memory_scope: context.memory_scope.clone(),
    };
    handle_incoming(state, config, incoming, Some(&scope)).await
}

async fn handle_incoming(
    state: &Arc<AppState>,
    config: &SignalConfig,
    incoming: IncomingSignal,
    scope: Option<&SignalBotScope>,
) -> AppResult<()> {
    if crate::recipes::hitl::signal::handle_reply(
        &crate::app_state::recipe_context(state),
        &incoming.source,
        &incoming.text,
        &[],
    )
    .await?
    {
        return Ok(());
    }
    let trimmed = incoming.text.trim();
    if let Some((short, answer)) = crate::integrations::telegram::parse_interaction_reply(trimmed)
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
        send_text(
            &config.cli_path,
            &config.account,
            &incoming.source,
            &message,
        )
        .await?;
        return Ok(());
    }
    let lower = trimmed.to_lowercase();
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
        send_text(
            &config.cli_path,
            &config.account,
            &incoming.source,
            &message,
        )
        .await?;
        return Ok(());
    }

    let configured_workspace = scope
        .map(|scope| scope.workspace_id.clone())
        .or_else(|| config.workspace_id.clone());
    let workspace_id = match &configured_workspace {
        Some(id) => id.clone(),
        None => settings::active_workspace_id(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("no active workspace for Signal".into()))?,
    };
    if crate::api::schedules::execute_signal_routines(
        state,
        &workspace_id,
        &incoming.source,
        &incoming.text,
    )
    .await?
    {
        return Ok(());
    }
    let external_id = scope
        .map(|scope| format!("signal:{}:{}", scope.bot_id, incoming.source))
        .unwrap_or_else(|| incoming.source.clone());
    let thread =
        match chat_db::find_external(&state.db, &workspace_id, "signal", &external_id).await? {
            Some(thread) => thread,
            None => {
                chat_db::create_thread(
                    &state.db,
                    chat_db::CreateThreadInput {
                        workspace_id: workspace_id.clone(),
                        title: Some(format!("Signal · {}", incoming.source)),
                        agent: scope
                            .map(|scope| scope.agent.clone())
                            .unwrap_or_else(|| config.agent.clone()),
                        model: None,
                        source: Some("signal".into()),
                        external_id: Some(external_id.clone()),
                    },
                )
                .await?
            }
        };

    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &state.db,
        Some(&thread.workspace_id),
    )
    .await
    .unwrap_or_default();
    let mut context =
        chat::channel_context_with(&templates, "Signal", &thread.workspace_id, &thread.id);
    if let Some(scope) = scope {
        if !scope.system_context.trim().is_empty() {
            context.push_str("\n\n## Contexto deste grupo\n");
            context.push_str(scope.system_context.trim());
        }
        if scope.memory_scope == "none" {
            context.push_str("\n\nEsta conversa não deve usar memória pessoal ou estruturada.");
        }
    }
    let reply =
        chat::send_message_with_context(state, thread, &incoming.text, Some(&context)).await?;
    send_text(
        &config.cli_path,
        &config.account,
        &incoming.source,
        &reply.assistant.content,
    )
    .await?;
    Ok(())
}

/// Sends a task HITL request to every allowlisted Signal recipient.
pub async fn notify_task_interaction(
    state: &Arc<AppState>,
    interaction: &crate::db::interactions::TaskInteraction,
    task: &Task,
) -> AppResult<usize> {
    let config = load(&state.db).await?;
    if !config.enabled || config.account.trim().is_empty() || config.allowlist.is_empty() {
        return Ok(0);
    }
    let kind = HitlKind::parse(&interaction.kind).unwrap_or(HitlKind::Text);
    let options =
        serde_json::from_str::<Vec<String>>(&interaction.options_json).unwrap_or_default();
    let short = crate::integrations::telegram::short_id(&interaction.id);
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
        message.push_str(context);
    }
    message.push_str("\n\n");
    match kind {
        HitlKind::Approval => {
            message.push_str(&format!(
                "Responda `#{short} aprovar` ou `#{short} negar`. Para explicar, use `#{short} aprovar <payload>`."
            ));
        }
        HitlKind::SingleChoice | HitlKind::MultiChoice => {
            message.push_str("Opções:");
            for (index, option) in options.iter().enumerate() {
                message.push_str(&format!("\n{}. {option}", index + 1));
            }
            message.push_str(&format!(
                "\n\nResponda `#{short} 1` (ou `#{short} 1,2` para várias) ou `#{short} negar`."
            ));
        }
        HitlKind::Text => {
            message.push_str(&format!(
                "Responda `#{short} <sua resposta>`. Use `#{short} negar` para encerrar sem responder."
            ));
        }
    }
    for recipient in &config.allowlist {
        send_text(&config.cli_path, &config.account, recipient, &message).await?;
    }
    Ok(config.allowlist.len())
}
