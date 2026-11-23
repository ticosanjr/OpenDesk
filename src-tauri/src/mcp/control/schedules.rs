use super::payload::{routine_body, routine_update_body};
use super::util::{bool_arg, optional_str, required, ws_path};
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
        "routine_list" => routine_list(client, endpoint, workspace_id, args).await,
        "routine_create" => routine_create(client, endpoint, workspace_id, args).await,
        "routine_update" => routine_update(client, endpoint, args).await,
        "routine_set_active" => routine_set_active(client, endpoint, args).await,
        "routine_run" => routine_run(client, endpoint, args).await,
        _ => return None,
    })
}

async fn routine_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        call(
            client,
            endpoint,
            Method::GET,
            &format!("/api/schedules/{id}"),
            None,
        )
        .await
    } else {
        call(
            client,
            endpoint,
            Method::GET,
            &ws_path("/api/schedules", workspace_id),
            None,
        )
        .await
    }
}

async fn routine_create(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let body = routine_body(workspace_id, args)?;
    call(
        client,
        endpoint,
        Method::POST,
        &ws_path("/api/schedules", workspace_id),
        Some(body),
    )
    .await
}

async fn routine_update(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let body = routine_update_body(args)?;
    call(
        client,
        endpoint,
        Method::PATCH,
        &format!("/api/schedules/{id}"),
        Some(body),
    )
    .await
}

async fn routine_set_active(
    client: &Client,
    endpoint: &ApiEndpoint,
    args: &Value,
) -> Result<Value> {
    let id = required(args, "id")?;
    let active =
        bool_arg(args, "active").ok_or_else(|| anyhow!("argumento 'active' é obrigatório"))?;
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/schedules/{id}/active"),
        Some(json!({ "active": active })),
    )
    .await
}

async fn routine_run(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let dry_run = bool_arg(args, "dry_run").unwrap_or(false);
    let path = if dry_run {
        format!("/api/schedules/{id}/dry-run")
    } else {
        format!("/api/schedules/{id}/run")
    };
    call(client, endpoint, Method::POST, &path, Some(json!({}))).await
}
