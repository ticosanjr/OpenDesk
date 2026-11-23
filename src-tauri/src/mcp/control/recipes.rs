use super::util::{bool_arg, optional, optional_str, required, ws_path};
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
        "recipe_list" => recipe_list(client, endpoint, workspace_id, args).await,
        "recipe_save" => recipe_save(client, endpoint, workspace_id, args).await,
        "recipe_set_enabled" => recipe_set_enabled(client, endpoint, workspace_id, args).await,
        "recipe_run" => recipe_run(client, endpoint, workspace_id, args).await,
        "recipe_resume" => recipe_resume(client, endpoint, args).await,
        "recipe_cancel" => recipe_cancel(client, endpoint, args).await,
        "recipe_versions" => recipe_versions(client, endpoint, workspace_id, args).await,
        "recipe_diff" => recipe_diff(client, endpoint, workspace_id, args).await,
        "recipe_rollback" => recipe_rollback(client, endpoint, workspace_id, args).await,
        _ => return None,
    })
}

async fn recipe_save(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let id = required(args, "id")?;
    let yaml = required(args, "yaml")?;
    call(
        client,
        endpoint,
        Method::PUT,
        &ws_path(&format!("/api/recipes/{id}"), workspace_id),
        Some(json!({ "yaml": yaml })),
    )
    .await
}

async fn recipe_set_enabled(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let id = required(args, "id")?;
    let enabled =
        bool_arg(args, "enabled").ok_or_else(|| anyhow!("argumento 'enabled' é obrigatório"))?;
    call(
        client,
        endpoint,
        Method::POST,
        &ws_path(&format!("/api/recipes/{id}/enabled"), workspace_id),
        Some(json!({ "enabled": enabled })),
    )
    .await
}

async fn recipe_run(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let id = required(args, "id")?;
    let body = json!({
        "trigger_context": args
            .get("trigger_context")
            .cloned()
            .filter(|value| !value.is_null())
            .unwrap_or_else(|| json!({ "manual": true })),
        "dry_run": bool_arg(args, "dry_run").unwrap_or(false),
    });
    call(
        client,
        endpoint,
        Method::POST,
        &ws_path(&format!("/api/recipes/{id}/run"), workspace_id),
        Some(body),
    )
    .await
}

async fn recipe_resume(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/recipes/runs/{id}/retry"),
        None,
    )
    .await
}

async fn recipe_cancel(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let id = required(args, "id")?;
    call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/recipes/runs/{id}/cancel"),
        None,
    )
    .await
}

async fn recipe_versions(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let id = required(args, "id")?;
    call(
        client,
        endpoint,
        Method::GET,
        &ws_path(&format!("/api/recipes/{id}/versions"), workspace_id),
        None,
    )
    .await
}

async fn recipe_diff(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let id = required(args, "id")?;
    let version = required(args, "version_id")?;
    let mut path = ws_path(
        &format!("/api/recipes/{id}/versions/{version}/diff"),
        workspace_id,
    );
    if let Some(against) = optional(args, "against") {
        path.push_str(&format!("&against={}", urlencoding::encode(&against)));
    }
    call(client, endpoint, Method::GET, &path, None).await
}

async fn recipe_rollback(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let id = required(args, "id")?;
    let version_id = required(args, "version_id")?;
    call(
        client,
        endpoint,
        Method::POST,
        &ws_path(&format!("/api/recipes/{id}/rollback"), workspace_id),
        Some(json!({ "version_id": version_id })),
    )
    .await
}

async fn recipe_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        let mut value = call(
            client,
            endpoint,
            Method::GET,
            &ws_path(&format!("/api/recipes/{id}"), workspace_id),
            None,
        )
        .await?;
        if !bool_arg(args, "include_yaml").unwrap_or(true)
            && let Some(object) = value.as_object_mut()
        {
            object.remove("yaml");
        }
        return Ok(value);
    }
    let value = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/recipes", workspace_id),
        None,
    )
    .await?;
    Ok(value.get("recipes").cloned().unwrap_or(value))
}
