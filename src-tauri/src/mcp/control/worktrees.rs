use super::util::{bool_arg, optional, required, ws_path};
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
        "worktree_list" => worktree_list(client, endpoint, workspace_id).await,
        "worktree_prepare" => worktree_prepare(client, endpoint, workspace_id, args).await,
        "worktree_diff" => worktree_diff(client, endpoint, args).await,
        "worktree_merge" => worktree_merge(client, endpoint, args).await,
        "worktree_cleanup" => worktree_cleanup(client, endpoint, args).await,
        _ => return None,
    })
}

async fn worktree_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
) -> Result<Value> {
    call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/worktrees", workspace_id),
        None,
    )
    .await
}

async fn worktree_prepare(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    call(
        client,
        endpoint,
        Method::POST,
        &ws_path("/api/worktrees", workspace_id),
        Some(json!({ "run_id": optional(args, "run_id") })),
    )
    .await
}

async fn worktree_diff(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/worktrees/{id}/diff"),
        None,
    )
    .await
}

async fn worktree_merge(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    let body = json!({ "approved": bool_arg(args, "approved").unwrap_or(false) });
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/worktrees/{id}/merge"),
        Some(body),
    )
    .await
}

async fn worktree_cleanup(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    call(
        client,
        endpoint,
        Method::DELETE,
        &format!("/api/worktrees/{id}"),
        None,
    )
    .await
}
