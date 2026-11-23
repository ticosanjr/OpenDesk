use crate::{
    adapters::HarnessSpec,
    runtime::{RuntimeEvent, RuntimeRequest},
};

use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::broadcast,
};
use tokio_util::sync::CancellationToken;

use super::SUMMARY_LIMIT;
use super::normalize::normalize_line;

pub(super) async fn run_process(
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
                        let line = match request.scrubber.as_deref() {
                            Some(scrubber) => scrubber.scrub(line),
                            None => line.to_string(),
                        };
                        for event in normalize_line(&line) {
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
pub(super) fn render_args(
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
