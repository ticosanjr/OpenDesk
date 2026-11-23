use crate::{app_state::ApiEndpoint, internal_api::call};
use anyhow::{Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};

use super::args::{Args, shorten, workspace_query};

pub(super) async fn list(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let tasks = call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/tasks{}", workspace_query(args)),
        None,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&tasks)?);
    } else if let Some(list) = tasks.as_array() {
        for task in list {
            println!(
                "{:<11} {:<8} {:<10} {}",
                task.get("status").and_then(Value::as_str).unwrap_or("?"),
                shorten(task.get("id").and_then(Value::as_str).unwrap_or("?")),
                task.get("origin")
                    .and_then(Value::as_str)
                    .unwrap_or("MANUAL"),
                task.get("title").and_then(Value::as_str).unwrap_or("?")
            );
        }
    }
    Ok(())
}

pub(super) async fn create(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let title = args
        .flags
        .get("title")
        .ok_or_else(|| anyhow!("--title é obrigatório"))?;
    let description = args
        .flags
        .get("description")
        .ok_or_else(|| anyhow!("--description é obrigatório"))?;
    let tags = args
        .flags
        .get("tags")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let task = call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/tasks{}", workspace_query(args)),
        Some(json!({
            "title": title,
            "description": description,
            "execution_mode": args.flags.get("mode").cloned().unwrap_or_else(|| "AUTO_DSH".into()),
            "assigned_agent": args.flags.get("agent").cloned(),
            "preferred_model": args.flags.get("model").cloned(),
            "priority": args.flags.get("priority").and_then(|value| value.parse::<i64>().ok()),
            "tags": tags,
        })),
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&task)?);
    } else {
        println!(
            "{}  {}  {}",
            shorten(task.get("id").and_then(Value::as_str).unwrap_or("?")),
            task.get("status").and_then(Value::as_str).unwrap_or("?"),
            task.get("title").and_then(Value::as_str).unwrap_or("?")
        );
        if task
            .get("deduplicated")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            eprintln!("nota: tarefa idêntica já existia; reutilizando a existente");
        }
    }
    Ok(())
}

pub(super) async fn run(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let id = args
        .positionals
        .get(2)
        .ok_or_else(|| anyhow!("uso: opendesk tasks run <task-id>"))?;
    let result = call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/tasks/{id}/dispatch"),
        Some(json!({})),
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "job {} · {}",
            shorten(
                result
                    .pointer("/job/id")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
            ),
            result
                .pointer("/job/status")
                .and_then(Value::as_str)
                .unwrap_or("?")
        );
    }
    Ok(())
}
