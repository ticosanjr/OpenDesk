//! Minimal headless CLI. It talks to the running local API; it never opens the
//! GUI or duplicates execution logic.

use crate::internal_api::{call, endpoint};
use crate::opencode_client::rest_client::{ModelRefDto, RestClient};
use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const HELP: &str = "OpenDesk CLI

Uso:
  opendesk status [--json]
  opendesk workspaces list [--json]
  opendesk tasks list [--workspace <id>] [--json]
  opendesk tasks create --title <título> --description <descrição>
        [--agent <id>] [--model <ref>] [--mode <modo>] [--priority <1-4>]
        [--tags a,b] [--workspace <id>] [--json]
  opendesk tasks run <task-id> [--json]
  opendesk chat [mensagem] [--workspace <id>] [--thread <id>] [--agent <id>] [--json]
  opendesk approvals list [--workspace <id>] [--json]
  opendesk approvals resolve <id> approve|deny [texto] [--json]
  opendesk diff <run-id> [--json]
  opendesk nodes list [--json]
  opendesk tui [--workspace <id>]
  opendesk node run --central <url> --token <token> [--once] [--poll <segundos>]
        [--opencode-url <url>] [--opencode-password <senha>]
        [--sync] [--workdir <diretório>]
  opendesk runs list [--workspace <id>] [--limit <n>] [--json]
  opendesk runs show <run-id> [--json]
  opendesk queue list [--workspace <id>] [--json]
  opendesk backups list [--workspace <id>] [--json]
  opendesk backups create [--workspace <id>] [--json]
  opendesk backups restore <backup-id> --path <diretório> [--name <nome>]
  opendesk backups download <backup-id> --out <arquivo>
  opendesk backups delete <backup-id>
  opendesk export <run-id> [--format md|html|json] [--out <arquivo>]

O servidor local precisa estar rodando (app desktop ou --headless).";

#[derive(Default)]
struct Args {
    positionals: Vec<String>,
    flags: HashMap<String, String>,
    json: bool,
    help: bool,
}

fn parse(args: &[String]) -> Args {
    let mut parsed = Args::default();
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--json" {
            parsed.json = true;
        } else if argument == "--help" || argument == "-h" {
            parsed.help = true;
        } else if let Some(rest) = argument.strip_prefix("--") {
            if let Some((key, value)) = rest.split_once('=') {
                parsed.flags.insert(key.to_string(), value.to_string());
            } else if index + 1 < args.len() && !args[index + 1].starts_with("--") {
                parsed
                    .flags
                    .insert(rest.to_string(), args[index + 1].clone());
                index += 1;
            } else {
                parsed.flags.insert(rest.to_string(), "true".into());
            }
        } else {
            parsed.positionals.push(argument.clone());
        }
        index += 1;
    }
    parsed
}

