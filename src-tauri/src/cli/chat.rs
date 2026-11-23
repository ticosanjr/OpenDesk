use std::io::{self, Write};

use crate::{app_state::ApiEndpoint, internal_api::call};
use anyhow::{Context, Result};
use reqwest::{Client, Method};
use serde_json::{Value, json};

use super::args::Args;

pub(super) async fn chat(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let workspace_id = match args.flags.get("workspace") {
        Some(value) => Some(value.clone()),
        None => {
            let workspaces = call(client, endpoint, Method::GET, "/api/workspaces", None).await?;
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
        let workspace_id = workspace_id.context("nenhum workspace ativo para iniciar o chat")?;
        let created = call(
            client,
            endpoint,
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
    Ok(())
}
