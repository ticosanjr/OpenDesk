use crate::{
    db::runs::RunUsage,
    events::{EventBus, OpenDeskWsEvent},
    opencode_client::rest_client::{FileDiff, ModelRefDto, RestClient},
};
use futures_util::StreamExt;
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

const RUN_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const IDLE_TIMEOUT: Duration = Duration::from_secs(20 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Success,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileWrite {
    pub path: String,
    pub action: String,
    pub diff_patch: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PermissionDecision {
    pub request_id: String,
    pub action: String,
    pub resources: Vec<String>,
    pub decision: String,
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub status: RunStatus,
    pub session_id: String,
    pub usage: RunUsage,
    pub summary: Option<String>,
    pub diffs: Vec<FileDiff>,
    pub writes: Vec<FileWrite>,
    pub error: Option<String>,
    pub log_tail: Option<String>,
    pub permissions: Vec<PermissionDecision>,
}

pub struct RunRequest<'a> {
    pub run_id: &'a str,
    pub task_id: Option<&'a str>,
    pub workspace_root: &'a Path,
    pub agent: &'a str,
    pub model: &'a str,
    pub phase: &'a str,
    pub title: &'a str,
    pub prompt: &'a str,
    pub log_path: &'a Path,
    pub cancel: CancellationToken,
}

pub async fn execute_run(
    rest: RestClient,
    request: RunRequest<'_>,
    events: &EventBus,
) -> RunOutcome {
    let model = parse_model_ref(request.model);
    let session_id = match rest
        .create_session(
            &request.workspace_root.to_string_lossy(),
            request.title,
            request.agent,
            model,
        )
        .await
    {
        Ok(session_id) => session_id,
        Err(error) => {
            return failed_outcome(
                String::new(),
                format!("could not create OpenCode session: {error:#}"),
                request.log_path,
            );
        }
    };

    publish_chunk(
        events,
        request.run_id,
        request.task_id,
        "status",
        format!("session {session_id} created for phase {}", request.phase),
    );
    append_log(
        request.log_path,
        &serde_json::json!({
            "type": "opendesk.session.created",
            "session_id": session_id,
            "phase": request.phase,
            "agent": request.agent,
            "model": request.model,
        }),
    );

    let stream = match rest.event_response().await {
        Ok(response) => Some(response.bytes_stream()),
        Err(error) => {
            append_log(
                request.log_path,
                &serde_json::json!({
                    "type": "opendesk.event_stream.error",
                    "error": format!("{error:#}"),
                }),
            );
            None
        }
    };

    if let Err(error) = rest.prompt(&session_id, request.prompt).await {
        return failed_outcome(
            session_id,
            format!("could not send OpenCode prompt: {error:#}"),
            request.log_path,
        );
    }

    let mut status = RunStatus::Success;
    let mut error_message = None;
    let mut permissions = Vec::new();
    let started = Instant::now();
    let mut last_activity = Instant::now();
    let mut completed = false;

    if let Some(mut stream) = stream {
        let mut buffer = String::new();

        loop {
            if started.elapsed() > RUN_TIMEOUT {
                let _ = rest.interrupt(&session_id).await;
                status = RunStatus::Failed;
                error_message = Some("run exceeded the 30 minute timeout".into());
                break;
            }
            if last_activity.elapsed() > IDLE_TIMEOUT {
                let _ = rest.interrupt(&session_id).await;
                status = RunStatus::Failed;
                error_message = Some("OpenCode produced no events for 20 minutes".into());
                break;
            }

            tokio::select! {
                _ = request.cancel.cancelled() => {
                    let _ = rest.interrupt(&session_id).await;
                    status = RunStatus::Cancelled;
                    error_message = Some("cancelled by user".into());
                    break;
                }
                chunk = timeout(Duration::from_secs(2), stream.next()) => {
                    match chunk {
                        Ok(Some(Ok(bytes))) => {
                            last_activity = Instant::now();
                            buffer.push_str(&String::from_utf8_lossy(&bytes));
                            buffer = buffer.replace("\r\n", "\n");

                            for event in extract_events(&mut buffer) {
                                handle_event(
                                    &event,
                                    &session_id,
                                    request.run_id,
                                    request.task_id,
                                    request.log_path,
                                    events,
                                    &rest,
                                    &mut permissions,
                                )
                                .await;

                                match event.get("type").and_then(Value::as_str) {
                                    Some("session.idle") | Some("session.execution.succeeded") => {
                                        if session_matches(&event, &session_id) {
                                            completed = true;
                                            break;
                                        }
                                    }
                                    Some("session.execution.failed") => {
                                        if session_matches(&event, &session_id) {
                                            status = RunStatus::Failed;
                                            error_message = Some(
                                                event
                                                    .pointer("/data/error")
                                                    .map(|value| value.to_string())
                                                    .unwrap_or_else(|| "OpenCode execution failed".into()),
                                            );
                                            completed = true;
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            if completed {
                                break;
                            }
                        }
                        Ok(Some(Err(stream_error))) => {
                            status = RunStatus::Failed;
                            error_message = Some(format!("OpenCode event stream failed: {stream_error}"));
                            break;
                        }
                        Ok(None) => {
                            // Stream closed. Fall through to wait_session below.
                            break;
                        }
                        Err(_) => {
                            // Two seconds without an event is normal; the timeout
                            // branch only keeps the loop responsive to cancellation.
                        }
                    }
                }
            }
        }

        // The stream can close between the prompt and the terminal event. The
        // wait endpoint is the fallback, not the primary completion signal.
        if !completed && matches!(status, RunStatus::Success) {
            match timeout(Duration::from_secs(5 * 60), rest.wait_session(&session_id)).await {
                Ok(Ok(())) => {
                    completed = true;
                }
                Ok(Err(wait_error)) => {
                    append_log(
                        request.log_path,
                        &serde_json::json!({
                            "type": "opendesk.wait.error",
                            "error": format!("{wait_error:#}"),
                        }),
                    );
                }
                Err(_) => {
                    let _ = rest.interrupt(&session_id).await;
                    status = RunStatus::Failed;
                    error_message = Some("timed out waiting for the OpenCode session".into());
                }
            }
        }
    } else {
        // No SSE stream: block on wait and then inspect messages.
        match timeout(Duration::from_secs(30 * 60), rest.wait_session(&session_id)).await {
            Ok(Ok(())) => {
                completed = true;
            }
            Ok(Err(error)) => {
                status = RunStatus::Failed;
                error_message = Some(format!("OpenCode wait failed: {error:#}"));
            }
            Err(_) => {
                let _ = rest.interrupt(&session_id).await;
                status = RunStatus::Failed;
                error_message = Some("run exceeded the 30 minute timeout".into());
            }
        }
    }

    if status == RunStatus::Success && !completed {
        status = RunStatus::Failed;
        error_message = Some("OpenCode run did not emit a completion event".into());
    }

    let (usage, summary, writes) = match rest.messages(&session_id).await {
        Ok(value) => parse_messages(&value),
        Err(error) => {
            append_log(
                request.log_path,
                &serde_json::json!({
                    "type": "opendesk.messages.error",
                    "error": format!("{error:#}"),
                }),
            );
            (RunUsage::default(), None, Vec::new())
        }
    };

    let diffs = match rest.diff(&session_id).await {
        Ok(diffs) => diffs,
        Err(error) => {
            append_log(
                request.log_path,
                &serde_json::json!({
                    "type": "opendesk.diff.error",
                    "error": format!("{error:#}"),
                }),
            );
            Vec::new()
        }
    };

    if status == RunStatus::Success && summary.is_none() {
        status = RunStatus::Failed;
        error_message = Some("OpenCode finished without an assistant response".into());
    }

    let log_tail = read_tail(request.log_path, 16 * 1024);
    publish_chunk(
        events,
        request.run_id,
        request.task_id,
        "status",
        format!("run finished: {}", status.as_str()),
    );

    RunOutcome {
        status,
        session_id,
        usage,
        summary,
        diffs,
        writes,
        error: error_message,
        log_tail,
        permissions,
    }
}

fn failed_outcome(session_id: String, error: String, log_path: &Path) -> RunOutcome {
    append_log(
        log_path,
        &serde_json::json!({
            "type": "opendesk.run.error",
            "error": error,
        }),
    );
    RunOutcome {
        status: RunStatus::Failed,
        session_id,
        usage: RunUsage::default(),
        summary: None,
        diffs: Vec::new(),
        writes: Vec::new(),
        error: Some(error),
        log_tail: read_tail(log_path, 16 * 1024),
        permissions: Vec::new(),
    }
}

async fn handle_event(
    event: &Value,
    session_id: &str,
    run_id: &str,
    task_id: Option<&str>,
    log_path: &Path,
    events: &EventBus,
    rest: &RestClient,
    permissions: &mut Vec<PermissionDecision>,
) {
    if !session_matches(event, session_id) {
        return;
    }

    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    append_log(log_path, event);

    match event_type {
        "session.text.delta" => {
            if let Some(delta) = event.pointer("/data/delta").and_then(Value::as_str) {
                publish_chunk(events, run_id, task_id, "text", delta.to_string());
            }
        }
        "session.reasoning.delta" => {
            if let Some(delta) = event.pointer("/data/delta").and_then(Value::as_str) {
                publish_chunk(events, run_id, task_id, "reasoning", delta.to_string());
            }
        }
        "session.tool.called"
        | "session.tool.input.ended"
        | "session.tool.success"
        | "session.tool.failed" => {
            let content = event
                .get("data")
                .map(|data| data.to_string())
                .unwrap_or_default();
            publish_chunk(events, run_id, task_id, "tool", content);
        }
        "session.usage.recorded" => {
            let content = event
                .get("data")
                .map(|data| data.to_string())
                .unwrap_or_default();
            publish_chunk(events, run_id, task_id, "usage", content);
        }
        "permission.asked" => {
            let request_id = event
                .pointer("/data/id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let action = event
                .pointer("/data/action")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let resources = event
                .pointer("/data/resources")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            publish_chunk(
                events,
                run_id,
                task_id,
                "permission",
                event
                    .get("data")
                    .map(|data| data.to_string())
                    .unwrap_or_default(),
            );

            if !request_id.is_empty() {
                match rest.reply_permission(session_id, &request_id, "once").await {
                    Ok(()) => permissions.push(PermissionDecision {
                        request_id,
                        action,
                        resources,
                        decision: "allow".into(),
                    }),
                    Err(error) => {
                        append_log(
                            log_path,
                            &serde_json::json!({
                                "type": "opendesk.permission.error",
                                "error": format!("{error:#}"),
                            }),
                        );
                    }
                }
            }
        }
        _ => {}
    }
}

fn publish_chunk(
    events: &EventBus,
    run_id: &str,
    task_id: Option<&str>,
    chunk_type: &str,
    content: String,
) {
    events.publish(OpenDeskWsEvent::RunStreamChunk {
        run_id: run_id.to_string(),
        task_id: task_id.map(str::to_string),
        chunk_type: chunk_type.to_string(),
        content,
    });
}

fn append_log(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let line = format!("{value}\n");
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()));
}

fn read_tail(path: &Path, max_bytes: usize) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let start = bytes.len().saturating_sub(max_bytes);
    Some(String::from_utf8_lossy(&bytes[start..]).to_string())
}

fn session_matches(event: &Value, session_id: &str) -> bool {
    event
        .pointer("/data/sessionID")
        .and_then(Value::as_str)
        .map(|value| value == session_id)
        .unwrap_or(false)
}

fn extract_events(buffer: &mut String) -> Vec<Value> {
    let mut events = Vec::new();

    loop {
        let Some(position) = buffer.find("\n\n") else {
            break;
        };
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

        if data.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&data) {
            events.push(value);
        }
    }

    events
}

fn parse_messages(value: &Value) -> (RunUsage, Option<String>, Vec<FileWrite>) {
    let mut usage = RunUsage::default();
    let mut summary = None;
    let mut writes = Vec::new();

    let Some(messages) = value.get("data").and_then(Value::as_array) else {
        return (usage, summary, writes);
    };

    for message in messages {
        if message.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }

        if let Some(cost) = message.get("cost").and_then(Value::as_f64) {
            usage.estimated_cost_usd += cost;
        }

        if let Some(tokens) = message.get("tokens") {
            usage.tokens_input += tokens.get("input").and_then(Value::as_i64).unwrap_or(0);
            usage.tokens_output += tokens.get("output").and_then(Value::as_i64).unwrap_or(0);
            usage.tokens_reasoning += tokens.get("reasoning").and_then(Value::as_i64).unwrap_or(0);
            if let Some(cache) = tokens.get("cache") {
                usage.tokens_cache_read += cache.get("read").and_then(Value::as_i64).unwrap_or(0);
                usage.tokens_cache_write += cache.get("write").and_then(Value::as_i64).unwrap_or(0);
            }
        }

        let content = message.get("content").and_then(Value::as_array);
        let text = content
            .map(|content| {
                content
                    .iter()
                    .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|part| part.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        if !text.trim().is_empty() {
            summary = Some(text);
        }

        if let Some(content) = content {
            for part in content {
                if part.get("type").and_then(Value::as_str) != Some("tool") {
                    continue;
                }
                let name = part.get("name").and_then(Value::as_str).unwrap_or("");
                if !matches!(name, "write" | "edit" | "patch") {
                    continue;
                }
                let state = part.get("state");
                let status = state
                    .and_then(|state| state.get("status"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if status != "completed" {
                    continue;
                }
                let input = state.and_then(|state| state.get("input"));
                let path = input
                    .and_then(|input| {
                        input
                            .get("path")
                            .or_else(|| input.get("filePath"))
                            .or_else(|| input.get("file_path"))
                    })
                    .and_then(Value::as_str);
                let Some(path) = path else {
                    continue;
                };

                let (action, patch) = if name == "write" {
                    let content = input
                        .and_then(|input| input.get("content"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let diff = format!(
                        "--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+{}\n",
                        content.lines().collect::<Vec<_>>().join("\n+")
                    );
                    ("CREATED".to_string(), Some(diff))
                } else {
                    let old = input
                        .and_then(|input| input.get("oldString"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let new = input
                        .and_then(|input| input.get("newString"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let diff = format!(
                        "--- a/{path}\n+++ b/{path}\n-{}\n+{}\n",
                        old.lines().collect::<Vec<_>>().join("\n-"),
                        new.lines().collect::<Vec<_>>().join("\n+")
                    );
                    ("MODIFIED".to_string(), Some(diff))
                };

                writes.push(FileWrite {
                    path: path.to_string(),
                    action,
                    diff_patch: patch,
                });
            }
        }
    }

    (usage, summary, writes)
}

fn parse_model_ref(model: &str) -> Option<ModelRefDto> {
    let (base, variant) = match model.split_once('#') {
        Some((base, variant)) => (base, Some(variant.to_string())),
        None => (model, None),
    };
    let (provider, id) = base.split_once('/')?;
    if provider.is_empty() || id.is_empty() {
        return None;
    }
    Some(ModelRefDto {
        provider_id: provider.to_string(),
        id: id.to_string(),
        variant,
    })
}
