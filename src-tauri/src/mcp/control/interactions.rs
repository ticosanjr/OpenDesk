use super::util::{bool_arg, integer_arg, optional, required};
use crate::app_state::ApiEndpoint;
use crate::internal_api::{call, client, endpoint};
use anyhow::{Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(super) async fn dispatch(
    client: &Client,
    endpoint: &ApiEndpoint,
    _workspace_id: &str,
    _default_agent: &str,
    tool: &str,
    args: &Value,
) -> Option<Result<Value>> {
    Some(match tool {
        "ask_user" => ask_user(client, endpoint, args).await,
        _ => return None,
    })
}

/// Fallback when the client does not support MCP elicitation: ask through the
/// HITL 2.0 interaction inbox using the existing `ask_user` path.
pub async fn elicit_fallback(args: &Value) -> Result<Value> {
    let task_id = required(args, "task_id")?;
    let message = required(args, "message")?;
    let endpoint = endpoint()?;
    let client = client();
    let body = json!({
        "task_id": task_id,
        "question": message,
        "kind": optional(args, "kind").unwrap_or_else(|| "TEXT".into()),
        "context": optional(args, "context"),
        "options": args.get("options").cloned().unwrap_or_else(|| json!([])),
        "multiple": false,
        "allow_custom": true,
        "timeout_minutes": integer_arg(args, "timeout_seconds")
            .map(|value| (value / 60).max(1))
            .unwrap_or(10),
    });
    ask_user(&client, &endpoint, &body).await
}

async fn ask_user(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let task_id = required(args, "task_id")?;
    let body = json!({
        "question": required(args, "question")?,
        "kind": optional(args, "kind"),
        "context": optional(args, "context"),
        "options": args.get("options").cloned().unwrap_or_else(|| json!([])),
        "multiple": bool_arg(args, "multiple").unwrap_or(false),
        "allow_custom": bool_arg(args, "allow_custom").unwrap_or(true),
        "timeout_minutes": integer_arg(args, "timeout_minutes").unwrap_or(10),
    });
    let created = call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/tasks/{task_id}/interactions"),
        Some(body),
    )
    .await?;
    let interaction_id = created
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("a API não devolveu o id da interação"))?
        .to_string();
    let timeout_minutes = integer_arg(args, "timeout_minutes")
        .unwrap_or(10)
        .clamp(1, 120) as u64;
    let started = Instant::now();
    loop {
        let current = call(
            client,
            endpoint,
            Method::GET,
            &format!("/api/interactions/{interaction_id}"),
            None,
        )
        .await?;
        let status = current
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("PENDING");
        if status != "PENDING" {
            let answer = current
                .get("answer_json")
                .and_then(Value::as_str)
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
            return Ok(json!({
                "interaction_id": interaction_id,
                "status": status,
                "answer": answer,
            }));
        }
        if started.elapsed() >= Duration::from_secs(timeout_minutes * 60) {
            return Ok(json!({
                "interaction_id": interaction_id,
                "status": "TIMEOUT",
                "message": "O humano não respondeu dentro do prazo. Não assuma dados críticos; bloqueie a tarefa ou peça instruções."
            }));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
