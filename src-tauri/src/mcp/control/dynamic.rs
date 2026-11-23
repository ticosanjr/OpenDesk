use super::util::{bool_arg, optional, required, ws_path};
use crate::app_state::ApiEndpoint;
use crate::internal_api::call;
use anyhow::Result;
use reqwest::{Client, Method};
use serde_json::{Value, json};

pub(super) async fn run_dynamic_recipe(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    id: &str,
    args: &Value,
) -> Result<Value> {
    let trigger = args.get("trigger").cloned().unwrap_or_else(|| args.clone());
    let dry_run = bool_arg(args, "dry_run").unwrap_or(false);
    call(
        client,
        endpoint,
        Method::POST,
        &ws_path(
            &format!("/api/recipes/{}/run", urlencoding::encode(id)),
            workspace_id,
        ),
        Some(json!({
            "trigger_context": trigger,
            "dry_run": dry_run,
        })),
    )
    .await
}

pub(super) async fn run_dynamic_agent(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    id: &str,
    args: &Value,
) -> Result<Value> {
    let prompt = required(args, "prompt")?;
    let title = optional(args, "title").unwrap_or_else(|| prompt.chars().take(60).collect());
    let created = call(
        client,
        endpoint,
        Method::POST,
        &ws_path("/api/tasks", workspace_id),
        Some(json!({
            "title": title,
            "description": prompt,
            "assigned_agent": id,
            "preferred_model": optional(args, "model"),
            "execution_mode": optional(args, "mode").unwrap_or_else(|| "AUTO_DSH".into()),
            "origin": "MANUAL",
        })),
    )
    .await?;
    let task_id = created
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut result = json!({ "agent": id, "task": created });
    if let Some(task_id) = task_id {
        match call(
            client,
            endpoint,
            Method::POST,
            &format!("/api/tasks/{task_id}/dispatch"),
            Some(json!({ "manual_agent": id })),
        )
        .await
        {
            Ok(dispatch) => result["dispatch"] = dispatch,
            Err(error) => result["dispatch_error"] = json!(format!("{error:#}")),
        }
    }
    Ok(result)
}
