use crate::{app_state::ApiEndpoint, internal_api::call};
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};

use super::args::{Args, shorten, workspace_query};

pub(super) async fn status(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let health = call(client, endpoint, Method::GET, "/api/health", None).await?;
    let queue = call(client, endpoint, Method::GET, "/api/queue/status", None).await?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "health": health, "queue": queue }))?
        );
    } else {
        println!(
            "OpenDesk {} · daemon {} · {} aguardando · {} executando{}",
            health
                .pointer("/version")
                .and_then(Value::as_str)
                .unwrap_or("?"),
            health
                .pointer("/daemon/state")
                .and_then(Value::as_str)
                .unwrap_or("?"),
            queue.get("waiting").and_then(Value::as_i64).unwrap_or(0),
            queue.get("running").and_then(Value::as_i64).unwrap_or(0),
            queue
                .get("paused")
                .and_then(Value::as_bool)
                .filter(|paused| *paused)
                .map(|_| " · PAUSADA")
                .unwrap_or("")
        );
    }
    Ok(())
}

pub(super) async fn nodes_list(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let nodes = call(client, endpoint, Method::GET, "/api/nodes", None).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&nodes)?);
    } else if let Some(list) = nodes.as_array() {
        if list.is_empty() {
            println!("Nenhum nó remoto registrado.");
        }
        for node in list {
            println!(
                "{}  {:<18}  {:<8}  {}",
                node.get("online")
                    .and_then(Value::as_bool)
                    .map(|online| if online { "online " } else { "offline" })
                    .unwrap_or("?"),
                node.get("name").and_then(Value::as_str).unwrap_or("?"),
                node.get("status").and_then(Value::as_str).unwrap_or("?"),
                node.get("endpoint").and_then(Value::as_str).unwrap_or("")
            );
        }
    }
    Ok(())
}

pub(super) async fn workspaces_list(
    args: &Args,
    client: &Client,
    endpoint: &ApiEndpoint,
) -> Result<()> {
    let workspaces = call(client, endpoint, Method::GET, "/api/workspaces", None).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&workspaces)?);
    } else if let Some(list) = workspaces.as_array() {
        for workspace in list {
            println!(
                "{}  {}  {}",
                shorten(workspace.get("id").and_then(Value::as_str).unwrap_or("?")),
                workspace.get("name").and_then(Value::as_str).unwrap_or("?"),
                workspace
                    .get("root_path")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            );
        }
    }
    Ok(())
}

pub(super) async fn approvals_list(
    args: &Args,
    client: &Client,
    endpoint: &ApiEndpoint,
) -> Result<()> {
    let interactions = call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/interactions/pending{}", workspace_query(args)),
        None,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&interactions)?);
    } else if let Some(list) = interactions.as_array() {
        if list.is_empty() {
            println!("Nenhuma aprovação pendente.");
        }
        for item in list {
            println!(
                "{}  {:<10}  {}",
                item.get("id").and_then(Value::as_str).unwrap_or("?"),
                item.get("kind").and_then(Value::as_str).unwrap_or("TEXT"),
                item.get("question").and_then(Value::as_str).unwrap_or("")
            );
        }
    }
    Ok(())
}

pub(super) async fn approvals_resolve(
    args: &Args,
    client: &Client,
    endpoint: &ApiEndpoint,
) -> Result<()> {
    let id = args
        .positionals
        .get(2)
        .context("informe o id da aprovação")?;
    let decision = args
        .positionals
        .get(3)
        .map(|value| value.to_lowercase())
        .unwrap_or_else(|| "approve".into());
    let decision = match decision.as_str() {
        "approve" | "aprovar" | "yes" | "sim" => "approve",
        "deny" | "negar" | "no" | "não" | "nao" => "deny",
        other => return Err(anyhow!("decisão inválida: {other}; use approve ou deny")),
    };
    let text = args
        .positionals
        .iter()
        .skip(4)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let result = call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/interactions/{id}/resolve"),
        Some(json!({
            "decision": decision,
            "text": (!text.is_empty()).then_some(text),
        })),
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("Aprovação {} registrada.", id);
    }
    Ok(())
}

pub(super) async fn diff(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let run_id = args.positionals.get(1).context("informe o id do run")?;
    let artifacts = call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/artifacts?run_id={}", urlencoding::encode(run_id)),
        None,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&artifacts)?);
    } else if let Some(list) = artifacts.as_array() {
        if list.is_empty() {
            println!("Nenhum artefato registrado para o run.");
        }
        for artifact in list {
            println!(
                "{}  {:<8}  {}",
                artifact
                    .get("action_type")
                    .and_then(Value::as_str)
                    .unwrap_or("?"),
                artifact
                    .get("file_path")
                    .and_then(Value::as_str)
                    .unwrap_or("?"),
                artifact
                    .get("diff_patch")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            );
        }
    }
    Ok(())
}
