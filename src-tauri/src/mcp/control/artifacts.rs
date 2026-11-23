use super::util::{bool_arg, integer_arg, optional_str, required, ws_path};
use crate::app_state::ApiEndpoint;
use crate::internal_api::call;
use anyhow::{Result, anyhow};
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
        "artifact_list" => artifact_list(client, endpoint, workspace_id, args).await,
        "artifact_read" => artifact_read(client, endpoint, args).await,
        "artifact_pin" => artifact_pin(client, endpoint, args).await,
        "artifact_rollback" => artifact_rollback(client, endpoint, args).await,
        _ => return None,
    })
}

async fn artifact_read(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let max_bytes = integer_arg(args, "max_bytes")
        .unwrap_or(65_536)
        .clamp(1024, 2 * 1024 * 1024);
    call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/artifacts/{id}/content?max_bytes={max_bytes}"),
        None,
    )
    .await
}

async fn artifact_pin(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let pinned =
        bool_arg(args, "pinned").ok_or_else(|| anyhow!("argumento 'pinned' é obrigatório"))?;
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/artifacts/{id}/pin"),
        Some(json!({ "pinned": pinned })),
    )
    .await
}

async fn artifact_rollback(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/artifacts/{id}/rollback"),
        Some(json!({})),
    )
    .await
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
