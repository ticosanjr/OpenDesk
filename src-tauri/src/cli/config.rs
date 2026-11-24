use super::args::Args;
use crate::{app_state::ApiEndpoint, internal_api::call};
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::path::PathBuf;

pub(super) async fn export(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let path = args
        .positionals
        .get(2)
        .map(PathBuf::from)
        .or_else(|| args.flags.get("out").map(PathBuf::from))
        .ok_or_else(|| anyhow!("uso: opendesk config export <arquivo.json>"))?;
    let sections = args.flags.get("sections").map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    });
    let bundle = call(
        client,
        endpoint,
        Method::POST,
        "/api/config-bundle/export",
        Some(json!({
            "workspace_id": args.flags.get("workspace"),
            "sections": sections,
        })),
    )
    .await?;
    std::fs::write(&path, serde_json::to_vec_pretty(&bundle)?)
        .with_context(|| format!("não foi possível gravar {}", path.display()))?;
    println!("bundle gravado em {}", path.display());
    Ok(())
}

pub(super) async fn import(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let path = args
        .positionals
        .get(2)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("uso: opendesk config import <arquivo.json>"))?;
    let bundle: Value = serde_json::from_slice(
        &std::fs::read(&path)
            .with_context(|| format!("não foi possível ler {}", path.display()))?,
    )
    .with_context(|| format!("{} não é um JSON válido", path.display()))?;
    let mode = match args
        .flags
        .get("mode")
        .map(String::as_str)
        .unwrap_or("merge-skip")
    {
        "merge-overwrite" => "merge-overwrite",
        "replace" | "replace-section" => "replace-section",
        _ => "merge-skip-existing",
    };
    let dry_run = args.flags.contains_key("dry-run");
    let route = if dry_run {
        "/api/config-bundle/preview"
    } else {
        "/api/config-bundle/import"
    };
    let report = call(
        client,
        endpoint,
        Method::POST,
        route,
        Some(json!({
            "workspace_id": args.flags.get("workspace"),
            "bundle": bundle,
            "mode": mode,
        })),
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "{} · {} seção(ões)",
            if dry_run { "prévia" } else { "importado" },
            report
                .get("sections")
                .and_then(Value::as_object)
                .map(|sections| sections.len())
                .unwrap_or(0)
        );
    }
    Ok(())
}
