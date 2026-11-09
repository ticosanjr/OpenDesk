//! Minimal headless CLI. It talks to the running local API; it never opens the
//! GUI or duplicates execution logic.

use crate::{app_state::ApiEndpoint, paths::AppPaths};
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::collections::HashMap;

const HELP: &str = "OpenDesk CLI

Uso:
  opendesk status [--json]
  opendesk workspaces list [--json]
  opendesk tasks list [--workspace <id>] [--json]
  opendesk tasks create --title <título> --description <descrição>
        [--agent <id>] [--model <ref>] [--mode <modo>] [--priority <1-4>]
        [--tags a,b] [--workspace <id>] [--json]
  opendesk tasks run <task-id> [--json]
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

fn endpoint() -> Result<ApiEndpoint> {
    let paths =
        AppPaths::discover().context("não foi possível localizar os diretórios do OpenDesk")?;
    let raw = std::fs::read_to_string(&paths.api_lock_path).with_context(|| {
        format!(
            "OpenDesk não está em execução ({} ausente). Abra o app ou rode com --headless.",
            paths.api_lock_path.display()
        )
    })?;
    serde_json::from_str(&raw).context("api.json inválido")
}

async fn call(
    client: &Client,
    endpoint: &ApiEndpoint,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    let mut request = client
        .request(method, format!("http://127.0.0.1:{}{path}", endpoint.port))
        .bearer_auth(&endpoint.token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .context("falha ao chamar a API local")?;
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
    Ok(response.json().await.unwrap_or(Value::Null))
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
