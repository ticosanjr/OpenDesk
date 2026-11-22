//! Generic CLI harness adapter.
//!
//! Claude Code, Codex, Pi and Hermes all expose a non-interactive mode that
//! prints one JSON object per line (or plain text). The adapter spawns the
//! configured command inside the workspace, normalizes stdout into
//! [`RuntimeEvent`]s and lets the queue settle the run exactly like OpenCode.

use crate::{
    adapters::HarnessSpec,
    error::{AppError, AppResult},
    runtime::{
        AgentRuntimeAdapter, RuntimeCapabilities, RuntimeEvent, RuntimeEventStream,
        RuntimeFileDiff, RuntimeRequest,
    },
};

use async_trait::async_trait;
use futures_util::stream;
use serde_json::Value;
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::broadcast,
};
use tokio_util::sync::CancellationToken;

const CHANNEL_CAPACITY: usize = 4096;
const SUMMARY_LIMIT: usize = 4000;

struct Session {
    request: RuntimeRequest,
    sender: broadcast::Sender<RuntimeEvent>,
    receiver: Mutex<Option<broadcast::Receiver<RuntimeEvent>>>,
    cancel: Mutex<Option<CancellationToken>>,
}

pub struct CliAdapter {
    spec: HarnessSpec,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
}

impl CliAdapter {
    pub fn new(spec: HarnessSpec) -> AppResult<Self> {
        let Some(command) = spec.command.as_deref().map(str::trim) else {
            return Err(AppError::BadRequest(format!(
                "harness {} não tem comando configurado",
                spec.id
            )));
        };
        if command.is_empty() {
            return Err(AppError::BadRequest(format!(
                "harness {} não tem comando configurado",
                spec.id
            )));
        }
        Ok(Self {
            spec,
            sessions: Mutex::new(HashMap::new()),
        })
    }

    fn session(&self, id: &str) -> AppResult<Arc<Session>> {
        self.sessions
            .lock()
            .map_err(|_| AppError::Internal("harness sessions poisoned".into()))?
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("harness session {id}")))
    }
}

#[async_trait]
impl AgentRuntimeAdapter for CliAdapter {
    fn id(&self) -> &str {
        &self.spec.id
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        self.spec.capabilities()
    }

    async fn start(&self, request: RuntimeRequest) -> AppResult<String> {
        let (sender, receiver) = broadcast::channel(CHANNEL_CAPACITY);
        let session_id = uuid::Uuid::now_v7().to_string();
        let session = Arc::new(Session {
            request,
            sender,
            receiver: Mutex::new(Some(receiver)),
            cancel: Mutex::new(None),
        });
        self.sessions
            .lock()
            .map_err(|_| AppError::Internal("harness sessions poisoned".into()))?
            .insert(session_id.clone(), session);
        Ok(session_id)
    }

    async fn prompt(&self, session_id: &str, prompt: &str) -> AppResult<()> {
        let session = self.session(session_id)?;
        let cancel = CancellationToken::new();
        *session
            .cancel
            .lock()
            .map_err(|_| AppError::Internal("harness cancel poisoned".into()))? =
            Some(cancel.clone());
        let spec = self.spec.clone();
        let request = session.request.clone();
        let sender = session.sender.clone();
        let prompt = prompt.to_string();
        tokio::spawn(async move {
            run_process(spec, request, prompt, sender, cancel).await;
        });
        Ok(())
    }

    async fn events(&self, session_id: &str) -> AppResult<RuntimeEventStream> {
        let session = self.session(session_id)?;
        let receiver = session
            .receiver
            .lock()
            .map_err(|_| AppError::Internal("harness events poisoned".into()))?
            .take()
            .ok_or_else(|| {
                AppError::BadRequest("eventos do harness só podem ser lidos uma vez".into())
            })?;
        Ok(boxed_stream(receiver))
    }

    async fn interrupt(&self, session_id: &str) -> AppResult<()> {
        let session = self.session(session_id)?;
        if let Some(cancel) = session
            .cancel
            .lock()
            .map_err(|_| AppError::Internal("harness cancel poisoned".into()))?
            .as_ref()
        {
            cancel.cancel();
        }
        Ok(())
    }
}

