//! OpenCode V2 adapter.
//!
//! Wraps the local daemon REST API behind the harness-agnostic contract. The
//! daemon remains the only executor for OpenCode sessions; this adapter only
//! normalizes its SSE stream into [`RuntimeEvent`]s.

use crate::{
    error::{AppError, AppResult},
    opencode_client::{
        daemon_manager::DaemonManager,
        rest_client::{FileDiff, ModelRefDto, RestClient},
    },
    runtime::{
        AgentRuntimeAdapter, RuntimeCapabilities, RuntimeEvent, RuntimeEventStream,
        RuntimeFileDiff, RuntimeRequest,
    },
};

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::{sync::mpsc, time::timeout};

pub struct OpenCodeAdapter {
    daemon: Arc<DaemonManager>,
}

impl OpenCodeAdapter {
    pub fn new(daemon: Arc<DaemonManager>) -> Self {
        Self { daemon }
    }

    async fn client(&self) -> AppResult<RestClient> {
        self.daemon
            .client()
            .await
            .ok_or_else(|| AppError::Daemon("OpenCode daemon is offline".into()))
    }
}

#[async_trait]
impl AgentRuntimeAdapter for OpenCodeAdapter {
    fn id(&self) -> &str {
        "opencode"
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            streaming: true,
            tools: true,
            permissions: true,
            reasoning: true,
            usage: true,
            diffs: true,
            sessions: true,
            sandbox: false,
        }
    }

    async fn start(&self, request: RuntimeRequest) -> AppResult<String> {
        let client = self.client().await?;
        let model = request.model.as_deref().and_then(ModelRefDto::parse);
        client
            .create_session(
                &request.workspace_root,
                "OpenDesk run",
                &request.agent,
                model,
            )
            .await
            .map_err(|error| AppError::Daemon(error.to_string()))
    }

    async fn prompt(&self, session_id: &str, prompt: &str) -> AppResult<()> {
        let client = self.client().await?;
        client
            .prompt(session_id, prompt)
            .await
            .map(|_| ())
            .map_err(|error| AppError::Daemon(error.to_string()))
    }

    async fn events(&self, session_id: &str) -> AppResult<RuntimeEventStream> {
        let client = self.client().await?;
        let session_id = session_id.to_string();
        let (sender, receiver) = mpsc::channel(4096);
        tokio::spawn(async move {
            stream_session(client, session_id, sender).await;
        });
        Ok(Box::pin(futures_util::stream::unfold(
            receiver,
            |mut receiver| async move { receiver.recv().await.map(|event| (Ok(event), receiver)) },
        )))
    }

    async fn interrupt(&self, session_id: &str) -> AppResult<()> {
        let client = self.client().await?;
        client
            .interrupt(session_id)
            .await
            .map_err(|error| AppError::Daemon(error.to_string()))
    }
}

async fn stream_session(
    client: RestClient,
    session_id: String,
    sender: mpsc::Sender<RuntimeEvent>,
) {
    let response = match client.event_response().await {
        Ok(response) => response,
        Err(error) => {
            let _ = sender
                .send(RuntimeEvent::Error {
                    code: "opencode_events".into(),
                    message: error.to_string(),
                })
                .await;
            let _ = sender
                .send(RuntimeEvent::RunFinished {
                    status: "failed".into(),
                    summary: None,
                })
                .await;
            return;
        }
    };

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut finished = false;
    let mut failed = false;

    'outer: while let Some(chunk) = stream.next().await {
        let bytes = match chunk {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = sender
                    .send(RuntimeEvent::Error {
                        code: "opencode_stream".into(),
                        message: error.to_string(),
                    })
                    .await;
                break;
            }
        };
        buffer.push_str(&String::from_utf8_lossy(&bytes));
        buffer = buffer.replace("\r\n", "\n");
        for event in extract_events(&mut buffer) {
            if !session_matches(&event, &session_id) {
                continue;
            }
            let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
            for normalized in normalize_event(kind, &event) {
                if matches!(
                    normalized,
                    RuntimeEvent::RunFinished {
                        ref status, ..
                    } if status != "success"
                ) {
                    failed = true;
                }
                if sender.send(normalized).await.is_err() {
                    break 'outer;
                }
            }
            match kind {
                "session.idle" | "session.execution.succeeded" => {
                    finished = true;
                    break 'outer;
                }
                "session.execution.failed" => {
                    failed = true;
                    finished = true;
                    break 'outer;
                }
                _ => {}
            }
        }
    }

    if !finished && !failed {
        // The SSE connection can close between the prompt and the terminal
        // event. Wait for the session as a fallback instead of reporting a
        // spurious success.
        match timeout(
            Duration::from_secs(5 * 60),
            client.wait_session(&session_id),
        )
        .await
        {
            Ok(Ok(())) => finished = true,
            Ok(Err(error)) => {
                failed = true;
                let _ = sender
                    .send(RuntimeEvent::Error {
                        code: "opencode_wait".into(),
                        message: error.to_string(),
                    })
                    .await;
            }
            Err(_) => {
                failed = true;
                let _ = sender
                    .send(RuntimeEvent::Error {
                        code: "opencode_timeout".into(),
                        message: "timeout aguardando a sessão OpenCode".into(),
                    })
                    .await;
            }
        }
    }

    if finished
        && !failed
        && let Ok(diffs) = client.diff(&session_id).await
        && !diffs.is_empty()
    {
        let _ = sender
            .send(RuntimeEvent::Diff {
                files: diffs.into_iter().map(from_file_diff).collect(),
            })
            .await;
    }

    let _ = sender
        .send(RuntimeEvent::RunFinished {
            status: if failed { "failed" } else { "success" }.into(),
            summary: None,
        })
        .await;
}

