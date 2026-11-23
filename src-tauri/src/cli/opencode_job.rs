use std::time::{Duration, Instant};

use crate::opencode_client::rest_client::{ModelRefDto, RestClient};
use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use serde_json::{Value, json};

pub(super) fn extract_sse_events(buffer: &mut String) -> Vec<Value> {
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

pub(super) fn opencode_session_matches(event: &Value, session: &str) -> bool {
    event
        .pointer("/data/sessionID")
        .and_then(Value::as_str)
        .is_some_and(|value| value == session)
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_opencode_job(
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
