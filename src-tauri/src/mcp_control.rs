//! Operational MCP server exposed to OpenCode as `mcp.servers.opendesk-control`.
//!
//! It is intentionally a thin client over the local OpenDesk API: validation,
//! events, queueing and audit stay in the backend. Configuration changes for
//! agents/skills/commands/MCP files stay in `opendesk-config` proposals; this
//! server covers day-to-day operation (tasks, runs, routines, recipes, queue,
//! artifacts and the attention inbox).
//!
//! Safety boundaries:
//! - HITL checkpoints, permission requests and config proposals are read-only.
//! - New routines and recipes are always created disabled by the backend.
//! - Destructive deletion is not exposed in this first version.

use crate::app_state::ApiEndpoint;
use crate::internal_api::{call, client, endpoint};
use crate::mcp_server;
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

mod payload;
mod tools;
mod util;

use payload::{routine_body, routine_update_body, task_create_body, task_update_body};
use tools::tools;
use util::{bool_arg, integer_arg, optional, optional_str, required, ws_path};

pub async fn run_stdio() -> Result<()> {
    let workspace_id = std::env::var("OPENDESK_WORKSPACE_ID")
        .context("OPENDESK_WORKSPACE_ID é obrigatório para o MCP de controle")?;

    mcp_server::serve_stdio("opendesk-control", tools(), |name, arguments| {
        let workspace_id = &workspace_id;
        async move {
            dispatch(workspace_id, &name, arguments)
                .await
                .map(|value| value.to_string())
        }
    })
    .await
}