fn shorten(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn workspace_query(args: &Args) -> String {
    args.flags
        .get("workspace")
        .map(|id| format!("?workspace_id={}", urlencoding::encode(id)))
        .unwrap_or_default()
}

pub fn run(args: Vec<String>) {
    let parsed = parse(&args);
    if args.is_empty() || parsed.help {
        println!("{HELP}");
        return;
    }
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("opendesk: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = runtime.block_on(execute(parsed)) {
        eprintln!("opendesk: {error:#}");
        std::process::exit(1);
    }
}

async fn execute(args: Args) -> Result<()> {
    let endpoint = endpoint()?;
    let client = Client::new();
    let command = args
        .positionals
        .first()
        .map(String::as_str)
        .unwrap_or("status");
    let sub = args.positionals.get(1).map(String::as_str);

    match (command, sub) {
        ("status", _) => {
            let health = call(&client, &endpoint, Method::GET, "/api/health", None).await?;
            let queue = call(&client, &endpoint, Method::GET, "/api/queue/status", None).await?;
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
        }
        ("node", Some("run")) => {
            let central = args
                .flags
                .get("central")
                .cloned()
                .context("informe --central com a URL do control plane")?;
            let token = args
                .flags
                .get("token")
                .cloned()
                .or_else(|| std::env::var("OPENDESK_NODE_TOKEN").ok())
                .context("informe --token ou defina OPENDESK_NODE_TOKEN")?;
            let poll = args
                .flags
                .get("poll")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(10)
                .max(2);
            let opencode_url = args
                .flags
                .get("opencode-url")
                .cloned()
                .or_else(|| std::env::var("OPENDESK_OPENCODE_URL").ok());
            let opencode_password = args
                .flags
                .get("opencode-password")
                .cloned()
                .or_else(|| std::env::var("OPENDESK_OPENCODE_PASSWORD").ok());
            run_node(
                &central,
                &token,
                args.flags.contains_key("once"),
                poll,
                opencode_url,
                opencode_password,
                args.flags.contains_key("sync"),
                args.flags.get("workdir").cloned(),
            )
            .await?;
        }
        ("tui", _) => {
            let workspace_id =
                resolve_workspace(&client, &endpoint, args.flags.get("workspace").cloned()).await?;
            run_tui(&client, &endpoint, workspace_id).await?;
        }
        ("nodes", Some("list")) => {
            let nodes = call(&client, &endpoint, Method::GET, "/api/nodes", None).await?;
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
        }
        ("workspaces", Some("list")) => {
            let workspaces = call(&client, &endpoint, Method::GET, "/api/workspaces", None).await?;
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
        }
        ("approvals", Some("list")) => {
            let interactions = call(
                &client,
                &endpoint,
                Method::GET,
                &format!("/api/interactions/pending{}", workspace_query(&args)),
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
        }
        ("approvals", Some("resolve")) => {
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
                &client,
                &endpoint,
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
        }
        ("diff", _) => {
            let run_id = args.positionals.get(1).context("informe o id do run")?;
            let artifacts = call(
                &client,
                &endpoint,
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
        }
        ("chat", _) => {
            let workspace_id = match args.flags.get("workspace") {
                Some(value) => Some(value.clone()),
                None => {
                    let workspaces =
                        call(&client, &endpoint, Method::GET, "/api/workspaces", None).await?;
                    workspaces
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
                        .and_then(|workspace| {
                            workspace
                                .get("id")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        })
                }
            };
            let agent = args
                .flags
                .get("agent")
                .cloned()
                .unwrap_or_else(|| "build".into());
            let mut thread_id = args.flags.get("thread").cloned();
            if thread_id.is_none() {
                let workspace_id =
                    workspace_id.context("nenhum workspace ativo para iniciar o chat")?;
                let created = call(
                    &client,
                    &endpoint,
                    Method::POST,
                    "/api/chat/threads",
                    Some(json!({
                        "workspace_id": workspace_id,
                        "title": "CLI chat",
                        "agent": agent,
                        "source": "desktop"
                    })),
                )
                .await?;
                thread_id = created
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            let thread_id = thread_id.context("a API não devolveu a conversa")?;
            let send = |message: String| {
                let client = client.clone();
                let endpoint = endpoint.clone();
                let thread_id = thread_id.clone();
                async move {
                    call(
                        &client,
                        &endpoint,
                        Method::POST,
                        &format!("/api/chat/threads/{thread_id}/messages"),
                        Some(json!({ "content": message })),
                    )
                    .await
                }
            };
            let initial = args
                .positionals
                .iter()
                .skip(1)
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            if !initial.trim().is_empty() {
                let reply = send(initial).await?;
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&reply)?);
                } else {
                    println!(
                        "{}",
                        reply
                            .pointer("/assistant/content")
                            .and_then(Value::as_str)
                            .unwrap_or("(sem resposta)")
                    );
                }
            } else {
                println!("Chat {thread_id}. Digite /exit para sair.");
                loop {
                    print!("> ");
                    io::stdout().flush()?;
                    let mut line = String::new();
                    if io::stdin().read_line(&mut line)? == 0 {
                        break;
                    }
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if line == "/exit" {
                        break;
                    }
                    let reply = send(line.to_string()).await?;
                    if args.json {
                        println!("{}", serde_json::to_string_pretty(&reply)?);
                    } else {
                        println!(
                            "{}",
                            reply
                                .pointer("/assistant/content")
                                .and_then(Value::as_str)
                                .unwrap_or("(sem resposta)")
                        );
                    }
                }
            }
        }
        ("tasks", Some("list")) => {
            let tasks = call(
                &client,
                &endpoint,
                Method::GET,
                &format!("/api/tasks{}", workspace_query(&args)),
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
        }
        ("tasks", Some("create")) => {
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
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/tasks{}", workspace_query(&args)),
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
        }
        ("tasks", Some("run")) => {
            let id = args
                .positionals
                .get(2)
                .ok_or_else(|| anyhow!("uso: opendesk tasks run <task-id>"))?;
            let result = call(
                &client,
                &endpoint,
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
        }
        ("runs", Some("list")) => {
            let mut path = workspace_query(&args);
            if let Some(limit) = args.flags.get("limit") {
                path.push(if path.contains('?') { '&' } else { '?' });
                path.push_str(&format!("limit={limit}"));
            }
            let runs = call(
                &client,
                &endpoint,
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
        }
        ("runs", Some("show")) => {
            let id = args
                .positionals
                .get(2)
                .ok_or_else(|| anyhow!("uso: opendesk runs show <run-id>"))?;
            let run = call(
                &client,
                &endpoint,
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
        }
        ("queue", Some("list")) => {
            let queue = call(
                &client,
                &endpoint,
                Method::GET,
                &format!("/api/queue{}", workspace_query(&args)),
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
        }
        ("backups", Some("list")) => {
            let backups = call(
                &client,
                &endpoint,
                Method::GET,
                &format!("/api/backups{}", workspace_query(&args)),
                None,
            )
            .await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&backups)?);
            } else if let Some(list) = backups.as_array() {
                for backup in list {
                    println!(
                        "{:<8} {:<22} {:>10}  {}",
                        shorten(backup.get("id").and_then(Value::as_str).unwrap_or("?")),
                        backup
                            .get("created_at")
                            .and_then(Value::as_str)
                            .unwrap_or("?"),
                        format!(
                            "{:.1} MB",
                            backup
                                .get("size_bytes")
                                .and_then(Value::as_u64)
                                .unwrap_or(0) as f64
                                / 1_048_576.0
                        ),
                        backup
                            .get("workspace_name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                    );
                }
            }
        }
        ("backups", Some("create")) => {
            let backup = call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/backups{}", workspace_query(&args)),
                None,
            )
            .await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&backup)?);
            } else {
                println!(
                    "{}  {}  {} arquivos",
                    shorten(backup.get("id").and_then(Value::as_str).unwrap_or("?")),
                    backup
                        .get("file_name")
                        .and_then(Value::as_str)
                        .unwrap_or("?"),
                    backup.get("files").and_then(Value::as_u64).unwrap_or(0)
                );
            }
        }
        ("backups", Some("restore")) => {
            let id = args.positionals.get(2).ok_or_else(|| {
                anyhow!("uso: opendesk backups restore <backup-id> --path <diretório>")
            })?;
            let target = args
                .flags
                .get("path")
                .ok_or_else(|| anyhow!("--path é obrigatório"))?;
            let result = call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/backups/{id}/restore{}", workspace_query(&args)),
                Some(json!({
                    "target_path": target,
                    "name": args.flags.get("name").cloned(),
                })),
            )
            .await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "workspace {}  {}  {}",
                    shorten(result.get("id").and_then(Value::as_str).unwrap_or("?")),
                    result.get("name").and_then(Value::as_str).unwrap_or("?"),
                    result
                        .get("root_path")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                );
            }
        }
        ("backups", Some("download")) => {
            let id = args.positionals.get(2).ok_or_else(|| {
                anyhow!("uso: opendesk backups download <backup-id> --out <arquivo>")
            })?;
            let output = args
                .flags
                .get("out")
                .ok_or_else(|| anyhow!("--out é obrigatório"))?;
            let response = client
                .get(format!(
                    "http://127.0.0.1:{}/api/backups/{id}/download{}",
                    endpoint.port,
                    workspace_query(&args)
                ))
                .bearer_auth(&endpoint.token)
                .send()
                .await
                .context("falha ao baixar backup")?;
            let status = response.status();
            if !status.is_success() {
                let text = response.text().await.unwrap_or_default();
                let message = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("message")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .unwrap_or(text);
                return Err(anyhow!("API {}: {}", status.as_u16(), message));
            }
            let bytes = response.bytes().await.context("falha ao ler backup")?;
            std::fs::write(output, &bytes)
                .with_context(|| format!("não foi possível gravar {output}"))?;
            println!("{output}");
        }
        ("backups", Some("delete")) => {
            let id = args
                .positionals
                .get(2)
                .ok_or_else(|| anyhow!("uso: opendesk backups delete <backup-id>"))?;
            call(
                &client,
                &endpoint,
                Method::DELETE,
                &format!("/api/backups/{id}{}", workspace_query(&args)),
                None,
            )
            .await?;
            println!("backup {id} removido");
        }
        ("export", _) => {
            let id = args.positionals.get(1).ok_or_else(|| {
                anyhow!("uso: opendesk export <run-id> [--format md|html|json] [--out arquivo]")
            })?;
            let format = args.flags.get("format").map(String::as_str).unwrap_or("md");
            let response = client
                .get(format!(
                    "http://127.0.0.1:{}/api/runs/{id}/export?format={}",
                    endpoint.port,
                    urlencoding::encode(format)
                ))
                .bearer_auth(&endpoint.token)
                .send()
                .await
                .context("falha ao exportar o run")?;
            let status = response.status();
            if !status.is_success() {
                let text = response.text().await.unwrap_or_default();
                let message = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("message")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .unwrap_or(text);
                return Err(anyhow!("API {}: {}", status.as_u16(), message));
            }
            let bytes = response.bytes().await.context("falha ao ler export")?;
            if let Some(output) = args.flags.get("out") {
                std::fs::write(output, &bytes)
                    .with_context(|| format!("não foi possível gravar {output}"))?;
                println!("{output}");
            } else {
                print!("{}", String::from_utf8_lossy(&bytes));
            }
        }
        _ => {
            println!("{HELP}");
            return Err(anyhow!("comando desconhecido: {command}"));
        }
    }
    Ok(())
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

