use crate::{app_state::ApiEndpoint, internal_api::call};
use anyhow::{Result, anyhow};
use reqwest::{Client, Method};
use serde_json::Value;

use super::args::{Args, shorten, workspace_query};

pub(super) async fn list(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let mut path = workspace_query(args);
    if let Some(limit) = args.flags.get("limit") {
        path.push(if path.contains('?') { '&' } else { '?' });
        path.push_str(&format!("limit={limit}"));
    }
    let runs = call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/runs{path}"),
        None,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&runs)?);
    } else if let Some(list) = runs.as_array() {
        for run in list {
            println!(
                "{:<9} {:<8} {:<18} {:<24} {}",
                run.get("status").and_then(Value::as_str).unwrap_or("?"),
                shorten(run.get("id").and_then(Value::as_str).unwrap_or("?")),
                run.get("agent_name").and_then(Value::as_str).unwrap_or("?"),
                run.get("model_id").and_then(Value::as_str).unwrap_or("?"),
                run.get("task_title").and_then(Value::as_str).unwrap_or("")
            );
        }
    }
    Ok(())
}

pub(super) async fn show(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let id = args
        .positionals
        .get(2)
        .ok_or_else(|| anyhow!("uso: opendesk runs show <run-id>"))?;
    let run = call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/runs/{id}"),
        None,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&run)?);
    } else {
        println!(
            "run      {}",
            run.get("id").and_then(Value::as_str).unwrap_or("?")
        );
        println!(
            "status   {}",
            run.get("status").and_then(Value::as_str).unwrap_or("?")
        );
        println!(
            "agente   {}",
            run.get("agent_name").and_then(Value::as_str).unwrap_or("?")
        );
        println!(
            "modelo   {}",
            run.get("model_id").and_then(Value::as_str).unwrap_or("?")
        );
        println!(
            "tokens   {} / {}",
            run.get("tokens_input").and_then(Value::as_i64).unwrap_or(0),
            run.get("tokens_output")
                .and_then(Value::as_i64)
                .unwrap_or(0)
        );
        println!(
            "custo    US$ {:.4}",
            run.get("estimated_cost_usd")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
        );
        if let Some(error) = run.get("error_message").and_then(Value::as_str) {
            println!("erro     {error}");
        }
    }
    Ok(())
}

pub(super) async fn queue_list(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let queue = call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/queue{}", workspace_query(args)),
        None,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&queue)?);
    } else if let Some(list) = queue.as_array() {
        for job in list {
            println!(
                "{:<10} {:<8} {:<8} {:<18} {}",
                job.get("status").and_then(Value::as_str).unwrap_or("?"),
                shorten(job.get("id").and_then(Value::as_str).unwrap_or("?")),
                job.get("job_kind").and_then(Value::as_str).unwrap_or("?"),
                job.get("resolved_agent")
                    .and_then(Value::as_str)
                    .unwrap_or("?"),
                job.get("title").and_then(Value::as_str).unwrap_or("")
            );
        }
    }
    Ok(())
}
