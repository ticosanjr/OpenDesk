use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};

use super::{
    args::{Args, shorten},
    node_job::{execute_node_job, node_workdir, sync_job_workspace, sync_upload_files},
    opencode_job::execute_opencode_job,
};

pub(super) async fn run_from_args(args: &Args) -> Result<()> {
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
    Ok(())
}

pub(super) async fn node_call(
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

pub(super) fn node_harness_command(harness: &str, prompt: &str) -> Option<(String, Vec<String>)> {
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