async fn node_call(
    client: &Client,
    method: Method,
    url: &str,
    token: &str,
    body: Option<Value>,
) -> Result<Value> {
    let mut request = client.request(method, url).bearer_auth(token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.context("control plane indisponível")?;
    let status = response.status();
    let value: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(anyhow!(
            "control plane respondeu {status}: {}",
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("erro desconhecido")
        ));
    }
    Ok(value)
}

fn node_capabilities() -> Vec<String> {
    let mut capabilities = Vec::new();
    for (harness, command) in [
        ("claude-code", "claude"),
        ("codex", "codex"),
        ("pi", "pi"),
        ("hermes", "hermes"),
    ] {
        if std::env::var_os("PATH")
            .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
            .unwrap_or(false)
        {
            capabilities.push(harness.to_string());
        }
    }
    capabilities
}

fn node_harness_command(harness: &str, prompt: &str) -> Option<(String, Vec<String>)> {
    let prompt = prompt.to_string();
    match harness {
        "claude-code" => Some((
            "claude".into(),
            vec![
                "-p".into(),
                prompt,
                "--output-format".into(),
                "stream-json".into(),
                "--verbose".into(),
            ],
        )),
        "codex" => Some(("codex".into(), vec!["exec".into(), "--json".into(), prompt])),
        "pi" => Some(("pi".into(), vec!["run".into(), "--json".into(), prompt])),
        "hermes" => Some(("hermes".into(), vec!["run".into(), "--json".into(), prompt])),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_node(
    central: &str,
    token: &str,
    once: bool,
    poll_secs: u64,
    opencode_url: Option<String>,
    opencode_password: Option<String>,
    force_sync: bool,
    workdir: Option<String>,
) -> Result<()> {
    let central = central.trim_end_matches('/').to_string();
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("cliente HTTP do nó")?;
    let capabilities = node_capabilities();
    if capabilities.is_empty() {
        println!("Aviso: nenhum harness CLI encontrado no PATH (claude/codex/pi/hermes).");
    }
    println!(
        "Nó OpenDesk conectado a {central} — harnesses: {}",
        if capabilities.is_empty() {
            "nenhum".to_string()
        } else {
            capabilities.join(", ")
        }
    );

    loop {
        node_call(
            &client,
            Method::POST,
            &format!("{central}/api/nodes/heartbeat"),
            token,
            Some(json!({ "capabilities": capabilities, "status": "ONLINE" })),
        )
        .await?;

        let claim = node_call(
            &client,
            Method::GET,
            &format!("{central}/api/nodes/next-job"),
            token,
            None,
        )
        .await?;
        let Some(job) = claim.get("job").filter(|value| !value.is_null()).cloned() else {
            if once {
                break;
            }
            tokio::time::sleep(Duration::from_secs(poll_secs)).await;
            continue;
        };

        let job_id = job
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let harness = job
            .get("resolved_harness")
            .and_then(Value::as_str)
            .unwrap_or("opencode")
            .to_string();
        let prompt = job
            .get("prompt_payload")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let controller_workspace = claim
            .get("workspace_root")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let workspace_id = job
            .get("workspace_id")
            .and_then(Value::as_str)
            .unwrap_or("workspace");
        let mut base_manifest = Vec::new();
        let mut synced_root: Option<PathBuf> = None;
        let workspace = if force_sync || !std::path::Path::new(&controller_workspace).is_dir() {
            let root = node_workdir(workdir.as_deref(), workspace_id);
            let manifest = sync_job_workspace(&client, &central, token, &job_id, &root).await?;
            base_manifest = manifest;
            synced_root = Some(root.clone());
            root.to_string_lossy().to_string()
        } else {
            controller_workspace
        };
        let agent = job
            .get("resolved_agent")
            .and_then(Value::as_str)
            .unwrap_or("build")
            .to_string();
        let model = job
            .get("resolved_model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let title = job
            .get("task_title")
            .and_then(Value::as_str)
            .unwrap_or("Remote node job")
            .to_string();
        let run_id = claim
            .get("run_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let sandbox: crate::sandbox::SandboxPolicy = claim
            .get("sandbox")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        println!(
            "Executando job {} com {} em {}",
            shorten(&job_id),
            harness,
            workspace
        );

        let stop = Arc::new(AtomicBool::new(false));
        let heartbeat = {
            let client = client.clone();
            let central = central.clone();
            let token = token.to_string();
            let stop = stop.clone();
            let job_id = job_id.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(20)).await;
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let _ = node_call(
                        &client,
                        Method::POST,
                        &format!("{central}/api/nodes/jobs/{job_id}/heartbeat"),
                        &token,
                        None,
                    )
                    .await;
                }
            })
        };

        let started = Instant::now();
        let outcome = match harness.as_str() {
            "opencode" => match (opencode_url.as_deref(), opencode_password.as_deref()) {
                (Some(url), Some(password)) => {
                    execute_opencode_job(url, password, &workspace, &title, &agent, &model, &prompt)
                        .await
                }
                _ => Err(anyhow!(
                    "job opencode exige --opencode-url e --opencode-password (ou OPENDESK_OPENCODE_URL/PASSWORD)"
                )),
            },
            _ => execute_node_job(&harness, &prompt, &workspace, &sandbox).await,
        };
        stop.store(true, Ordering::Relaxed);
        heartbeat.abort();

        let body = match outcome {
            Ok((success, output, files)) => {
                let summary: String = output
                    .chars()
                    .rev()
                    .take(4000)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                let uploaded = match &synced_root {
                    Some(root) => sync_upload_files(root, &base_manifest)?,
                    None => files,
                };
                json!({
                    "status": if success { "completed" } else { "failed" },
                    "run_id": run_id,
                    "duration_ms": started.elapsed().as_millis() as i64,
                    "summary": summary,
                    "usage": {},
                    "files": uploaded
                })
            }
            Err(error) => json!({
                "status": "failed",
                "run_id": run_id,
                "duration_ms": started.elapsed().as_millis() as i64,
                "error": error.to_string(),
            }),
        };
        node_call(
            &client,
            Method::POST,
            &format!("{central}/api/nodes/jobs/{job_id}/complete"),
            token,
            Some(body),
        )
        .await?;
        println!("Job {} concluído.", shorten(&job_id));

        if once {
            break;
        }
    }
    Ok(())
}