fn boxed_stream(receiver: broadcast::Receiver<RuntimeEvent>) -> RuntimeEventStream {
    struct State {
        receiver: broadcast::Receiver<RuntimeEvent>,
        done: bool,
    }
    let state = State {
        receiver,
        done: false,
    };
    Box::pin(stream::unfold(state, |mut state| async move {
        if state.done {
            return None;
        }
        loop {
            match state.receiver.recv().await {
                Ok(event) => {
                    // Errors may still be followed by the terminal event, so
                    // only RunFinished closes the stream.
                    if matches!(event, RuntimeEvent::RunFinished { .. }) {
                        state.done = true;
                    }
                    return Some((Ok(event), state));
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, harness = %"cli", "runtime events lagged");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }))
}

async fn run_process(
    spec: HarnessSpec,
    request: RuntimeRequest,
    prompt: String,
    sender: broadcast::Sender<RuntimeEvent>,
    cancel: CancellationToken,
) {
    let command = spec.command.clone().unwrap_or_default();
    let args = render_args(
        &spec.args(),
        &prompt,
        &request.agent,
        request.model.as_deref(),
        &request.workspace_root,
    );
    if !request.sandbox.command_allowed(&command) {
        let _ = sender.send(RuntimeEvent::Error {
            code: "sandbox_command".into(),
            message: format!("comando {command} não está na allowlist do sandbox"),
        });
        let _ = sender.send(RuntimeEvent::RunFinished {
            status: "failed".into(),
            summary: None,
        });
        return;
    }
    let wrapped = match crate::sandbox::wrap_strict(
        &request.sandbox,
        &request.workspace_root,
        &command,
        &args,
    ) {
        Ok(wrapped) => wrapped,
        Err(error) => {
            let _ = sender.send(RuntimeEvent::Error {
                code: "sandbox_unavailable".into(),
                message: error.to_string(),
            });
            let _ = sender.send(RuntimeEvent::RunFinished {
                status: "failed".into(),
                summary: None,
            });
            return;
        }
    };
    let (program, program_args) = wrapped
        .split_first()
        .map(|(program, args)| (program.clone(), args.to_vec()))
        .unwrap_or((command.clone(), Vec::new()));
    let mut builder = Command::new(&program);
    builder.kill_on_drop(true);
    #[cfg(unix)]
    builder.process_group(0);
    crate::sandbox::scrub_environment(&mut builder, &request.sandbox);
    builder
        .args(&program_args)
        .current_dir(&request.workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("OPENDESK_HARNESS", &spec.id)
        .env("OPENDESK_AGENT", &request.agent)
        .env("OPENDESK_WORKSPACE", &request.workspace_root);
    if let Some(model) = request.model.as_deref() {
        builder.env("OPENDESK_MODEL", model);
    }
    for (key, value) in &request.env {
        builder.env(key, value);
    }

    let mut child = match builder.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = sender.send(RuntimeEvent::Error {
                code: "harness_spawn".into(),
                message: format!("não foi possível iniciar {command}: {error}"),
            });
            let _ = sender.send(RuntimeEvent::RunFinished {
                status: "failed".into(),
                summary: None,
            });
            return;
        }
    };

    let stderr_tail = child.stderr.take().map(|stderr| {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            let mut tail = Vec::new();
            while let Ok(Some(line)) = lines.next_line().await {
                tail.push(line);
                if tail.len() > 40 {
                    tail.remove(0);
                }
            }
            tail.join("\n")
        })
    });

    let Some(stdout) = child.stdout.take() else {
        let _ = sender.send(RuntimeEvent::Error {
            code: "harness_stdout".into(),
            message: "harness não expôs stdout".into(),
        });
        let _ = sender.send(RuntimeEvent::RunFinished {
            status: "failed".into(),
            summary: None,
        });
        return;
    };

    let mut lines = BufReader::new(stdout).lines();
    let mut summary = String::new();
    let mut finished = false;
    let mut cancelled = false;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                cancelled = true;
                let _ = child.kill().await;
                break;
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let line = line.trim_end_matches('\r');
                        if line.trim().is_empty() {
                            continue;
                        }
                        for event in normalize_line(line) {
                            if let RuntimeEvent::TextDelta { delta } = &event
                                && summary.len() < SUMMARY_LIMIT
                            {
                                summary.push_str(delta);
                                if summary.len() > SUMMARY_LIMIT {
                                    summary.truncate(SUMMARY_LIMIT);
                                }
                            }
                            if matches!(event, RuntimeEvent::RunFinished { .. }) {
                                finished = true;
                            }
                            let _ = sender.send(event);
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let _ = sender.send(RuntimeEvent::Error {
                            code: "harness_stdout".into(),
                            message: format!("falha ao ler o harness: {error}"),
                        });
                        break;
                    }
                }
            }
        }
    }

    let status = match child.wait().await {
        Ok(status) => status,
        Err(error) => {
            let _ = sender.send(RuntimeEvent::Error {
                code: "harness_wait".into(),
                message: format!("falha ao aguardar o harness: {error}"),
            });
            let _ = sender.send(RuntimeEvent::RunFinished {
                status: "failed".into(),
                summary: None,
            });
            return;
        }
    };
    let stderr = match stderr_tail {
        Some(handle) => handle.await.unwrap_or_default(),
        None => String::new(),
    };
    if !status.success() && !cancelled && stderr.trim().is_empty() {
        let _ = sender.send(RuntimeEvent::Error {
            code: "harness_exit".into(),
            message: format!("{command} saiu com {status}"),
        });
    }
    if !finished || cancelled {
        let _ = sender.send(RuntimeEvent::RunFinished {
            status: if cancelled {
                "cancelled".into()
            } else if status.success() {
                "success".into()
            } else {
                "failed".into()
            },
            summary: (!summary.trim().is_empty()).then(|| summary.trim().to_string()),
        });
    }
}

