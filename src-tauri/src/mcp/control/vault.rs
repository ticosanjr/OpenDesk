use super::util::{integer_arg, optional, required, ws_path};
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
        "vault_list_refs" => list(client, endpoint, workspace_id).await,
        "vault_request_fill" => request_fill(client, endpoint, args).await,
        _ => return None,
    })
}

async fn list(client: &Client, endpoint: &ApiEndpoint, workspace_id: &str) -> Result<Value> {
    let value = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/vault", workspace_id),
        None,
    )
    .await?;
    Ok(value.get("entries").cloned().unwrap_or(value))
}

async fn request_fill(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let entry_id = required(args, "entry_id")?;
    let task_id = required(args, "task_id")?;
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/vault/{entry_id}/request-fill"),
        Some(json!({
            "task_id": task_id,
            "reason": optional(args, "reason"),
            "timeout_minutes": integer_arg(args, "timeout_minutes").unwrap_or(10),
        })),
    )
    .await
}