fn node_workdir(base: Option<&str>, workspace_id: &str) -> PathBuf {
    let base = base
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("opendesk-node"));
    let safe: String = workspace_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect();
    base.join(if safe.is_empty() { "workspace" } else { &safe })
}

async fn node_call_bytes(client: &Client, url: &str, token: &str) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context("control plane indisponível")?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow!("control plane respondeu {status}"));
    }
    Ok(response.bytes().await?.to_vec())
}

async fn sync_job_workspace(
    client: &Client,
    central: &str,
    token: &str,
    job_id: &str,
    root: &std::path::Path,
) -> Result<Vec<crate::workspace_sync::FileEntry>> {
    std::fs::create_dir_all(root)?;
    let local = crate::workspace_sync::manifest(root)?;
    let response = node_call(
        client,
        Method::POST,
        &format!("{central}/api/nodes/jobs/{job_id}/sync"),
        token,
        Some(json!({ "base": local })),
    )
    .await?;
    let manifest: Vec<crate::workspace_sync::FileEntry> =
        serde_json::from_value(response.get("manifest").cloned().unwrap_or(json!([])))
            .context("manifest de sync inválido")?;
    let files = response.get("files").and_then(Value::as_u64).unwrap_or(0);
    if files > 0 {
        let bundle_id = response
            .get("bundle_id")
            .and_then(Value::as_str)
            .context("sync sem bundle_id")?;
        let bytes = node_call_bytes(
            client,
            &format!("{central}/api/nodes/jobs/{job_id}/sync/{bundle_id}"),
            token,
        )
        .await?;
        let bundle = root.join(".opendesk-sync.tar.zst");
        std::fs::write(&bundle, bytes)?;
        crate::workspace_sync::apply(&bundle, root, false)?;
        let _ = std::fs::remove_file(bundle);
    }
    Ok(manifest)
}

