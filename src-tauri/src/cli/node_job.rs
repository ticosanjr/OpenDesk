use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};

use super::node::{node_call, node_harness_command};

pub(super) fn node_workdir(base: Option<&str>, workspace_id: &str) -> PathBuf {
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

pub(super) async fn sync_job_workspace(
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

pub(super) fn sync_upload_files(
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

pub(super) async fn execute_node_job(
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
    command.kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
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
