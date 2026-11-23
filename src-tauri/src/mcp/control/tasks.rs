use super::payload::{task_create_body, task_update_body};
use super::util::{bool_arg, integer_arg, optional, optional_str, required, ws_path};
use crate::app_state::ApiEndpoint;
use crate::internal_api::call;
use anyhow::Result;
use reqwest::{Client, Method};
use serde_json::{Value, json};

pub(super) async fn dispatch(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    _default_agent: &str,
    tool: &str,
    args: &Value,
) -> Option<Result<Value>> {
    Some(match tool {
        "task_list" => task_list(client, endpoint, workspace_id, args).await,
        "task_create" => task_create(client, endpoint, workspace_id, args).await,
        "task_update" => task_update(client, endpoint, args).await,
        "task_move" => task_move(client, endpoint, args).await,
        "task_dispatch" => task_dispatch(client, endpoint, args).await,
        "task_message_list" => task_message_list(client, endpoint, args).await,
        "task_message_post" => task_message_post(client, endpoint, args).await,
        "run_list" => run_list(client, endpoint, workspace_id, args).await,
        _ => return None,
    })
}

async fn task_create(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let body = task_create_body(args)?;
    call(
        client,
        endpoint,
        Method::POST,
        &ws_path("/api/tasks", workspace_id),
        Some(body),
    )
    .await
}

async fn task_update(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let body = task_update_body(args)?;
    call(
        client,
        endpoint,
        Method::PATCH,
        &format!("/api/tasks/{id}"),
        Some(body),
    )
    .await
}

async fn task_move(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let body = json!({
        "status": required(args, "status")?,
        "reason": optional(args, "reason"),
    });
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/tasks/{id}/transition"),
        Some(body),
    )
    .await
}

async fn task_dispatch(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let body = json!({
        "policy": optional(args, "policy"),
        "manual_agent": optional(args, "manual_agent"),
        "manual_model": optional(args, "manual_model"),
        "manual_effort": optional(args, "manual_effort"),
        "execution_mode": optional(args, "execution_mode"),
    });
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/tasks/{id}/dispatch"),
        Some(body),
    )
    .await
}

async fn task_message_list(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/tasks/{id}/messages"),
        None,
    )
    .await
}

async fn task_message_post(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let body = json!({
        "content": required(args, "content")?,
        "run_id": optional(args, "run_id"),
        "continue_execution": bool_arg(args, "continue_execution").unwrap_or(false),
        "agent": optional(args, "agent"),
        "model": optional(args, "model"),
    });
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/tasks/{id}/messages"),
        Some(body),
    )
    .await
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