fn sync_upload_files(
    root: &std::path::Path,
    base: &[crate::workspace_sync::FileEntry],
) -> Result<Vec<Value>> {
    let current = crate::workspace_sync::manifest(root)?;
    let delta = crate::workspace_sync::delta(base, &current);
    let mut files = Vec::new();
    for (path, status) in delta
        .added
        .iter()
        .map(|path| (path, "added"))
        .chain(delta.changed.iter().map(|path| (path, "modified")))
    {
        let bytes = std::fs::read(root.join(path))?;
        files.push(json!({
            "path": path,
            "status": status,
            "patch": "",
            "content_base64": base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                bytes,
            ),
        }));
    }
    for path in delta.removed {
        files.push(json!({ "path": path, "status": "deleted", "patch": "" }));
    }
    Ok(files)
}

async fn execute_node_job(
    harness: &str,
    prompt: &str,
    workspace: &str,
    sandbox: &crate::sandbox::SandboxPolicy,
) -> Result<(bool, String, Vec<Value>)> {
    let Some((program, args)) = node_harness_command(harness, prompt) else {
        return Err(anyhow!(
            "harness {harness} não é suportado pelo worker remoto; use claude-code, codex, pi ou hermes"
        ));
    };
    if !sandbox.command_allowed(&program) {
        return Err(anyhow!(
            "comando {program} não está na allowlist do sandbox do workspace"
        ));
    }
    if !std::path::Path::new(workspace).is_dir() {
        return Err(anyhow!(
            "workspace {workspace} não está acessível neste nó; monte a pasta compartilhada"
        ));
    }
    let wrapped = crate::sandbox::wrap_strict(sandbox, workspace, &program, &args)?;
    let (program, args) = wrapped
        .split_first()
        .map(|(program, args)| (program.clone(), args.to_vec()))
        .unwrap_or((program.clone(), Vec::new()));
    let timeout_minutes = if sandbox.max_runtime_minutes > 0 {
        sandbox.max_runtime_minutes as u64
    } else {
        30
    };
    let mut command = tokio::process::Command::new(&program);
    crate::sandbox::scrub_environment(&mut command, sandbox);
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_minutes * 60),
        command
            .args(&args)
            .current_dir(workspace)
            .stdin(std::process::Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| anyhow!("execução excedeu {timeout_minutes} minutos"))?
    .with_context(|| format!("não foi possível executar {program}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    let files = crate::adapters::cli::git_diffs(workspace)
        .await
        .into_iter()
        .map(|diff| {
            json!({
                "path": diff.path,
                "status": diff.status,
                "patch": diff.patch,
            })
        })
        .collect::<Vec<_>>();
    Ok((output.status.success(), text, files))
}

fn extract_sse_events(buffer: &mut String) -> Vec<Value> {
    let mut events = Vec::new();
    while let Some(position) = buffer.find("\n\n") {
        let block = buffer[..position].to_string();
        buffer.drain(..position + 2);
        let mut data = String::new();
        for line in block.lines() {
            if let Some(value) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.trim_start());
            }
        }
        if let Ok(value) = serde_json::from_str::<Value>(&data) {
            events.push(value);
        }
    }
    events
}

