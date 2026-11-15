use crate::{
    app_state::AppState,
    db::{
        budget,
        chat::{self, ChatMessage, ChatThread},
        chat_turns::{self, CreateChatTurnInput},
        runs::{self, CreateChatRunInput, RunUsage},
        workspaces,
    },
    error::{AppError, AppResult},
    opencode_client::rest_client::ModelRefDto,
};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const CANCEL_MESSAGE: &str = "chat turn cancelled";

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

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChatUsage {
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub tokens_cache_read: i64,
    pub tokens_cache_write: i64,
    pub estimated_cost_usd: f64,
}

pub async fn send_message_with_context(
    state: &AppState,
    thread: ChatThread,
    content: &str,
    context: Option<&str>,
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

    let user = chat::add_message(&state.db, &thread.id, "user", content, None).await?;
    state
        .events
        .publish(crate::events::OpenDeskWsEvent::ChatUpdated {
            thread_id: thread.id.clone(),
            workspace_id: thread.workspace_id.clone(),
        });

    let personal_context = crate::personal_context::render(&state.db, &thread.workspace_id)
        .await
        .unwrap_or_default();
    let prompt_text = match context.map(str::trim).filter(|value| !value.is_empty()) {
        Some(context) => format!("{personal_context}\n\n{context}\n\n{content}"),
        None if personal_context.is_empty() => content.to_string(),
        None => format!("{personal_context}\n\n{content}"),
    };

    let since_ms = now_ms();
    let mut prompt_message = Value::Null;
    let prompt_result = async {
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
            Some(cancel) => wait_for_turn(&client, &session_id, since_ms, cancel).await,
            None => wait_for_turn(&client, &session_id, since_ms, &CancellationToken::new()).await,
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

    let metadata = serde_json::json!({
        "usage": usage,
        "error": error,
        "session_id": session_id,
        "run_id": run_id,
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

fn run_usage(usage: &ChatUsage) -> RunUsage {
    RunUsage {
        tokens_input: usage.tokens_input,
        tokens_output: usage.tokens_output,
        tokens_reasoning: usage.tokens_reasoning,
        tokens_cache_read: usage.tokens_cache_read,
        tokens_cache_write: usage.tokens_cache_write,
        estimated_cost_usd: usage.estimated_cost_usd,
    }
}

fn turn_model(value: &Value, since_ms: i64) -> Option<(Option<String>, String)> {
    let messages = value.get("data").and_then(Value::as_array)?;
    for message in messages.iter().rev() {
        if message.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let created = message
            .pointer("/time/created")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if created < since_ms {
            continue;
        }
        let Some(model_id) = message.get("modelID").and_then(Value::as_str) else {
            continue;
        };
        return Some((
            message
                .get("providerID")
                .and_then(Value::as_str)
                .map(str::to_string),
            model_id.to_string(),
        ));
    }
    None
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

/// The daemon reports a missing session as a 404 or "session not found". Only
/// those cases are safe to retry with a fresh session; other errors (network,
/// budget, invalid model) must surface without creating duplicate work.
fn is_missing_session_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("404")
        || (lower.contains("session") && (lower.contains("not found") || lower.contains("unknown")))
}

fn parse_model_ref(model: &str) -> Option<ModelRefDto> {
    ModelRefDto::parse(model)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0)
}

fn message_created(message: &Value) -> Option<i64> {
    message
        .pointer("/time/created")
        .and_then(Value::as_i64)
        .or_else(|| message.get("created").and_then(Value::as_i64))
}

fn message_text(message: &Value) -> String {
    message
        .get("content")
        .and_then(Value::as_array)
        .map(|content| {
            content
                .iter()
                .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

fn turn_has_idle(value: &Value, since_ms: i64) -> bool {
    value
        .get("data")
        .and_then(Value::as_array)
        .map(|messages| {
            messages.iter().any(|message| {
                message.get("type").and_then(Value::as_str) == Some("idle")
                    && message_created(message).is_some_and(|created| created >= since_ms)
            })
        })
        .unwrap_or(false)
}

fn turn_has_text(value: &Value, since_ms: i64) -> bool {
    value
        .get("data")
        .and_then(Value::as_array)
        .map(|messages| {
            messages.iter().any(|message| {
                message.get("type").and_then(Value::as_str) == Some("assistant")
                    && message_created(message).is_some_and(|created| created >= since_ms)
                    && !message_text(message).trim().is_empty()
            })
        })
        .unwrap_or(false)
}

fn turn_signature(value: &Value, since_ms: i64) -> String {
    value
        .get("data")
        .and_then(Value::as_array)
        .map(|messages| {
            messages
                .iter()
                .filter(|message| {
                    message.get("type").and_then(Value::as_str) == Some("assistant")
                        && message_created(message).is_some_and(|created| created >= since_ms)
                })
                .map(|message| {
                    let id = message.get("id").and_then(Value::as_str).unwrap_or("");
                    let text = message_text(message);
                    format!("{id}:{}", text.len())
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .unwrap_or_default()
}

/// Waits for the current turn to finish before reading the answer. OpenCode's
/// prompt endpoint is asynchronous; reading the message list immediately used
/// to return the previous assistant reply.
///
/// A tool-using turn emits intermediate `assistant` messages that contain only
/// tool calls. Those must never be mistaken for the final answer, so the idle
/// marker is authoritative: the turn ends only when OpenCode reports idle and a
/// text message from this turn exists. The quiet-signature fallback is only a
/// safety net for a missing idle marker and requires text (30s of silence).
async fn wait_for_turn(
    client: &crate::opencode_client::rest_client::RestClient,
    session_id: &str,
    since_ms: i64,
    cancel: &CancellationToken,
) -> Result<Value, String> {
    const IDLE_WITHOUT_TEXT_GRACE: Duration = Duration::from_secs(2);
    const TEXT_STABLE_GRACE: Duration = Duration::from_secs(30);
    let mut stable_signature = String::new();
    let mut stable_since: Option<std::time::Instant> = None;
    let mut idle_without_text_since: Option<std::time::Instant> = None;
    loop {
        if cancel.is_cancelled() {
            return Err(CANCEL_MESSAGE.into());
        }
        let value = client
            .messages(session_id)
            .await
            .map_err(|error| format!("could not read session messages: {error:#}"))?;
        let signature = turn_signature(&value, since_ms);
        let has_text = turn_has_text(&value, since_ms);
        let has_idle = turn_has_idle(&value, since_ms);

        if has_text && has_idle {
            return Ok(value);
        }
        if has_idle && !has_text {
            // The message list is ordered, but keep a tiny grace in case the
            // final text lands just after the idle marker.
            let since = *idle_without_text_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= IDLE_WITHOUT_TEXT_GRACE {
                return Ok(value);
            }
        } else {
            idle_without_text_since = None;
        }

        if !has_text {
            stable_signature.clear();
            stable_since = None;
        } else if signature == stable_signature {
            if stable_since.is_some_and(|since| since.elapsed() >= TEXT_STABLE_GRACE) {
                return Ok(value);
            }
        } else {
            stable_signature = signature;
            stable_since = Some(std::time::Instant::now());
        }
        tokio::select! {
            _ = cancel.cancelled() => return Err(CANCEL_MESSAGE.into()),
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
        }
    }
}

fn parse_assistant_since(value: &Value, since_ms: i64) -> (Option<String>, ChatUsage) {
    let mut usage = ChatUsage::default();
    let mut answer = None;
    let Some(messages) = value.get("data").and_then(Value::as_array) else {
        return (answer, usage);
    };
    for message in messages {
        if message.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        if !message_created(message).is_some_and(|created| created >= since_ms) {
            continue;
        }
        if let Some(cost) = message.get("cost").and_then(Value::as_f64) {
            usage.estimated_cost_usd += cost;
        }
        if let Some(tokens) = message.get("tokens") {
            usage.tokens_input += tokens.get("input").and_then(Value::as_i64).unwrap_or(0);
            usage.tokens_output += tokens.get("output").and_then(Value::as_i64).unwrap_or(0);
            usage.tokens_reasoning += tokens.get("reasoning").and_then(Value::as_i64).unwrap_or(0);
            if let Some(cache) = tokens.get("cache") {
                usage.tokens_cache_read += cache.get("read").and_then(Value::as_i64).unwrap_or(0);
                usage.tokens_cache_write += cache.get("write").and_then(Value::as_i64).unwrap_or(0);
            }
        }
        let text = message_text(message);
        if !text.trim().is_empty() {
            answer = Some(text);
        }
    }
    (answer, usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_refs_with_variants() {
        let model = parse_model_ref("opencode-go/glm-5.3#high").expect("model");
        assert_eq!(model.provider_id, "opencode-go");
        assert_eq!(model.id, "glm-5.3");
        assert_eq!(model.variant.as_deref(), Some("high"));
        assert_eq!(
            parse_model_ref("opencode-go/glm-5.3")
                .expect("plain")
                .variant,
            None
        );
        assert_eq!(
            parse_model_ref("opencode-go/glm-5.3# ")
                .expect("empty")
                .variant,
            None
        );
        assert!(parse_model_ref("no-provider").is_none());
    }

    #[test]
    fn detects_only_missing_session_errors() {
        assert!(is_missing_session_error(
            "OpenCode prompt failed (404 Not Found): {\"error\":\"session not found\"}"
        ));
        assert!(is_missing_session_error("unknown session abc"));
        assert!(!is_missing_session_error(
            "OpenCode prompt failed (500): timeout"
        ));
        assert!(!is_missing_session_error("could not send OpenCode prompt"));
    }

    fn turn_fixture() -> serde_json::Value {
        serde_json::json!({
            "data": [
                {
                    "id": "old",
                    "type": "assistant",
                    "time": { "created": 500, "completed": 600 },
                    "cost": 99.0,
                    "tokens": { "input": 1, "output": 1 },
                    "content": [{ "type": "text", "text": "Olá! Sou o agente." }]
                },
                {
                    "id": "new",
                    "type": "assistant",
                    "time": { "created": 2000, "completed": 2100 },
                    "cost": 0.02,
                    "tokens": { "input": 10, "output": 20 },
                    "content": [
                        { "type": "reasoning", "text": "pensando" },
                        { "type": "tool", "name": "read" },
                        { "type": "text", "text": "Meu poema novo" }
                    ]
                },
                { "id": "idle", "type": "idle", "time": { "created": 2200 } }
            ]
        })
    }

    #[test]
    fn parses_only_the_current_turn_and_ignores_the_old_answer() {
        let value = turn_fixture();
        assert!(turn_has_idle(&value, 1000));
        assert!(turn_has_text(&value, 1000));
        assert!(!turn_has_text(&value, 2300));
        assert!(!turn_has_idle(&value, 2300));
        let (text, usage) = parse_assistant_since(&value, 1000);
        assert_eq!(text.as_deref(), Some("Meu poema novo"));
        assert_eq!(usage.tokens_input, 10);
        assert_eq!(usage.tokens_output, 20);
        assert!((usage.estimated_cost_usd - 0.02).abs() < f64::EPSILON);

        let (old, usage) = parse_assistant_since(&value, 3000);
        assert!(old.is_none(), "never fall back to an older answer");
        assert_eq!(usage.tokens_input, 0);
    }

    #[tokio::test]
    async fn waits_for_a_new_turn_instead_of_returning_the_previous_answer() {
        use axum::{Json, Router, extract::State, routing::get};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        async fn messages(State(calls): State<Arc<AtomicUsize>>) -> Json<serde_json::Value> {
            let call = calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                Json(serde_json::json!({
                    "data": [{
                        "id": "old",
                        "type": "assistant",
                        "time": { "created": 500, "completed": 600 },
                        "content": [{ "type": "text", "text": "Olá! Sou o agente." }]
                    }]
                }))
            } else {
                Json(turn_fixture())
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/api/session/{id}/message", get(messages))
            .with_state(calls);
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let client = crate::opencode_client::rest_client::RestClient::new_with_url(
            &format!("http://{address}"),
            "secret".into(),
        );
        let value = wait_for_turn(&client, "ses-1", 1000, &CancellationToken::new())
            .await
            .expect("turn");
        let (text, _) = parse_assistant_since(&value, 1000);
        assert_eq!(text.as_deref(), Some("Meu poema novo"));
        handle.abort();
    }

    #[tokio::test]
    async fn does_not_stop_on_a_tool_only_message() {
        use axum::{Json, Router, extract::State, routing::get};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        async fn messages(State(calls): State<Arc<AtomicUsize>>) -> Json<serde_json::Value> {
            let call = calls.fetch_add(1, Ordering::SeqCst);
            if call < 3 {
                // The agent is still working: an assistant message exists, but
                // it only carries a tool call and there is no idle marker.
                Json(serde_json::json!({
                    "data": [{
                        "id": "tool-step",
                        "type": "assistant",
                        "time": { "created": 2000 },
                        "content": [{
                            "type": "tool",
                            "name": "execute",
                            "state": { "status": "completed" }
                        }]
                    }]
                }))
            } else {
                Json(turn_fixture())
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/api/session/{id}/message", get(messages))
            .with_state(calls);
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let client = crate::opencode_client::rest_client::RestClient::new_with_url(
            &format!("http://{address}"),
            "secret".into(),
        );
        let value = wait_for_turn(&client, "ses-1", 1000, &CancellationToken::new())
            .await
            .expect("turn with tools");
        let (text, _) = parse_assistant_since(&value, 1000);
        assert_eq!(text.as_deref(), Some("Meu poema novo"));
        handle.abort();
    }
}