/// Substitutes the supported placeholders. When the template has no
/// `{prompt}`, the prompt is appended as the final argument.
fn render_args(
    template: &[String],
    prompt: &str,
    agent: &str,
    model: Option<&str>,
    workspace: &str,
) -> Vec<String> {
    let mut has_prompt = false;
    let mut args = template
        .iter()
        .map(|arg| {
            if arg.contains("{prompt}") {
                has_prompt = true;
            }
            arg.replace("{prompt}", prompt)
                .replace("{agent}", agent)
                .replace("{model}", model.unwrap_or(""))
                .replace("{workspace}", workspace)
        })
        .collect::<Vec<_>>();
    if !has_prompt {
        args.push(prompt.to_string());
    }
    args
}

/// Converts one stdout line into normalized runtime events.
pub fn normalize_line(line: &str) -> Vec<RuntimeEvent> {
    let trimmed = line.trim();
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return vec![RuntimeEvent::TextDelta {
            delta: format!("{line}\n"),
        }];
    };
    let events = normalize_json(&value);
    if events.is_empty() {
        // Unknown structured events are still useful in the live stream.
        let text = value
            .get("text")
            .and_then(Value::as_str)
            .or_else(|| value.get("message").and_then(Value::as_str))
            .or_else(|| value.get("content").and_then(Value::as_str))
            .unwrap_or("");
        if text.is_empty() {
            vec![]
        } else {
            vec![RuntimeEvent::TextDelta {
                delta: text.to_string(),
            }]
        }
    } else {
        events
    }
}

