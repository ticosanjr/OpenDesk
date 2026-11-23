use super::util::{bool_arg, optional, required};
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
        "learning_save" => learning_save(client, endpoint, _workspace_id, args).await,
        "personal_context_save" => {
            personal_context_save(client, endpoint, _workspace_id, args).await
        }
        _ => return None,
    })
}

async fn learning_save(
    client: &Client,
    endpoint: &ApiEndpoint,
    _workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let body = json!({
        "title": required(args, "title")?,
        "detail": required(args, "detail")?,
        "category": optional(args, "category"),
        "severity": optional(args, "severity"),
        "agent_name": optional(args, "agent_name"),
        "remember": bool_arg(args, "remember").unwrap_or(false),
        "workspace_id": optional(args, "workspace_id"),
    });
    call(
        client,
        endpoint,
        Method::POST,
        "/api/learning/lessons",
        Some(body),
    )
    .await
}

async fn personal_context_save(
    client: &Client,
    endpoint: &ApiEndpoint,
    _workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let body = json!({
        "scope": optional(args, "scope").unwrap_or_else(|| "workspace".into()),
        "name": optional(args, "name"),
        "instructions": required(args, "instructions")?,
        "user_request": required(args, "user_request")?,
        "is_enabled": true,
        "source_quote": required(args, "user_request")?,
        "workspace_id": optional(args, "workspace_id"),
    });
    call(
        client,
        endpoint,
        Method::POST,
        "/api/personal-context",
        Some(body),
    )
    .await
}
