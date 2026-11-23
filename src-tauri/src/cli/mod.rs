//! Minimal headless CLI. It talks to the running local API; it never opens the
//! GUI or duplicates execution logic.

mod args;
mod backups;
mod chat;
mod commands;
mod node;
mod node_job;
mod opencode_job;
mod runs;
mod tasks;
mod tui;

use anyhow::{Result, anyhow};
use reqwest::Client;

use crate::internal_api::endpoint;

use args::{Args, HELP, parse};

#[cfg(test)]
mod tests;

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
        ("status", _) => commands::status(&args, &client, &endpoint).await?,
        ("node", Some("run")) => node::run_from_args(&args).await?,
        ("tui", _) => tui::run(&args, &client, &endpoint).await?,
        ("nodes", Some("list")) => commands::nodes_list(&args, &client, &endpoint).await?,
        ("workspaces", Some("list")) => {
            commands::workspaces_list(&args, &client, &endpoint).await?
        }
        ("approvals", Some("list")) => commands::approvals_list(&args, &client, &endpoint).await?,
        ("approvals", Some("resolve")) => {
            commands::approvals_resolve(&args, &client, &endpoint).await?
        }
        ("diff", _) => commands::diff(&args, &client, &endpoint).await?,
        ("chat", _) => chat::chat(&args, &client, &endpoint).await?,
        ("tasks", Some("list")) => tasks::list(&args, &client, &endpoint).await?,
        ("tasks", Some("create")) => tasks::create(&args, &client, &endpoint).await?,
        ("tasks", Some("run")) => tasks::run(&args, &client, &endpoint).await?,
        ("runs", Some("list")) => runs::list(&args, &client, &endpoint).await?,
        ("runs", Some("show")) => runs::show(&args, &client, &endpoint).await?,
        ("queue", Some("list")) => runs::queue_list(&args, &client, &endpoint).await?,
        ("backups", Some("list")) => backups::list(&args, &client, &endpoint).await?,
        ("backups", Some("create")) => backups::create(&args, &client, &endpoint).await?,
        ("backups", Some("restore")) => backups::restore(&args, &client, &endpoint).await?,
        ("backups", Some("download")) => backups::download(&args, &client, &endpoint).await?,
        ("backups", Some("delete")) => backups::delete(&args, &client, &endpoint).await?,
        ("export", _) => backups::export(&args, &client, &endpoint).await?,
        _ => {
            println!("{HELP}");
            return Err(anyhow!("comando desconhecido: {command}"));
        }
    }
    Ok(())
}