fn from_file_diff(diff: FileDiff) -> RuntimeFileDiff {
    RuntimeFileDiff {
        path: diff.file,
        status: diff.status,
        patch: diff.patch,
        additions: diff.additions,
        deletions: diff.deletions,
    }
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
        if data.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&data) {
            events.push(value);
        }
    }
    events
}

fn normalize_event(kind: &str, event: &Value) -> Vec<RuntimeEvent> {
    let mut events = Vec::new();
    match kind {
        "session.text.delta" => {
            if let Some(delta) = event.pointer("/data/delta").and_then(Value::as_str) {
                events.push(RuntimeEvent::TextDelta {
                    delta: delta.to_string(),
                });
            }
        }
        "session.reasoning.delta" => {
            if let Some(delta) = event.pointer("/data/delta").and_then(Value::as_str) {
                events.push(RuntimeEvent::ReasoningDelta {
                    delta: delta.to_string(),
                });
            }
        }
        "session.tool.called" | "session.tool.success" | "session.tool.failed" => {
            let data = event.get("data").cloned().unwrap_or(Value::Null);
            events.push(RuntimeEvent::ToolCall {
                call_id: data
                    .get("callID")
                    .or_else(|| data.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                name: data
                    .get("name")
                    .or_else(|| data.get("tool"))
                    .and_then(Value::as_str)
                    .unwrap_or("tool")
                    .to_string(),
                input: data.get("input").cloned().unwrap_or(Value::Null),
                status: kind.rsplit('.').next().unwrap_or("called").to_string(),
            });
        }
        "session.usage.recorded" => {
            let data = event.get("data").cloned().unwrap_or(Value::Null);
            let number = |key: &str| data.get(key).and_then(Value::as_f64).unwrap_or(0.0);
            events.push(RuntimeEvent::Usage {
                input_tokens: number("inputTokens") as i64,
                output_tokens: number("outputTokens") as i64,
                cache_read: number("cacheRead") as i64,
                cache_write: number("cacheWrite") as i64,
                cost_usd: number("cost"),
            });
        }
        "session.execution.failed" => {
            events.push(RuntimeEvent::Error {
                code: "opencode_failed".into(),
                message: event
                    .pointer("/data/error")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "OpenCode execution failed".into()),
            });
        }
        _ => {}
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_sse_blocks() {
        let mut buffer = "data: {\"type\":\"x\"}\n\ndata: {\"type\":\"y\"}\n\n".to_string();
        let events = extract_events(&mut buffer);
        assert_eq!(events.len(), 2);
        assert!(buffer.is_empty());
    }

    #[test]
    fn normalizes_text_usage_and_failures() {
        let event =
            json!({ "type": "session.text.delta", "data": { "sessionID": "s", "delta": "oi" } });
        assert!(session_matches(&event, "s"));
        assert!(matches!(
            normalize_event("session.text.delta", &event)[0],
            RuntimeEvent::TextDelta { .. }
        ));

        let usage = json!({
            "type": "session.usage.recorded",
            "data": { "sessionID": "s", "inputTokens": 3, "outputTokens": 4, "cost": 0.1 }
        });
        assert!(matches!(
            normalize_event("session.usage.recorded", &usage)[0],
            RuntimeEvent::Usage { input_tokens: 3, output_tokens: 4, cost_usd, .. } if cost_usd > 0.0
        ));

        let failed = json!({ "type": "session.execution.failed", "data": { "sessionID": "s", "error": "boom" } });
        assert!(matches!(
            normalize_event("session.execution.failed", &failed)[0],
            RuntimeEvent::Error { .. }
        ));
    }
}
