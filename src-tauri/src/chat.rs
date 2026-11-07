use crate::{
    app_state::AppState,
    db::{
        budget,
        chat::{self, ChatMessage, ChatThread},
        workspaces,
    },
    error::{AppError, AppResult},
    opencode_client::rest_client::ModelRefDto,
};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

const CHAT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Serialize)]
pub struct ChatReply {
    pub thread: ChatThread,
    pub user: ChatMessage,
    pub assistant: ChatMessage,
    pub usage: ChatUsage,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChatUsage {
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub estimated_cost_usd: f64,
}

pub async fn send_message(
    state: &AppState,
    thread: ChatThread,
    content: &str,
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

    let session_id = match thread.opencode_session_id.clone() {
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

    let user = chat::add_message(&state.db, &thread.id, "user", content, None).await?;

    let prompt_result = tokio::time::timeout(CHAT_TIMEOUT, async {
        client
            .prompt(&session_id, content)
            .await
            .map_err(|error| format!("could not send prompt: {error:#}"))?;
        // The wait endpoint is preferred; if it is unavailable, polling the
        // message list below still gives a useful answer.
        let _ =
            tokio::time::timeout(Duration::from_secs(90), client.wait_session(&session_id)).await;
        client
            .messages(&session_id)
            .await
            .map_err(|error| format!("could not read session messages: {error:#}"))
    })
    .await;

    let (answer, usage, error) = match prompt_result {
        Ok(Ok(value)) => {
            let (text, usage) = parse_assistant(&value);
            let text = text.unwrap_or_else(|| {
                "O agente concluiu a execução sem retornar texto. Veja o log do run.".to_string()
            });
            (text, usage, None)
        }
        Ok(Err(message)) => (format!("⚠️ {message}"), ChatUsage::default(), Some(message)),
        Err(_) => (
            "⚠️ A resposta excedeu o tempo limite de 5 minutos.".to_string(),
            ChatUsage::default(),
            Some("chat timeout after 5 minutes".to_string()),
        ),
    };

    let metadata = serde_json::json!({
        "usage": usage,
        "error": error,
        "session_id": session_id,
    });
    let assistant = chat::add_message(
        &state.db,
        &thread.id,
        "assistant",
        &answer,
        Some(&metadata.to_string()),
    )
    .await?;

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
    })
}

fn parse_model_ref(model: &str) -> Option<ModelRefDto> {
    ModelRefDto::parse(model)
}

fn parse_assistant(value: &Value) -> (Option<String>, ChatUsage) {
    let mut usage = ChatUsage::default();
    let mut answer = None;
    let Some(messages) = value.get("data").and_then(Value::as_array) else {
        return (answer, usage);
    };
    for message in messages {
        if message.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        if let Some(cost) = message.get("cost").and_then(Value::as_f64) {
            usage.estimated_cost_usd += cost;
        }
        if let Some(tokens) = message.get("tokens") {
            usage.tokens_input += tokens.get("input").and_then(Value::as_i64).unwrap_or(0);
            usage.tokens_output += tokens.get("output").and_then(Value::as_i64).unwrap_or(0);
        }
        let text = message
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
            .unwrap_or_default();
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
    fn parses_assistant_text_and_usage() {
        let value = serde_json::json!({
            "data": [
                { "type": "user", "content": [{ "type": "text", "text": "oi" }] },
                {
                    "type": "assistant",
                    "cost": 0.02,
                    "tokens": { "input": 10, "output": 20 },
                    "content": [{ "type": "text", "text": "Olá!" }]
                }
            ]
        });
        let (text, usage) = parse_assistant(&value);
        assert_eq!(text.as_deref(), Some("Olá!"));
        assert_eq!(usage.tokens_input, 10);
        assert_eq!(usage.tokens_output, 20);
        assert!((usage.estimated_cost_usd - 0.02).abs() < f64::EPSILON);
    }
}
