//! Minimal headless CLI. It talks to the running local API; it never opens the
//! GUI or duplicates execution logic.

use crate::internal_api::{call, endpoint};
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, Write};

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
    fn shortens_ids_safely() {
        assert_eq!(shorten("123456789"), "12345678");
        assert_eq!(shorten("abc"), "abc");
    }
}
