//! Bridges the normalized runtime contract to the queue's `RunOutcome`.

use crate::{
    events::EventBus,
    opencode_client::{
        rest_client::FileDiff,
        session_runner::{
            FileWrite, PermissionDecision, RunOutcome, RunRequest, RunStatus, append_log,
            publish_chunk,
        },
    },
    runtime::{AgentRuntimeAdapter, RuntimeEvent, RuntimeRequest},
};
use futures_util::StreamExt;
use std::{path::Path, sync::Arc, time::Instant};
use tokio::time::{Duration, timeout};

const RUN_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const IDLE_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// Executes one turn through any non-OpenCode adapter. The returned outcome is
/// deliberately identical to the OpenCode runner, so settlement, artifacts,
/// budget and notifications do not branch on the harness.
pub async fn execute(
    adapter: Arc<dyn AgentRuntimeAdapter>,
    request: RunRequest<'_>,
    events: &EventBus,
) -> RunOutcome {
    let sandbox = crate::sandbox::get_for(request.pool, Some(request.workspace_id))
        .await
        .unwrap_or_default();
    let run_timeout = if sandbox.max_runtime_minutes > 0 {
        Duration::from_secs(sandbox.max_runtime_minutes as u64 * 60)
    } else {
        RUN_TIMEOUT
    };
    let runtime_request = RuntimeRequest {
        workspace_root: request.workspace_root.to_string_lossy().to_string(),
        agent: request.agent.to_string(),
        model: (!request.model.trim().is_empty()).then(|| request.model.to_string()),
        prompt: request.prompt.to_string(),
        env: Default::default(),
        sandbox,
    };

    let session_id = match adapter.start(runtime_request).await {
        Ok(session_id) => session_id,
        Err(error) => {
            return failed_outcome(
                request.log_path,
                format!("não foi possível iniciar {}: {error}", adapter.id()),
            );
        }
    };
    publish_chunk(
        events,
        request.run_id,
        request.task_id,
        "status",
        format!("harness {} session {session_id}", adapter.id()),
    );
    append_log(
        request.log_path,
        &serde_json::json!({
            "type": "opendesk.harness.started",
            "harness": adapter.id(),
            "session_id": session_id,
            "phase": request.phase,
            "agent": request.agent,
            "model": request.model,
        }),
    );

    if let Err(error) = adapter.prompt(&session_id, request.prompt).await {
        return failed_outcome(
            request.log_path,
            format!("não foi possível enviar o prompt: {error}"),
        );
    }

    let mut stream = match adapter.events(&session_id).await {
        Ok(stream) => stream,
        Err(error) => {
            return failed_outcome(
                request.log_path,
                format!("não foi possível ler os eventos: {error}"),
            );
        }
    };

    let mut status: Option<RunStatus> = None;
    let mut error_message: Option<String> = None;
    let mut summary: Option<String> = None;
    let mut diffs: Vec<FileDiff> = Vec::new();
    let mut writes: Vec<FileWrite> = Vec::new();
    let mut usage = crate::db::runs::RunUsage::default();
    let mut permissions: Vec<PermissionDecision> = Vec::new();
    let started = Instant::now();
    let mut last_activity = Instant::now();

    loop {
        if started.elapsed() > run_timeout {
            let _ = adapter.interrupt(&session_id).await;
            status = Some(RunStatus::Failed);
            error_message = Some("run exceeded the 30 minute timeout".into());
            break;
        }
        if last_activity.elapsed() > IDLE_TIMEOUT {
            let _ = adapter.interrupt(&session_id).await;
            status = Some(RunStatus::Failed);
            error_message = Some("harness produced no events for 20 minutes".into());
            break;
        }

        tokio::select! {
            _ = request.cancel.cancelled() => {
                let _ = adapter.interrupt(&session_id).await;
                status = Some(RunStatus::Cancelled);
                error_message = Some("cancelled by user".into());
                break;
            }
            item = timeout(Duration::from_secs(2), stream.next()) => {
                match item {
                    Ok(Some(Ok(event))) => {
                        last_activity = Instant::now();
                        append_log(
                            request.log_path,
                            &serde_json::json!({
                                "type": "opendesk.runtime.event",
                                "event": event,
                            }),
                        );
                        match event {
                            RuntimeEvent::TextDelta { delta } => {
                                publish_chunk(events, request.run_id, request.task_id, "text", delta.clone());
                                let entry = summary.get_or_insert_with(String::new);
                                if entry.len() < 8000 {
                                    entry.push_str(&delta);
                                }
                            }
                            RuntimeEvent::ReasoningDelta { delta } => {
                                publish_chunk(events, request.run_id, request.task_id, "reasoning", delta);
                            }
                            RuntimeEvent::ToolCall { call_id, name, input, status: tool_status } => {
                                publish_chunk(
                                    events,
                                    request.run_id,
                                    request.task_id,
                                    "tool",
                                    serde_json::json!({
                                        "call_id": call_id,
                                        "name": name,
                                        "input": input,
                                        "status": tool_status,
                                    })
                                    .to_string(),
                                );
                            }
                            RuntimeEvent::PermissionAsked { request_id, action, resources } => {
                                publish_chunk(
                                    events,
                                    request.run_id,
                                    request.task_id,
                                    "permission",
                                    serde_json::json!({
                                        "request_id": request_id,
                                        "action": action,
                                        "resources": resources,
                                    })
                                    .to_string(),
                                );
                                permissions.push(PermissionDecision {
                                    request_id,
                                    action,
                                    resources,
                                    decision: "pending".into(),
                                });
                            }
                            RuntimeEvent::Usage {
                                input_tokens,
                                output_tokens,
                                cache_read,
                                cache_write,
                                cost_usd,
                            } => {
                                usage.tokens_input += input_tokens;
                                usage.tokens_output += output_tokens;
                                usage.tokens_cache_read += cache_read;
                                usage.tokens_cache_write += cache_write;
                                usage.estimated_cost_usd += cost_usd;
                            }
                            RuntimeEvent::Diff { files } => {
                                for file in files {
                                    diffs.push(FileDiff {
                                        file: file.path,
                                        patch: file.patch,
                                        additions: file.additions,
                                        deletions: file.deletions,
                                        status: file.status,
                                    });
                                }
                            }
                            RuntimeEvent::Artifact { path, action } => {
                                writes.push(FileWrite {
                                    path,
                                    action,
                                    diff_patch: None,
                                });
                            }
                            RuntimeEvent::RunFinished { status: final_status, summary: final_summary } => {
                                status = Some(match final_status.as_str() {
                                    "success" | "completed" | "done" => RunStatus::Success,
                                    "cancelled" => RunStatus::Cancelled,
                                    _ => RunStatus::Failed,
                                });
                                if let Some(text) = final_summary
                                    && !text.trim().is_empty()
                                {
                                    summary = Some(text);
                                }
                                break;
                            }
                            RuntimeEvent::Error { code, message } => {
                                error_message = Some(format!("{code}: {message}"));
                                status = Some(RunStatus::Failed);
                            }
                            _ => {}
                        }
                    }
                    Ok(Some(Err(error))) => {
                        error_message = Some(error.to_string());
                        status = Some(RunStatus::Failed);
                        break;
                    }
                    Ok(None) => break,
                    Err(_) => {}
                }
            }
        }
    }

    if diffs.is_empty() {
        let git = crate::adapters::cli::git_diffs(&request.workspace_root.to_string_lossy()).await;
        diffs = git
            .into_iter()
            .map(|diff| FileDiff {
                file: diff.path,
                patch: diff.patch,
                additions: diff.additions,
                deletions: diff.deletions,
                status: diff.status,
            })
            .collect();
    }

    RunOutcome {
        status: status.unwrap_or(RunStatus::Failed),
        session_id,
        usage,
        summary: summary.map(|text| text.chars().take(4000).collect()),
        diffs,
        writes,
        error: error_message,
        log_tail: read_tail(request.log_path, 16 * 1024),
        permissions,
    }
}

/// Builds the failure outcome used when an adapter cannot even start.
pub fn failed_outcome(log_path: &Path, error: String) -> RunOutcome {
    append_log(
        log_path,
        &serde_json::json!({
            "type": "opendesk.harness.error",
            "error": error,
        }),
    );
    RunOutcome {
        status: RunStatus::Failed,
        session_id: String::new(),
        usage: Default::default(),
        summary: None,
        diffs: Vec::new(),
        writes: Vec::new(),
        error: Some(error),
        log_tail: read_tail(log_path, 16 * 1024),
        permissions: Vec::new(),
    }
}

fn read_tail(path: &Path, max_bytes: usize) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let start = bytes.len().saturating_sub(max_bytes);
    Some(String::from_utf8_lossy(&bytes[start..]).to_string())
}
