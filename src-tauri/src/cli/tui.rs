use std::io::{self, Write};

use crate::{app_state::ApiEndpoint, internal_api::call};
use anyhow::Result;
use reqwest::{Client, Method};
use serde_json::{Value, json};

use super::args::{Args, shorten};

pub(super) async fn run(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let workspace_id =
        resolve_workspace(client, endpoint, args.flags.get("workspace").cloned()).await?;
    run_tui(client, endpoint, workspace_id).await
}

async fn resolve_workspace(
    client: &Client,
    endpoint: &crate::app_state::ApiEndpoint,
    requested: Option<String>,
) -> Result<Option<String>> {
    if let Some(id) = requested {
        return Ok(Some(id));
    }
    let workspaces = call(client, endpoint, Method::GET, "/api/workspaces", None).await?;
    Ok(workspaces
        .as_array()
        .and_then(|list| {
            list.iter()
                .find(|workspace| {
                    workspace
                        .get("is_active")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .or_else(|| list.first())
        })
        .and_then(|workspace| workspace.get("id").and_then(Value::as_str))
        .map(str::to_string))
}

async fn run_tui(
    client: &Client,
    endpoint: &crate::app_state::ApiEndpoint,
    workspace_id: Option<String>,
) -> Result<()> {
    let workspace_query = workspace_id
        .as_deref()
        .map(|id| format!("?workspace_id={}", urlencoding::encode(id)))
        .unwrap_or_default();
    println!(
        "OpenDesk TUI — workspace {}",
        workspace_id.as_deref().unwrap_or("(nenhum)")
    );
    println!(
        "Comandos: tasks, runs, approvals, queue, nodes, diff <run>, approve <id>, deny <id>, help, exit"
    );
    loop {
        print!("opendesk> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, ' ');
        let command = parts.next().unwrap_or("");
        match command {
            "exit" | "quit" | "q" => break,
            "help" | "?" => {
                println!(
                    "tasks | runs | approvals | queue | nodes | diff <run> | approve <id> [texto] | deny <id> [texto] | exit"
                );
            }
            "tasks" => {
                let tasks = call(
                    client,
                    endpoint,
                    Method::GET,
                    &format!("/api/tasks{workspace_query}"),
                    None,
                )
                .await?;
                for task in tasks.as_array().into_iter().flatten() {
                    println!(
                        "{}  {:<12}  @{:<12}  {}",
                        shorten(task.get("id").and_then(Value::as_str).unwrap_or("?")),
                        task.get("workflow_state")
                            .or_else(|| task.get("status"))
                            .and_then(Value::as_str)
                            .unwrap_or("?"),
                        task.get("assigned_agent")
                            .and_then(Value::as_str)
                            .unwrap_or("-"),
                        task.get("title").and_then(Value::as_str).unwrap_or("")
                    );
                }
            }
            "runs" => {
                let limit = if workspace_query.is_empty() {
                    "?limit=20"
                } else {
                    "&limit=20"
                };
                let runs = call(
                    client,
                    endpoint,
                    Method::GET,
                    &format!("/api/runs{workspace_query}{limit}"),
                    None,
                )
                .await?;
                for run in runs.as_array().into_iter().flatten() {
                    println!(
                        "{}  {:<10}  {:<24}  US$ {:.4}  {}",
                        shorten(run.get("id").and_then(Value::as_str).unwrap_or("?")),
                        run.get("status").and_then(Value::as_str).unwrap_or("?"),
                        run.get("model_id").and_then(Value::as_str).unwrap_or("?"),
                        run.get("estimated_cost_usd")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0),
                        run.get("task_title").and_then(Value::as_str).unwrap_or("")
                    );
                }
            }
            "approvals" => {
                let list = call(
                    client,
                    endpoint,
                    Method::GET,
                    &format!("/api/interactions/pending{workspace_query}"),
                    None,
                )
                .await?;
                if list.as_array().map(Vec::is_empty).unwrap_or(true) {
                    println!("Nenhuma aprovação pendente.");
                }
                for item in list.as_array().into_iter().flatten() {
                    println!(
                        "{}  {:<10}  {}",
                        item.get("id").and_then(Value::as_str).unwrap_or("?"),
                        item.get("kind").and_then(Value::as_str).unwrap_or("?"),
                        item.get("question").and_then(Value::as_str).unwrap_or("")
                    );
                }
            }
            "queue" => {
                let jobs = call(
                    client,
                    endpoint,
                    Method::GET,
                    &format!("/api/queue{workspace_query}"),
                    None,
                )
                .await?;
                for job in jobs.as_array().into_iter().flatten() {
                    println!(
                        "{}  {:<10}  p{}  {}",
                        shorten(job.get("id").and_then(Value::as_str).unwrap_or("?")),
                        job.get("status").and_then(Value::as_str).unwrap_or("?"),
                        job.get("priority").and_then(Value::as_i64).unwrap_or(0),
                        job.get("title").and_then(Value::as_str).unwrap_or("")
                    );
                }
            }
            "nodes" => {
                let nodes = call(client, endpoint, Method::GET, "/api/nodes", None).await?;
                for node in nodes.as_array().into_iter().flatten() {
                    println!(
                        "{}  {:<8}  {:<18}  {}",
                        if node.get("online").and_then(Value::as_bool).unwrap_or(false) {
                            "online "
                        } else {
                            "offline"
                        },
                        node.get("name").and_then(Value::as_str).unwrap_or("?"),
                        node.get("status").and_then(Value::as_str).unwrap_or("?"),
                        node.get("endpoint").and_then(Value::as_str).unwrap_or("")
                    );
                }
            }
            "diff" => {
                let Some(run_id) = parts
                    .next()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    println!("uso: diff <run-id>");
                    continue;
                };
                let artifacts = call(
                    client,
                    endpoint,
                    Method::GET,
                    &format!("/api/artifacts?run_id={}", urlencoding::encode(run_id)),
                    None,
                )
                .await?;
                for artifact in artifacts.as_array().into_iter().flatten() {
                    println!(
                        "{}  {:<8}  {}\n",
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
            "approve" | "deny" => {
                let decision = if command == "approve" {
                    "approve"
                } else {
                    "deny"
                };
                let Some(rest) = parts.next() else {
                    println!("uso: {command} <id> [texto]");
                    continue;
                };
                let mut fields = rest.splitn(2, ' ');
                let Some(id) = fields.next().filter(|value| !value.is_empty()) else {
                    println!("uso: {command} <id> [texto]");
                    continue;
                };
                let text = fields.next().unwrap_or("").trim();
                call(
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
                println!("Aprovação {id} registrada como {decision}.");
            }
            other => println!("comando desconhecido: {other} (use help)"),
        }
    }
    Ok(())
}
