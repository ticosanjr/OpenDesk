use super::util::required;
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
        "voice_speak" => voice_speak(client, endpoint, workspace_id, args).await,
        _ => return None,
    })
}

async fn voice_speak(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let body = json!({
        "text": required(args, "text")?,
        "workspace_id": workspace_id,
    });
    call(
        client,
        endpoint,
        Method::POST,
        "/api/voice/speak",
        Some(body),
    )
    .await
}