async fn dispatch(workspace_id: &str, tool: &str, args: Value) -> Result<Value> {
    let endpoint = endpoint()?;
    let client = client();
    match tool {
        "opendesk_status" => status(&client, &endpoint, workspace_id).await,
        "task_list" => task_list(&client, &endpoint, workspace_id, &args).await,
        "task_create" => {
            let body = task_create_body(&args)?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path("/api/tasks", workspace_id),
                Some(body),
            )
            .await
        }
        "task_update" => {
            let id = required(&args, "id")?;
            let body = task_update_body(&args)?;
            call(
                &client,
                &endpoint,
                Method::PATCH,
                &format!("/api/tasks/{id}"),
                Some(body),
            )
            .await
        }
        "task_move" => {
            let id = required(&args, "id")?;
            let body = json!({
                "status": required(&args, "status")?,
                "reason": optional(&args, "reason"),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/tasks/{id}/transition"),
                Some(body),
            )
            .await
        }
        "ask_user" => ask_user(&client, &endpoint, &args).await,
        "task_dispatch" => {
            let id = required(&args, "id")?;
            let body = json!({
                "policy": optional(&args, "policy"),
                "manual_agent": optional(&args, "manual_agent"),
                "manual_model": optional(&args, "manual_model"),
                "manual_effort": optional(&args, "manual_effort"),
                "execution_mode": optional(&args, "execution_mode"),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/tasks/{id}/dispatch"),
                Some(body),
            )
            .await
        }
        "task_message_list" => {
            let id = required(&args, "id")?;
            call(
                &client,
                &endpoint,
                Method::GET,
                &format!("/api/tasks/{id}/messages"),
                None,
            )
            .await
        }
        "task_message_post" => {
            let id = required(&args, "id")?;
            let body = json!({
                "content": required(&args, "content")?,
                "run_id": optional(&args, "run_id"),
                "continue_execution": bool_arg(&args, "continue_execution").unwrap_or(false),
                "agent": optional(&args, "agent"),
                "model": optional(&args, "model"),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/tasks/{id}/messages"),
                Some(body),
            )
            .await
        }
        "run_list" => run_list(&client, &endpoint, workspace_id, &args).await,
        "routine_list" => {
            if let Some(id) = optional_str(&args, "id") {
                call(
                    &client,
                    &endpoint,
                    Method::GET,
                    &format!("/api/schedules/{id}"),
                    None,
                )
                .await
            } else {
                call(
                    &client,
                    &endpoint,
                    Method::GET,
                    &ws_path("/api/schedules", workspace_id),
                    None,
                )
                .await
            }
        }
        "routine_create" => {
            let body = routine_body(workspace_id, &args)?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path("/api/schedules", workspace_id),
                Some(body),
            )
            .await
        }
        "routine_update" => {
            let id = required(&args, "id")?;
            let body = routine_update_body(&args)?;
            call(
                &client,
                &endpoint,
                Method::PATCH,
                &format!("/api/schedules/{id}"),
                Some(body),
            )
            .await
        }
        "routine_set_active" => {
            let id = required(&args, "id")?;
            let active = bool_arg(&args, "active")
                .ok_or_else(|| anyhow!("argumento 'active' é obrigatório"))?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/schedules/{id}/active"),
                Some(json!({ "active": active })),
            )
            .await
        }
        "routine_run" => {
            let id = required(&args, "id")?;
            let dry_run = bool_arg(&args, "dry_run").unwrap_or(false);
            let path = if dry_run {
                format!("/api/schedules/{id}/dry-run")
            } else {
                format!("/api/schedules/{id}/run")
            };
            call(&client, &endpoint, Method::POST, &path, Some(json!({}))).await
        }
        "recipe_list" => recipe_list(&client, &endpoint, workspace_id, &args).await,
        "recipe_save" => {
            let id = required(&args, "id")?;
            let yaml = required(&args, "yaml")?;
            call(
                &client,
                &endpoint,
                Method::PUT,
                &ws_path(&format!("/api/recipes/{id}"), workspace_id),
                Some(json!({ "yaml": yaml })),
            )
            .await
        }
        "recipe_set_enabled" => {
            let id = required(&args, "id")?;
            let enabled = bool_arg(&args, "enabled")
                .ok_or_else(|| anyhow!("argumento 'enabled' é obrigatório"))?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path(&format!("/api/recipes/{id}/enabled"), workspace_id),
                Some(json!({ "enabled": enabled })),
            )
            .await
        }
        "recipe_run" => {
            let id = required(&args, "id")?;
            let body = json!({
                "trigger_context": args
                    .get("trigger_context")
                    .cloned()
                    .filter(|value| !value.is_null())
                    .unwrap_or_else(|| json!({ "manual": true })),
                "dry_run": bool_arg(&args, "dry_run").unwrap_or(false),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path(&format!("/api/recipes/{id}/run"), workspace_id),
                Some(body),
            )
            .await
        }
        "attention_list" => call(
            &client,
            &endpoint,
            Method::GET,
            &ws_path("/api/attention", workspace_id),
            None,
        )
        .await
        .map(attention_summary),
        "queue_control" => queue_control(&client, &endpoint, workspace_id, &args).await,
        "artifact_list" => artifact_list(&client, &endpoint, workspace_id, &args).await,
        "artifact_read" => {
            let id = required(&args, "id")?;
            let max_bytes = integer_arg(&args, "max_bytes")
                .unwrap_or(65_536)
                .clamp(1024, 2 * 1024 * 1024);
            call(
                &client,
                &endpoint,
                Method::GET,
                &format!("/api/artifacts/{id}/content?max_bytes={max_bytes}"),
                None,
            )
            .await
        }
        "artifact_pin" => {
            let id = required(&args, "id")?;
            let pinned = bool_arg(&args, "pinned")
                .ok_or_else(|| anyhow!("argumento 'pinned' é obrigatório"))?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/artifacts/{id}/pin"),
                Some(json!({ "pinned": pinned })),
            )
            .await
        }
        "artifact_rollback" => {
            let id = required(&args, "id")?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/artifacts/{id}/rollback"),
                Some(json!({})),
            )
            .await
        }
        other => Err(anyhow!("ferramenta desconhecida: {other}")),
    }
}

async fn status(client: &Client, endpoint: &ApiEndpoint, workspace_id: &str) -> Result<Value> {
    let health = call(client, endpoint, Method::GET, "/api/health", None).await?;
    let queue = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/queue/status", workspace_id),
        None,
    )
    .await?;
    let dashboard = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/dashboard", workspace_id),
        None,
    )
    .await?;
    let attention = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/attention", workspace_id),
        None,
    )
    .await?;
    Ok(json!({
        "workspace_id": workspace_id,
        "health": health,
        "queue": queue,
        "dashboard": dashboard,
        "attention": attention_summary(attention),
    }))
}

fn attention_summary(value: Value) -> Value {
    let count = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0)
    };
    json!({
        "awaiting_approval": count("awaiting_approval"),
        "blocked": count("blocked"),
        "failed_runs": count("failed_runs"),
        "requests": count("requests"),
        "hitl": count("hitl"),
        "proposals": count("proposals"),
        "items": value,
    })
}

