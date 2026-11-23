use super::util::{optional, required};
use crate::app_state::ApiEndpoint;
use crate::internal_api::call;
use anyhow::Result;
use reqwest::{Client, Method};
use serde_json::{Value, json};

pub(super) async fn dispatch(
    client: &Client,
    endpoint: &ApiEndpoint,
    _workspace_id: &str,
    _default_agent: &str,
    tool: &str,
    args: &Value,
) -> Option<Result<Value>> {
    Some(match tool {
        "chat_send_file" => chat_send_file(client, endpoint, args).await,
        _ => return None,
    })
}

async fn chat_send_file(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let thread_id = required(args, "thread_id")?;
    let body = json!({
        "path": required(args, "path")?,
        "caption": optional(args, "caption"),
    });
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/chat/threads/{thread_id}/files"),
        Some(body),
    )
    .await
}
