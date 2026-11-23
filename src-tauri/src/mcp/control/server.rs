use super::attention::attention_summary;
use super::util::ws_path;
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
    _args: &Value,
) -> Option<Result<Value>> {
    Some(match tool {
        "opendesk_status" => status(client, endpoint, workspace_id).await,
        _ => return None,
    })
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
