use crate::{app_state::ApiEndpoint, internal_api::call};
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};

use super::args::{Args, shorten, workspace_query};

pub(super) async fn list(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let backups = call(
        client,
        endpoint,
        Method::GET,
        &format!("/api/backups{}", workspace_query(args)),
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
    Ok(())
}

pub(super) async fn create(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let backup = call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/backups{}", workspace_query(args)),
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
    Ok(())
}

pub(super) async fn restore(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let id = args
        .positionals
        .get(2)
        .ok_or_else(|| anyhow!("uso: opendesk backups restore <backup-id> --path <diretório>"))?;
    let target = args
        .flags
        .get("path")
        .ok_or_else(|| anyhow!("--path é obrigatório"))?;
    let result = call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/backups/{id}/restore{}", workspace_query(args)),
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
    Ok(())
}

pub(super) async fn download(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let id = args
        .positionals
        .get(2)
        .ok_or_else(|| anyhow!("uso: opendesk backups download <backup-id> --out <arquivo>"))?;
    let output = args
        .flags
        .get("out")
        .ok_or_else(|| anyhow!("--out é obrigatório"))?;
    let response = client
        .get(format!(
            "http://127.0.0.1:{}/api/backups/{id}/download{}",
            endpoint.port,
            workspace_query(args)
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
    std::fs::write(output, &bytes).with_context(|| format!("não foi possível gravar {output}"))?;
    println!("{output}");
    Ok(())
}

pub(super) async fn delete(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
    let id = args
        .positionals
        .get(2)
        .ok_or_else(|| anyhow!("uso: opendesk backups delete <backup-id>"))?;
    call(
        client,
        endpoint,
        Method::DELETE,
        &format!("/api/backups/{id}{}", workspace_query(args)),
        None,
    )
    .await?;
    println!("backup {id} removido");
    Ok(())
}

pub(super) async fn export(args: &Args, client: &Client, endpoint: &ApiEndpoint) -> Result<()> {
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
    Ok(())
}
