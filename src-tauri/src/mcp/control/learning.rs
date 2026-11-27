use super::payload::usage_scope_arg;
use super::util::{bool_arg, integer_arg, optional, required, ws_path};
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
        "learning_save" => learning_save(client, endpoint, workspace_id, args).await,
        "personal_context_save" | "personal_context_create" => {
            personal_context_create(client, endpoint, workspace_id, args).await
        }
        "personal_context_list" => {
            personal_context_list(client, endpoint, workspace_id, args).await
        }
        "personal_context_update" => {
            personal_context_update(client, endpoint, workspace_id, args).await
        }
        "personal_context_set_scope" => {
            personal_context_set_scope(client, endpoint, workspace_id, args).await
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

async fn personal_context_create(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let usage_scope = usage_scope_arg(args)?;
    let user_request = required(args, "user_request")?;
    let mut body = json!({
        "scope": optional(args, "scope").unwrap_or_else(|| "workspace".into()),
        "name": optional(args, "name"),
        "instructions": required(args, "instructions")?,
        "is_enabled": args.get("is_enabled").and_then(Value::as_bool).unwrap_or(true),
        "source_quote": user_request,
        "workspace_id": optional(args, "workspace_id").unwrap_or_else(|| workspace_id.to_string()),
    });
    if let Some(position) = integer_arg(args, "position") {
        body["position"] = json!(position);
    }
    if let Some(scope) = usage_scope {
        body["usage_scope"] = json!(scope);
    }
    call(
        client,
        endpoint,
        Method::POST,
        "/api/personal-context",
        Some(body),
    )
    .await
}

async fn personal_context_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let workspace = optional(args, "workspace_id").unwrap_or_else(|| workspace_id.to_string());
    call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/personal-context", &workspace),
        None,
    )
    .await
}

async fn personal_context_update(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let id = required(args, "id")?;
    let current = load_context(client, endpoint, workspace_id, id, args).await?;
    let usage_scope = usage_scope_arg(args)?;
    let mut body = merge_context_body(&current, args);
    body["usage_scope"] = json!(
        usage_scope
            .or_else(|| current
                .get("usage_scope")
                .cloned()
                .and_then(|value| value.as_str().map(str::to_string)))
            .unwrap_or_else(|| "both".into())
    );
    call(
        client,
        endpoint,
        Method::PUT,
        &format!("/api/personal-context/{}", urlencoding::encode(id)),
        Some(body),
    )
    .await
}

async fn personal_context_set_scope(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let id = required(args, "id")?;
    let scope = usage_scope_arg(args)?
        .ok_or_else(|| anyhow!("INVALID_PARAMS: usage_scope é obrigatório"))?;
    let current = load_context(client, endpoint, workspace_id, id, args).await?;
    let mut body = merge_context_body(&current, args);
    body["usage_scope"] = json!(scope);
    call(
        client,
        endpoint,
        Method::PUT,
        &format!("/api/personal-context/{}", urlencoding::encode(id)),
        Some(body),
    )
    .await
}

async fn load_context(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    id: &str,
    args: &Value,
) -> Result<Value> {
    let workspace = optional(args, "workspace_id").unwrap_or_else(|| workspace_id.to_string());
    let list = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/personal-context", &workspace),
        None,
    )
    .await?;
    list.as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
                .cloned()
        })
        .ok_or_else(|| anyhow!("contexto pessoal {id} não encontrado"))
}

fn merge_context_body(current: &Value, args: &Value) -> Value {
    let mut body = json!({
        "scope": current.get("scope").and_then(Value::as_str).unwrap_or("workspace"),
        "workspace_id": current.get("workspace_id").cloned().unwrap_or(Value::Null),
        "name": current.get("name").and_then(Value::as_str),
        "instructions": current.get("instructions").and_then(Value::as_str).unwrap_or_default(),
        "is_enabled": current.get("is_enabled").and_then(Value::as_bool).unwrap_or(true),
        "position": current.get("position").and_then(Value::as_i64),
        "source_quote": current.get("source_quote").and_then(Value::as_str),
    });
    for key in ["scope", "name", "instructions", "source_quote"] {
        if let Some(value) = optional(args, key) {
            body[key] = json!(value);
        }
    }
    if let Some(value) = args.get("is_enabled").and_then(Value::as_bool) {
        body["is_enabled"] = json!(value);
    }
    if let Some(value) = integer_arg(args, "position") {
        body["position"] = json!(value);
    }
    body
}
