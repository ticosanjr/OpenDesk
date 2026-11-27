use super::payload::usage_scope_arg;
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
        "memory_save" | "memory_create" => {
            memory_save(client, endpoint, workspace_id, default_agent, args).await
        }
        "memory_search" => memory_search(client, endpoint, workspace_id, default_agent, args).await,
        "memory_list" => memory_list(client, endpoint, workspace_id, default_agent, args).await,
        "memory_update" => memory_update(client, endpoint, args).await,
        "memory_set_scope" => memory_set_scope(client, endpoint, args).await,
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
    let usage_scope = usage_scope_arg(args)?;
    let mut path = ws_path("/api/memory/notes", workspace_id);
    path.push_str(&format!("&agent={}", urlencoding::encode(&agent)));
    let mut body = json!({
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
    if let Some(scope) = usage_scope {
        body["usage_scope"] = json!(scope);
    }
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
    if let Some(scope) = usage_scope_arg(args)? {
        path.push_str(&format!("&usage_scope={}", urlencoding::encode(&scope)));
    }
    if let Some(limit) = integer_arg(args, "limit") {
        path.push_str(&format!("&limit={limit}"));
    }
    call(client, endpoint, Method::GET, &path, None).await
}

async fn memory_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    default_agent: &str,
    args: &Value,
) -> Result<Value> {
    let agent = optional(args, "agent").unwrap_or_else(|| default_agent.to_string());
    let mut path = ws_path("/api/memory/notes", workspace_id);
    path.push_str(&format!("&agent={}", urlencoding::encode(&agent)));
    if let Some(namespace) = optional(args, "namespace") {
        path.push_str(&format!("&namespace={}", urlencoding::encode(&namespace)));
    }
    if let Some(scope) = usage_scope_arg(args)? {
        path.push_str(&format!("&usage_scope={}", urlencoding::encode(&scope)));
    }
    if let Some(limit) = integer_arg(args, "limit") {
        path.push_str(&format!("&limit={limit}"));
    }
    call(client, endpoint, Method::GET, &path, None).await
}

async fn memory_update(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let mut body = serde_json::Map::new();
    for key in ["title", "body", "expires_at"] {
        if let Some(value) = util::optional_str(args, key) {
            body.insert(key.into(), json!(value));
        }
    }
    if args.get("tags").is_some() {
        body.insert("tags".into(), json!(util::string_array(args, "tags")));
    }
    if let Some(confidence) = args.get("confidence").and_then(Value::as_f64) {
        body.insert("confidence".into(), json!(confidence));
    }
    if let Some(trust) = util::optional_str(args, "trust_level") {
        body.insert("trust_level".into(), json!(trust));
    }
    if let Some(clear) = args.get("clear_expiry").and_then(Value::as_bool) {
        body.insert("clear_expiry".into(), json!(clear));
    }
    if let Some(scope) = usage_scope_arg(args)? {
        body.insert("usage_scope".into(), json!(scope));
    }
    call(
        client,
        endpoint,
        Method::PATCH,
        &format!("/api/memory/notes/{}", urlencoding::encode(id)),
        Some(Value::Object(body)),
    )
    .await
}

async fn memory_set_scope(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let scope = usage_scope_arg(args)?
        .ok_or_else(|| anyhow::anyhow!("INVALID_PARAMS: usage_scope é obrigatório"))?;
    call(
        client,
        endpoint,
        Method::PATCH,
        &format!("/api/memory/notes/{}", urlencoding::encode(id)),
        Some(json!({ "usage_scope": scope })),
    )
    .await
}