fn normalize_json(value: &Value) -> Vec<RuntimeEvent> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let mut events = Vec::new();

    match kind {
        // ---- Claude Code stream-json ----
        "assistant" => {
            if let Some(content) = value.pointer("/message/content").and_then(Value::as_array) {
                for block in content {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                events.push(RuntimeEvent::TextDelta {
                                    delta: text.to_string(),
                                });
                            }
                        }
                        Some("thinking") => {
                            if let Some(text) = block.get("thinking").and_then(Value::as_str) {
                                events.push(RuntimeEvent::ReasoningDelta {
                                    delta: text.to_string(),
                                });
                            }
                        }
                        Some("tool_use") => events.push(RuntimeEvent::ToolCall {
                            call_id: block
                                .get("id")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            name: block
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("tool")
                                .to_string(),
                            input: block.get("input").cloned().unwrap_or(Value::Null),
                            status: "requested".into(),
                        }),
                        _ => {}
                    }
                }
            }
        }
        "result" => {
            if let Some(usage) = value.get("usage") {
                events.push(usage_event(usage, value));
            }
            let failed = value
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || value.get("subtype").and_then(Value::as_str) == Some("error");
            events.push(RuntimeEvent::RunFinished {
                status: if failed { "failed" } else { "success" }.into(),
                summary: value
                    .get("result")
                    .and_then(Value::as_str)
                    .map(|text| text.chars().take(SUMMARY_LIMIT).collect()),
            });
        }
        // ---- Codex CLI --json ----
        "item.completed" | "item.updated" => {
            if let Some(item) = value.get("item") {
                match item.get("type").and_then(Value::as_str) {
                    Some("agent_message") => {
                        if let Some(text) = item.get("text").and_then(Value::as_str) {
                            events.push(RuntimeEvent::TextDelta {
                                delta: text.to_string(),
                            });
                        }
                    }
                    Some("command_execution") | Some("tool_call") => {
                        events.push(RuntimeEvent::ToolCall {
                            call_id: item
                                .get("id")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            name: item
                                .get("command")
                                .and_then(Value::as_str)
                                .unwrap_or("command")
                                .to_string(),
                            input: item.clone(),
                            status: item
                                .get("status")
                                .and_then(Value::as_str)
                                .unwrap_or("completed")
                                .to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }
        "turn.completed" => {
            if let Some(usage) = value.get("usage") {
                events.push(usage_event(usage, value));
            }
            events.push(RuntimeEvent::RunFinished {
                status: "success".into(),
                summary: None,
            });
        }
        "turn.failed" => {
            events.push(RuntimeEvent::Error {
                code: "harness_turn".into(),
                message: value
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("turn failed")
                    .to_string(),
            });
            events.push(RuntimeEvent::RunFinished {
                status: "failed".into(),
                summary: None,
            });
        }
        // ---- Generic JSON-lines protocol ----
        "text" | "assistant_text" | "delta" => {
            if let Some(text) = value
                .get("text")
                .or_else(|| value.get("delta"))
                .and_then(Value::as_str)
            {
                events.push(RuntimeEvent::TextDelta {
                    delta: text.to_string(),
                });
            }
        }
        "reasoning" | "thinking" => {
            if let Some(text) = value
                .get("delta")
                .or_else(|| value.get("text"))
                .and_then(Value::as_str)
            {
                events.push(RuntimeEvent::ReasoningDelta {
                    delta: text.to_string(),
                });
            }
        }
        "tool_call" | "tool_use" | "tool" => events.push(RuntimeEvent::ToolCall {
            call_id: value
                .get("call_id")
                .or_else(|| value.get("id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("tool")
                .to_string(),
            input: value.get("input").cloned().unwrap_or(Value::Null),
            status: value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("completed")
                .to_string(),
        }),
        "usage" => events.push(usage_event(value, value)),
        "diff" => {
            if let Some(files) = value.get("files").and_then(Value::as_array) {
                let files = files
                    .iter()
                    .map(|file| RuntimeFileDiff {
                        path: file
                            .get("path")
                            .or_else(|| file.get("file"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        status: file
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("modified")
                            .to_string(),
                        patch: file
                            .get("patch")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        additions: file.get("additions").and_then(Value::as_i64).unwrap_or(0),
                        deletions: file.get("deletions").and_then(Value::as_i64).unwrap_or(0),
                    })
                    .collect::<Vec<_>>();
                if !files.is_empty() {
                    events.push(RuntimeEvent::Diff { files });
                }
            }
        }
        "artifact" | "file" => {
            if let Some(path) = value.get("path").and_then(Value::as_str) {
                events.push(RuntimeEvent::Artifact {
                    path: path.to_string(),
                    action: value
                        .get("action")
                        .and_then(Value::as_str)
                        .unwrap_or("CREATED")
                        .to_string(),
                });
            }
        }
        "error" => events.push(RuntimeEvent::Error {
            code: value
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("harness_error")
                .to_string(),
            message: value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("harness error")
                .to_string(),
        }),
        "done" | "finished" | "complete" | "completed" => {
            let failed = value
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| !matches!(status, "success" | "completed" | "done" | "ok"));
            events.push(RuntimeEvent::RunFinished {
                status: if failed { "failed" } else { "success" }.into(),
                summary: value
                    .get("summary")
                    .or_else(|| value.get("result"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
        "session.text.delta" => {
            if let Some(delta) = value.pointer("/data/delta").and_then(Value::as_str) {
                events.push(RuntimeEvent::TextDelta {
                    delta: delta.to_string(),
                });
            }
        }
        _ => {}
    }

    events
}

fn usage_event(usage: &Value, parent: &Value) -> RuntimeEvent {
    let token = |snake: &str, camel: &str| {
        usage
            .get(snake)
            .or_else(|| usage.get(camel))
            .and_then(Value::as_i64)
            .unwrap_or(0)
    };
    RuntimeEvent::Usage {
        input_tokens: token("input_tokens", "inputTokens"),
        output_tokens: token("output_tokens", "outputTokens"),
        cache_read: token("cache_read_input_tokens", "cacheRead")
            .max(token("cache_read", "cacheReadTokens")),
        cache_write: token("cache_creation_input_tokens", "cacheWrite")
            .max(token("cache_write", "cacheWriteTokens")),
        cost_usd: parent
            .get("total_cost_usd")
            .or_else(|| usage.get("cost_usd"))
            .or_else(|| parent.get("cost_usd"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
    }
}

/// Collects files changed during a CLI run so artifacts/provenance work
/// without an OpenCode diff endpoint. Uses `git status --porcelain`.
pub async fn git_diffs(workspace_root: &str) -> Vec<RuntimeFileDiff> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(workspace_root)
        .args(["status", "--porcelain"])
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let output = tokio::time::timeout(Duration::from_secs(15), command.output()).await;
    let Ok(Ok(output)) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            if line.len() < 4 {
                return None;
            }
            let code = line[..2].trim();
            let path = line[3..].trim().trim_matches('"').to_string();
            if path.is_empty() {
                return None;
            }
            let status = if code.contains('A') || code == "??" {
                "added"
            } else if code.contains('D') {
                "deleted"
            } else {
                "modified"
            };
            Some(RuntimeFileDiff {
                path,
                status: status.into(),
                patch: String::new(),
                additions: 0,
                deletions: 0,
            })
        })
        .take(50)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_prompt_placeholder_or_appends_it() {
        let args = render_args(
            &["-p".into(), "{prompt}".into(), "{model}".into()],
            "hello",
            "build",
            Some("m1"),
            "/tmp/ws",
        );
        assert_eq!(args, vec!["-p", "hello", "m1"]);

        let args = render_args(
            &["exec".into(), "--json".into()],
            "hi",
            "build",
            None,
            "/ws",
        );
        assert_eq!(args, vec!["exec", "--json", "hi"]);
    }

    #[test]
    fn normalizes_claude_stream_json() {
        let line = json!({
            "type": "assistant",
            "message": { "content": [
                { "type": "text", "text": "Vou editar." },
                { "type": "tool_use", "id": "t1", "name": "Edit", "input": { "file": "a.rs" } }
            ]}
        })
        .to_string();
        let events = normalize_line(&line);
        assert!(matches!(events[0], RuntimeEvent::TextDelta { .. }));
        assert!(matches!(events[1], RuntimeEvent::ToolCall { .. }));

        let result = json!({
            "type": "result",
            "is_error": false,
            "result": "ok",
            "total_cost_usd": 0.01,
            "usage": { "input_tokens": 10, "output_tokens": 5 }
        })
        .to_string();
        let events = normalize_line(&result);
        assert!(matches!(
            events[0],
            RuntimeEvent::Usage {
                input_tokens: 10,
                ..
            }
        ));
        assert!(matches!(
            events[1],
            RuntimeEvent::RunFinished { ref status, .. } if status == "success"
        ));
    }

    #[test]
    fn normalizes_codex_and_plain_text() {
        let line = json!({
            "type": "item.completed",
            "item": { "id": "item_1", "type": "agent_message", "text": "feito" }
        })
        .to_string();
        assert!(matches!(
            normalize_line(&line)[0],
            RuntimeEvent::TextDelta { .. }
        ));

        let events = normalize_line("compilando...");
        assert!(matches!(events[0], RuntimeEvent::TextDelta { .. }));
    }

    fn spec(command: &str, args: &[&str]) -> HarnessSpec {
        HarnessSpec {
            id: "test-cli".into(),
            name: "Test CLI".into(),
            kind: "cli".into(),
            command: Some(command.into()),
            capabilities_json: "{}".into(),
            status: "READY".into(),
            args_json: serde_json::to_string(args).expect("args"),
            health_args_json: "[]".into(),
            last_health_at: None,
        }
    }

    #[tokio::test]
    async fn streams_plain_stdout_and_finishes() {
        use futures_util::StreamExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = CliAdapter::new(spec("/bin/echo", &["{prompt}"])).expect("adapter");
        let session = adapter
            .start(RuntimeRequest {
                workspace_root: dir.path().to_string_lossy().to_string(),
                agent: "build".into(),
                model: None,
                prompt: String::new(),
                env: Default::default(),
                sandbox: Default::default(),
            })
            .await
            .expect("start");
        adapter
            .prompt(&session, "olá harness")
            .await
            .expect("prompt");
        let mut stream = adapter.events(&session).await.expect("events");
        let mut text = String::new();
        let mut finished = false;
        while let Some(event) = stream.next().await {
            match event.expect("event") {
                RuntimeEvent::TextDelta { delta } => text.push_str(&delta),
                RuntimeEvent::RunFinished { status, .. } => {
                    assert_eq!(status, "success");
                    finished = true;
                }
                _ => {}
            }
        }
        assert!(text.contains("olá harness"), "text: {text}");
        assert!(finished);
    }

    #[test]
    fn parses_porcelain_paths() {
        let rd = |line: &str| {
            let code = line[..2].trim();
            (code.to_string(), line[3..].trim().to_string())
        };
        assert_eq!(rd(" M src/a.rs"), ("M".into(), "src/a.rs".into()));
        assert_eq!(rd("?? novo.md"), ("??".into(), "novo.md".into()));
    }
}
