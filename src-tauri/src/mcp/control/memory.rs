use super::util;
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
    default_agent: &str,
    tool: &str,
    args: &Value,
) -> Option<Result<Value>> {
    Some(match tool {
        "memory_save" => memory_save(client, endpoint, workspace_id, default_agent, args).await,
        "memory_search" => memory_search(client, endpoint, workspace_id, default_agent, args).await,
        _ => return None,
    })
}

async fn memory_save(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    default_agent: &str,
    args: &Value,
) -> Result<Value> {
    let agent = optional(args, "agent").unwrap_or_else(|| default_agent.to_string());
    let mut path = ws_path("/api/memory/notes", workspace_id);
    path.push_str(&format!("&agent={}", urlencoding::encode(&agent)));
    let body = json!({
        "namespace": optional(args, "namespace").unwrap_or_else(|| "core".into()),
        "kind": optional(args, "kind").unwrap_or_else(|| "semantic".into()),
        "title": required(args, "title")?,
        "body": required(args, "body")?,
        "tags": util::string_array(args, "tags"),
        "confidence": args.get("confidence").and_then(Value::as_f64).unwrap_or(0.6),
        "trust_level": optional(args, "trust_level").unwrap_or_else(|| "observed".into()),
        "expires_at": optional(args, "expires_at"),
        "source": optional(args, "source"),
    });
    call(client, endpoint, Method::POST, &path, Some(body)).await
}

async fn memory_search(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    default_agent: &str,
    args: &Value,
) -> Result<Value> {
    let agent = optional(args, "agent").unwrap_or_else(|| default_agent.to_string());
    let query = required(args, "query")?;
    let mut path = ws_path("/api/memory/search", workspace_id);
    path.push_str(&format!(
        "&q={}&agent={}",
        urlencoding::encode(query),
        urlencoding::encode(&agent)
    ));
    if let Some(namespace) = optional(args, "namespace") {
        path.push_str(&format!("&namespace={}", urlencoding::encode(&namespace)));
    }
    if let Some(limit) = integer_arg(args, "limit") {
        path.push_str(&format!("&limit={limit}"));
    }
    call(client, endpoint, Method::GET, &path, None).await
}
