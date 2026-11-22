mod artifacts;
mod stream;

pub use artifacts::chat_turn_signals;
use artifacts::{parse_messages, parse_model_ref};
pub(crate) use stream::{append_log, publish_chunk};
use stream::{extract_events, handle_event, read_tail, session_matches};

use crate::opencode_client::rest_client::{FileDiff, RestClient};
use opendesk_storage::events::EventBus;
use opendesk_storage::repos::runs::RunUsage;
use opendesk_storage::secrets::SecretStore;

use futures_util::StreamExt;
use serde_json::Value;
use sqlx::SqlitePool;
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
    pub pool: &'a SqlitePool,
    pub secrets: &'a SecretStore,
    pub workspace_id: &'a str,
    pub run_id: &'a str,
    pub task_id: Option<&'a str>,
    pub session_id: Option<&'a str>,
    pub workspace_root: &'a Path,
    pub agent: &'a str,
    pub model: &'a str,
    pub phase: &'a str,
    pub title: &'a str,
    pub prompt: &'a str,
    pub log_path: &'a Path,
    pub cancel: CancellationToken,
    pub notifier: Option<&'a dyn crate::notifier::RunNotifier>,
}

pub async fn execute_run(
    rest: RestClient,
    request: RunRequest<'_>,
    events: &EventBus,
) -> RunOutcome {
    let model = parse_model_ref(request.model);
    let session_id = match request.session_id.filter(|value| !value.trim().is_empty()) {
        Some(session_id) => session_id.to_string(),
        None => match rest
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
        },
    };

    publish_chunk(
        events,
        request.run_id,
        request.task_id,
        "status",
        format!(
            "session {session_id} {} for phase {}",
            if request.session_id.is_some() {
                "reused"
            } else {
                "created"
            },
            request.phase
        ),
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
                                if handle_event(
                                    &event,
                                    &session_id,
                                    request.run_id,
                                    request.task_id,
                                    request.workspace_id,
                                    request.pool,
                                    request.secrets,
                                    request.log_path,
                                    events,
                                    &rest,
                                    &mut permissions,
                                    &request.cancel,
                                    request.notifier,
                                )
                                .await
                                {
                                    let _ = rest.interrupt(&session_id).await;
                                    status = RunStatus::Failed;
                                    error_message = Some(
                                        "permission_timeout: decisão humana não chegou a tempo"
                                            .into(),
                                    );
                                    completed = true;
                                    break;
                                }

                                match event.get("type").and_then(Value::as_str) {
                                    Some("session.idle") | Some("session.execution.succeeded") => {
                                        if session_matches(&event, &session_id) {
                                            completed = true;
                                            break;
                                        }
                                    }
                                    Some("session.execution.failed")
                                        if session_matches(&event, &session_id) => {
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
                                    _ => {}
                                }
                            }

                            if completed {
                                break;
                            }
                        }
                        Ok(Some(Err(stream_error))) => {
                            // The HTTP stream can be interrupted while the
                            // session keeps running (network blip, proxy, long
                            // build). Do not fail the run yet: wait_session and
                            // the final messages below decide the outcome.
                            append_log(
                                request.log_path,
                                &serde_json::json!({
                                    "type": "opendesk.event_stream.interrupted",
                                    "error": format!("{stream_error}"),
                                }),
                            );
                            error_message = Some(format!(
                                "OpenCode event stream interrupted: {stream_error}"
                            ));
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

    if status == RunStatus::Success && !completed {
        if summary.is_some() || !writes.is_empty() || !diffs.is_empty() {
            // The stream was interrupted or the completion event was lost, but
            // the session produced real work. Preserve it as a success.
            error_message = None;
            tracing::warn!(
                run = request.run_id,
                "run recovered after stream interruption using messages/artifacts"
            );
        } else {
            status = RunStatus::Failed;
            error_message = Some("OpenCode run did not emit a completion event".into());
        }
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

#[cfg(test)]
mod tests;
