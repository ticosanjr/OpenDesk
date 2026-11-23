use super::util;
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
        "handoff_create" => handoff_create(client, endpoint, workspace_id, args).await,
        "handoff_list" => handoff_list(client, endpoint, workspace_id, args).await,
        "handoff_respond" => handoff_respond(client, endpoint, args).await,
        "handoff_finish" => handoff_finish(client, endpoint, args).await,
        _ => return None,
    })
}

async fn handoff_create(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let body = json!({
        "source_agent": required(args, "source_agent")?,
        "target_agent": required(args, "target_agent")?,
        "objective": required(args, "objective")?,
        "task_id": optional(args, "task_id"),
        "parent_handoff_id": optional(args, "parent_handoff_id"),
        "input_artifacts": util::string_array(args, "input_artifacts"),
        "output_schema": args.get("output_schema").cloned().unwrap_or(Value::Null),
        "acceptance": util::string_array(args, "acceptance"),
        "sub_budget_usd": args.get("sub_budget_usd").and_then(Value::as_f64),
    });
    call(
        client,
        endpoint,
        Method::POST,
        &ws_path("/api/handoffs", workspace_id),
        Some(body),
    )
    .await
}

async fn handoff_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let mut path = ws_path("/api/handoffs", workspace_id);
    if let Some(status) = optional(args, "status") {
        path.push_str(&format!("&status={}", urlencoding::encode(&status)));
    }
    if let Some(limit) = integer_arg(args, "limit") {
        path.push_str(&format!("&limit={limit}"));
    }
    call(client, endpoint, Method::GET, &path, None).await
}

async fn handoff_respond(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    match required(args, "decision")?.to_lowercase().as_str() {
        "accept" => {
            let body = json!({
                "note": optional(args, "note"),
                "force": bool_arg(args, "force").unwrap_or(false),
            });
            call(
                client,
                endpoint,
                Method::POST,
                &format!("/api/handoffs/{id}/accept"),
                Some(body),
            )
            .await
        }
        "reject" => {
            let body = json!({ "reason": optional(args, "reason") });
            call(
                client,
                endpoint,
                Method::POST,
                &format!("/api/handoffs/{id}/reject"),
                Some(body),
            )
            .await
        }
        other => Err(anyhow!("decisão inválida: {other}; use accept ou reject")),
    }
}

async fn handoff_finish(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let body = json!({
        "status": required(args, "status")?,
        "result": args.get("result").cloned().unwrap_or(Value::Null),
    });
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/handoffs/{id}/finish"),
        Some(body),
    )
    .await
}