async fn task_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        return call(
            client,
            endpoint,
            Method::GET,
            &format!("/api/tasks/{id}"),
            None,
        )
        .await;
    }
    let tasks = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/tasks", workspace_id),
        None,
    )
    .await?;
    let status = optional_str(args, "status");
    let agent = optional_str(args, "agent");
    let limit = integer_arg(args, "limit").unwrap_or(100).max(1) as usize;
    let filtered = tasks
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|task| {
                    status.is_none_or(|expected| {
                        task.get("status").and_then(Value::as_str) == Some(expected)
                    }) && agent.is_none_or(|expected| {
                        task.get("assigned_agent").and_then(Value::as_str) == Some(expected)
                    })
                })
                .take(limit)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(Value::Array(filtered))
}

async fn run_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        let run = call(
            client,
            endpoint,
            Method::GET,
            &format!("/api/runs/{id}"),
            None,
        )
        .await?;
        if bool_arg(args, "include_timeline").unwrap_or(false) {
            let timeline = call(
                client,
                endpoint,
                Method::GET,
                &format!("/api/runs/{id}/timeline"),
                None,
            )
            .await?;
            return Ok(json!({ "run": run, "timeline": timeline }));
        }
        return Ok(run);
    }
    let mut path = ws_path("/api/runs", workspace_id);
    if let Some(task_id) = optional_str(args, "task_id") {
        path.push('&');
        path.push_str("task_id=");
        path.push_str(&urlencoding::encode(task_id));
    }
    if let Some(limit) = integer_arg(args, "limit") {
        path.push('&');
        path.push_str(&format!("limit={}", limit.max(1)));
    }
    let runs = call(client, endpoint, Method::GET, &path, None).await?;
    let status = optional_str(args, "status");
    let filtered = runs
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|run| {
                    status.is_none_or(|expected| {
                        run.get("status").and_then(Value::as_str) == Some(expected)
                    })
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(Value::Array(filtered))
}

async fn recipe_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        let mut value = call(
            client,
            endpoint,
            Method::GET,
            &ws_path(&format!("/api/recipes/{id}"), workspace_id),
            None,
        )
        .await?;
        if !bool_arg(args, "include_yaml").unwrap_or(true)
            && let Some(object) = value.as_object_mut()
        {
            object.remove("yaml");
        }
        return Ok(value);
    }
    let value = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/recipes", workspace_id),
        None,
    )
    .await?;
    Ok(value.get("recipes").cloned().unwrap_or(value))
}

async fn queue_control(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let action = required(args, "action")?;
    match action {
        "status" => {
            call(
                client,
                endpoint,
                Method::GET,
                &ws_path("/api/queue/status", workspace_id),
                None,
            )
            .await
        }
        "pause" => {
            let reason = optional_str(args, "reason").unwrap_or("pausada pelo agente");
            call(
                client,
                endpoint,
                Method::POST,
                &format!("/api/queue/pause?reason={}", urlencoding::encode(reason)),
                Some(json!({})),
            )
            .await
        }
        "resume" => {
            call(
                client,
                endpoint,
                Method::POST,
                "/api/queue/resume",
                Some(json!({})),
            )
            .await
        }
        "cancel" | "retry" => {
            let id = required(args, "job_id")?;
            let verb = if action == "cancel" {
                "cancel"
            } else {
                "retry"
            };
            call(
                client,
                endpoint,
                Method::POST,
                &format!("/api/queue/{id}/{verb}"),
                Some(json!({})),
            )
            .await
        }
        other => Err(anyhow!(
            "ação inválida '{other}'; use status, pause, resume, cancel ou retry"
        )),
    }
}

async fn artifact_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        return call(
            client,
            endpoint,
            Method::GET,
            &format!("/api/artifacts/{id}"),
            None,
        )
        .await;
    }
    let mut path = ws_path("/api/artifacts", workspace_id);
    for key in ["run_id", "task_id", "agent", "action"] {
        if let Some(value) = optional_str(args, key) {
            path.push('&');
            path.push_str(key);
            path.push('=');
            path.push_str(&urlencoding::encode(value));
        }
    }
    if let Some(limit) = integer_arg(args, "limit") {
        path.push('&');
        path.push_str(&format!("limit={}", limit.max(1)));
    }
    call(client, endpoint, Method::GET, &path, None).await
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