fn opencode_session_matches(event: &Value, session: &str) -> bool {
    event
        .pointer("/data/sessionID")
        .and_then(Value::as_str)
        .is_some_and(|value| value == session)
}

#[allow(clippy::too_many_arguments)]
async fn execute_opencode_job(
    url: &str,
    password: &str,
    workspace: &str,
    title: &str,
    agent: &str,
    model: &str,
    prompt: &str,
) -> Result<(bool, String, Vec<Value>)> {
    if !std::path::Path::new(workspace).is_dir() {
        return Err(anyhow!(
            "workspace {workspace} não está acessível neste nó; monte a pasta compartilhada"
        ));
    }
    let client = RestClient::new_with_url(url, password.to_string());
    let session = client
        .create_session(workspace, title, agent, ModelRefDto::parse(model))
        .await
        .context("não foi possível criar a sessão OpenCode")?;
    let mut stream = client
        .event_response()
        .await
        .context("eventos do OpenCode indisponíveis")?
        .bytes_stream();
    client
        .prompt(&session, prompt)
        .await
        .context("não foi possível enviar o prompt ao OpenCode")?;

    let started = Instant::now();
    let mut last_activity = Instant::now();
    let mut buffer = String::new();
    let mut text = String::new();
    let mut completed = false;
    let mut failed = false;
    let mut error: Option<String> = None;

    while !completed && !failed {
        if started.elapsed() > Duration::from_secs(30 * 60) {
            let _ = client.interrupt(&session).await;
            return Err(anyhow!("execução OpenCode excedeu 30 minutos"));
        }
        if last_activity.elapsed() > Duration::from_secs(20 * 60) {
            let _ = client.interrupt(&session).await;
            return Err(anyhow!("OpenCode ficou 20 minutos sem eventos"));
        }
        match tokio::time::timeout(Duration::from_secs(2), stream.next()).await {
            Ok(Some(Ok(bytes))) => {
                last_activity = Instant::now();
                buffer.push_str(&String::from_utf8_lossy(&bytes));
                buffer = buffer.replace("\r\n", "\n");
                for event in extract_sse_events(&mut buffer) {
                    if !opencode_session_matches(&event, &session) {
                        continue;
                    }
                    match event.get("type").and_then(Value::as_str).unwrap_or("") {
                        "session.text.delta" => {
                            if let Some(delta) =
                                event.pointer("/data/delta").and_then(Value::as_str)
                            {
                                text.push_str(delta);
                            }
                        }
                        "session.execution.succeeded" | "session.idle" => {
                            completed = true;
                        }
                        "session.execution.failed" => {
                            failed = true;
                            error = Some(
                                event
                                    .pointer("/data/error")
                                    .map(Value::to_string)
                                    .unwrap_or_else(|| "OpenCode execution failed".into()),
                            );
                        }
                        _ => {}
                    }
                    if completed || failed {
                        break;
                    }
                }
            }
            Ok(Some(Err(stream_error))) => {
                // The session may still finish; wait_session below decides.
                error = Some(format!("stream do OpenCode interrompido: {stream_error}"));
                break;
            }
            Ok(None) => break,
            Err(_) => {}
        }
    }

    if !completed && !failed {
        match tokio::time::timeout(Duration::from_secs(5 * 60), client.wait_session(&session)).await
        {
            Ok(Ok(())) => completed = true,
            Ok(Err(wait_error)) => {
                failed = true;
                error = Some(format!("wait_session falhou: {wait_error:#}"));
            }
            Err(_) => {
                let _ = client.interrupt(&session).await;
                failed = true;
                error = Some("timeout aguardando a sessão OpenCode".into());
            }
        }
    }

    let mut files = Vec::new();
    if completed
        && !failed
        && let Ok(diffs) = client.diff(&session).await
    {
        files = diffs
            .into_iter()
            .map(|diff| {
                json!({
                    "path": diff.file,
                    "status": diff.status,
                    "patch": diff.patch,
                })
            })
            .collect();
    }
    if let Some(error) = error {
        text.push_str(&format!("\n{error}"));
    }
    Ok((completed && !failed, text, files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags_positionals_and_json() {
        let args = vec![
            "tasks".to_string(),
            "create".to_string(),
            "--title".to_string(),
            "Relatório".to_string(),
            "--description=Resumo".to_string(),
            "--json".to_string(),
        ];
        let parsed = parse(&args);
        assert_eq!(parsed.positionals, vec!["tasks", "create"]);
        assert_eq!(
            parsed.flags.get("title").map(String::as_str),
            Some("Relatório")
        );
        assert_eq!(
            parsed.flags.get("description").map(String::as_str),
            Some("Resumo")
        );
        assert!(parsed.json);
    }

    #[test]
    fn parses_sse_and_matches_sessions() {
        let mut buffer = "data: {\"type\":\"session.text.delta\",\"data\":{\"sessionID\":\"s1\",\"delta\":\"oi\"}}\n\n".to_string();
        let events = extract_sse_events(&mut buffer);
        assert_eq!(events.len(), 1);
        assert!(opencode_session_matches(&events[0], "s1"));
        assert!(!opencode_session_matches(&events[0], "s2"));
        assert!(buffer.is_empty());
    }

    #[test]
    fn maps_remote_harnesses_to_commands() {
        let (program, args) = node_harness_command("claude-code", "olá").expect("claude");
        assert_eq!(program, "claude");
        assert!(args.contains(&"olá".to_string()));
        let (program, args) = node_harness_command("codex", "olá").expect("codex");
        assert_eq!(program, "codex");
        assert_eq!(args[0], "exec");
        assert!(node_harness_command("opencode", "olá").is_none());
        assert!(node_harness_command("desconhecido", "olá").is_none());
    }

    #[test]
    fn shortens_ids_safely() {
        assert_eq!(shorten("123456789"), "12345678");
        assert_eq!(shorten("abc"), "abc");
    }
}
