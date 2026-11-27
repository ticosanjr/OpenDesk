use crate::{
    app_state::AppState,
    db::{
        budget,
        chat::{self, ChatMessage, ChatThread},
        chat_turns::{self, CreateChatTurnInput},
        runs::{self, CreateChatRunInput},
        settings, workspaces,
    },
    error::{AppError, AppResult},
};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(super) const CANCEL_MESSAGE: &str = "chat turn cancelled";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatAttachmentMeta {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub mime: Option<String>,
    #[serde(default)]
    pub size: Option<i64>,
    pub direction: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[path = "chat_parse.rs"]
mod chat_parse;

pub use chat_parse::ChatUsage;
use chat_parse::*;

#[path = "chat/chat_turn.rs"]
mod chat_turn;
use chat_turn::wait_for_turn;

#[cfg(test)]
#[path = "chat/chat_tests.rs"]
mod chat_tests;

#[derive(Debug, Clone, Serialize)]
pub struct ChatReply {
    pub thread: ChatThread,
    pub user: ChatMessage,
    pub assistant: ChatMessage,
    pub usage: ChatUsage,
    pub error: Option<String>,
    pub run_id: Option<String>,
    pub turn_id: Option<String>,
}

pub async fn send_message_with_context(
    state: &AppState,
    thread: ChatThread,
    content: &str,
    context: Option<&str>,
) -> AppResult<ChatReply> {
    send_message_with_metadata(state, thread, content, context, None).await
}

pub async fn send_message_with_metadata(
    state: &AppState,
    thread: ChatThread,
    content: &str,
    context: Option<&str>,
    user_metadata: Option<String>,
) -> AppResult<ChatReply> {
    let content = content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest("message is required".into()));
    }
    let workspace = workspaces::get(&state.db, &thread.workspace_id).await?;

    let check = budget::check(&state.db).await?;
    if !check.allowed {
        return Err(AppError::Conflict(check.message));
    }

    let client = state.daemon.client().await.ok_or_else(|| {
        AppError::Daemon(
            "OpenCode is offline. Configure o motor em Configurações → OpenCode Engine.".into(),
        )
    })?;

    let mut session_id = match thread.opencode_session_id.clone() {
        Some(session_id) => session_id,
        None => {
            let session_id = client
                .create_session(
                    &workspace.root_path,
                    &thread.title,
                    &thread.agent,
                    thread.model.as_deref().and_then(parse_model_ref),
                )
                .await
                .map_err(|error| {
                    AppError::Daemon(format!("could not create chat session: {error:#}"))
                })?;
            chat::update_session(&state.db, &thread.id, &session_id).await?;
            session_id
        }
    };

    let started = Instant::now();
    let started_at = Utc::now().to_rfc3339();
    let turn = chat_turns::create(
        &state.db,
        CreateChatTurnInput {
            workspace_id: thread.workspace_id.clone(),
            thread_id: thread.id.clone(),
            source: thread.source.clone(),
            agent: thread.agent.clone(),
            model: thread.model.clone(),
            user_content: content.to_string(),
        },
    )
    .await
    .ok();
    let cancel = turn
        .as_ref()
        .map(|turn| state.begin_chat_turn(&thread.id, &turn.id, Some(&session_id)));

    let user = chat::add_message(
        &state.db,
        &thread.id,
        "user",
        content,
        user_metadata.as_deref(),
    )
    .await?;
    state
        .events
        .publish(crate::events::OpenDeskWsEvent::ChatUpdated {
            thread_id: thread.id.clone(),
            workspace_id: thread.workspace_id.clone(),
        });

    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &state.db,
        Some(&thread.workspace_id),
    )
    .await
    .unwrap_or_default();
    let memory_bodies = crate::chat_memory::chat_memory_bodies(state, &thread)
        .await
        .unwrap_or_default();
    let personal_body = memory_bodies.personal;
    let personal_context = if personal_body.is_empty() {
        String::new()
    } else {
        crate::dsh_router::prompt_templates::render(
            &templates.personal_block,
            &[("personal_context", &personal_body)],
        )
    };
    let memory_policy = crate::dsh_router::prompt_templates::render(&templates.memory_policy, &[]);
    let mut parts = Vec::new();
    if !personal_context.trim().is_empty() {
        parts.push(personal_context.trim().to_string());
    }
    if !memory_bodies.structured.trim().is_empty() {
        parts.push(crate::dsh_router::prompt_templates::render(
            &templates.structured_memory_block,
            &[("structured_memory", memory_bodies.structured.trim())],
        ));
    }
    if let Some(context) = context.map(str::trim).filter(|value| !value.is_empty()) {
        parts.push(context.to_string());
    }
    if !memory_policy.trim().is_empty() {
        parts.push(memory_policy);
    }
    parts.push(crate::dsh_router::context_packer::untrusted_block(
        "chat", content,
    ));
    let prompt_text = parts.join("\n\n");

    let since_ms = now_ms();
    let timeout_minutes = settings::get(&state.db, "chat_turn_timeout_minutes")
        .await
        .ok()
        .flatten()
        .and_then(|value| value.as_u64())
        .unwrap_or(30)
        .clamp(1, 180);
    let turn_timeout = Duration::from_secs(timeout_minutes * 60);
    let mut prompt_message = Value::Null;
    let prompt_result = async {
        // Subscribe before prompting so an immediate terminal event cannot be
        // lost between the prompt and wait_for_turn.
        let mut session_events = client.event_receiver().await.ok();
        match client.prompt(&session_id, &prompt_text).await {
            Ok(value) => prompt_message = value,
            Err(error) => {
                let message = format!("{error:#}");
                if !is_missing_session_error(&message) {
                    return Err(format!("could not send prompt: {message}"));
                }
                // The daemon may have restarted or pruned old sessions.
                // Recreate once so the local thread keeps its history/context.
                let session_id_new = client
                    .create_session(
                        &workspace.root_path,
                        &thread.title,
                        &thread.agent,
                        thread.model.as_deref().and_then(parse_model_ref),
                    )
                    .await
                    .map_err(|error| format!("could not recreate chat session: {error:#}"))?;
                chat::update_session(&state.db, &thread.id, &session_id_new)
                    .await
                    .map_err(|error| format!("could not persist new chat session: {error}"))?;
                session_id = session_id_new;
                prompt_message = client
                    .prompt(&session_id, &prompt_text)
                    .await
                    .map_err(|error| format!("could not send prompt: {error:#}"))?;
            }
        }
        match cancel.as_ref() {
            Some(cancel) => {
                wait_for_turn(
                    &client,
                    &session_id,
                    since_ms,
                    cancel,
                    session_events.take(),
                    turn_timeout,
                )
                .await
            }
            None => {
                wait_for_turn(
                    &client,
                    &session_id,
                    since_ms,
                    &CancellationToken::new(),
                    session_events.take(),
                    turn_timeout,
                )
                .await
            }
        }
    }
    .await;

    let (answer, usage, error, status, turn_value) = match prompt_result {
        Ok(value) => {
            let (text, usage) = parse_assistant_since(&value, since_ms);
            match text {
                Some(text) => (text, usage, None, "SUCCESS", Some(value)),
                None => {
                    let message = "O agente encerrou o turno sem texto. Tente novamente; se persistir, veja os eventos da sessão.";
                    (
                        format!("⚠️ {message}"),
                        usage,
                        Some("empty assistant response".to_string()),
                        "FAILED",
                        Some(value),
                    )
                }
            }
        }
        Err(message) if message == CANCEL_MESSAGE => (
            "Execução interrompida.".to_string(),
            ChatUsage::default(),
            Some("cancelled".to_string()),
            "CANCELLED",
            None,
        ),
        Err(message) => (
            format!("⚠️ {message}"),
            ChatUsage::default(),
            Some(message),
            "FAILED",
            None,
        ),
    };

    let duration_ms = started.elapsed().as_millis() as i64;
    let prompt_message_id = prompt_message
        .pointer("/data/info/id")
        .or_else(|| prompt_message.pointer("/info/id"))
        .or_else(|| prompt_message.pointer("/data/id"))
        .or_else(|| prompt_message.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let (has_tool, writes) = turn_value
        .as_ref()
        .map(|value| crate::opencode_client::session_runner::chat_turn_signals(value, since_ms))
        .unwrap_or((false, Vec::new()));
    let diffs = if let (Some(value), Some(message_id)) = (&turn_value, prompt_message_id.as_deref())
    {
        let _ = value;
        client
            .diff_for_message(&session_id, message_id)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let used_tools = has_tool || !diffs.is_empty() || !writes.is_empty();

    let mut run_id = None;
    if used_tools {
        let (provider_id, model_id) = turn_value
            .as_ref()
            .and_then(|value| turn_model(value, since_ms))
            .or_else(|| {
                thread
                    .model
                    .as_deref()
                    .and_then(parse_model_ref)
                    .map(|model| (Some(model.provider_id), model.id))
            })
            .unwrap_or((None, "auto".to_string()));
        let id = Uuid::now_v7().to_string();
        let run = runs::create_chat(
            &state.db,
            CreateChatRunInput {
                id: id.clone(),
                workspace_id: thread.workspace_id.clone(),
                session_id: Some(session_id.clone()),
                agent_name: thread.agent.clone(),
                provider_id,
                model_id,
                channel: thread.source.clone(),
                origin_ref: thread.id.clone(),
                started_at: started_at.clone(),
            },
        )
        .await;
        if let Ok(_run) = run {
            let _ = runs::finish(
                &state.db,
                &id,
                status,
                &run_usage(&usage),
                duration_ms,
                Some(&answer),
                None,
            )
            .await;
            if let Some(error) = &error {
                let _ = runs::set_error(&state.db, &id, "chat_turn_failed", error, "").await;
            }
            if let Some(turn) = &turn {
                let _ = chat_turns::set_run(&state.db, &turn.id, &id).await;
            }
            crate::queue_scheduler::run_execution::record_chat_artifacts(
                &state.db,
                &state.events,
                &thread.workspace_id,
                &id,
                &thread.agent,
                &format!("Chat · {}", thread.source),
                std::path::Path::new(&workspace.root_path),
                &diffs,
                &writes,
            )
            .await;
            run_id = Some(id);
        }
    }

    let attachments = match run_id.as_deref() {
        Some(id) => crate::db::artifacts::list_by_run(&state.db, id, 100)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|artifact| ChatAttachmentMeta {
                name: artifact
                    .file_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&artifact.file_path)
                    .to_string(),
                path: artifact.file_path,
                mime: None,
                size: Some(artifact.file_size_bytes),
                direction: "received".into(),
                source: Some("agent".into()),
            })
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };
    let metadata = serde_json::json!({
        "usage": usage,
        "error": error,
        "session_id": session_id,
        "run_id": run_id,
        "attachments": attachments,
    });
    let assistant = chat::add_message(
        &state.db,
        &thread.id,
        "assistant",
        &answer,
        Some(&metadata.to_string()),
    )
    .await?;
    state
        .events
        .publish(crate::events::OpenDeskWsEvent::ChatUpdated {
            thread_id: thread.id.clone(),
            workspace_id: thread.workspace_id.clone(),
        });

    if let Some(turn) = &turn {
        let _ = chat_turns::finish(
            &state.db,
            &turn.id,
            status,
            Some(&answer),
            &run_usage(&usage),
            duration_ms,
            error.as_deref().map(|_| "chat_turn_failed"),
            error.as_deref(),
        )
        .await;
        state.finish_chat_turn(&thread.id, &turn.id);
    }

    let thread = chat::get_thread(&state.db, &thread.id).await?;
    if thread.title == "Nova conversa" {
        let title = content.chars().take(60).collect::<String>();
        chat::update_title(&state.db, &thread.id, title.trim()).await?;
    }
    let thread = chat::get_thread(&state.db, &thread.id).await?;

    Ok(ChatReply {
        thread,
        user,
        assistant,
        usage,
        error,
        run_id,
        turn_id: turn.map(|turn| turn.id),
    })
}

/// Operational context prepended to every channel prompt. Telegram, Signal and
/// the desktop chat share one vocabulary so the agent always knows where it is
/// and how to send a workspace file back to the user.
pub fn channel_context_with(
    templates: &crate::dsh_router::prompt_templates::PromptTemplates,
    source: &str,
    workspace_id: &str,
    thread_id: &str,
) -> String {
    let external = matches!(source.to_ascii_lowercase().as_str(), "telegram" | "signal");
    let file_hint = if external {
        format!(
            "Para enviar um arquivo do workspace ao usuário, chame a ferramenta MCP `chat_send_file` com thread_id `{thread_id}` e o caminho relativo. Quando o usuário pedir um arquivo, envie-o; não diga que não consegue anexar.\n"
        )
    } else {
        String::new()
    };
    crate::dsh_router::prompt_templates::render(
        &templates.chat_channel,
        &[
            ("source", source),
            ("workspace_id", workspace_id),
            ("thread_id", thread_id),
            ("file_hint", &file_hint),
        ],
    )
}
