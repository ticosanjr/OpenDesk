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
        "attention_list" => call(
            client,
            endpoint,
            Method::GET,
            &ws_path("/api/attention", workspace_id),
            None,
        )
        .await
        .map(attention_summary),
        _ => return None,
    })
}

pub(super) fn attention_summary(value: Value) -> Value {
    let count = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0)
    };
    json!({
        "awaiting_approval": count("awaiting_approval"),
        "blocked": count("blocked"),
        "failed_runs": count("failed_runs"),
        "requests": count("requests"),
        "hitl": count("hitl"),
        "proposals": count("proposals"),
        "items": value,
    })
}
