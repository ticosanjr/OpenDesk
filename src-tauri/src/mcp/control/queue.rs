use super::util::{optional_str, required, ws_path};
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
        "queue_control" => queue_control(client, endpoint, workspace_id, args).await,
        _ => return None,
    })
}

async fn queue_control(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let action = required(args, "action")?;
    match action {
        "status" => {
            call(
                client,
                endpoint,
                Method::GET,
                &ws_path("/api/queue/status", workspace_id),
                None,
            )
            .await
        }
        "pause" => {
            let reason = optional_str(args, "reason").unwrap_or("pausada pelo agente");
            call(
                client,
                endpoint,
                Method::POST,
                &format!("/api/queue/pause?reason={}", urlencoding::encode(reason)),
                Some(json!({})),
            )
            .await
        }
        "resume" => {
            call(
                client,
                endpoint,
                Method::POST,
                "/api/queue/resume",
                Some(json!({})),
            )
            .await
        }
        "cancel" | "retry" => {
            let id = required(args, "job_id")?;
            let verb = if action == "cancel" {
                "cancel"
            } else {
                "retry"
            };
            call(
                client,
                endpoint,
                Method::POST,
                &format!("/api/queue/{id}/{verb}"),
                Some(json!({})),
            )
            .await
        }
        other => Err(anyhow!(
            "ação inválida '{other}'; use status, pause, resume, cancel ou retry"
        )),
    }
}
