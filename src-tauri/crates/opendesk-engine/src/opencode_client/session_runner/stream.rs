use super::PermissionDecision;
use crate::opencode_client::event_stream::session_matches;
use crate::opencode_client::rest_client::RestClient;
use opendesk_storage::events::{EventBus, OpenDeskWsEvent};
use opendesk_storage::repos::{permissions, settings};
use opendesk_storage::secrets::SecretStore;

use serde_json::Value;
use sqlx::SqlitePool;
use std::{path::Path, time::Duration};
use tokio_util::sync::CancellationToken;

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_event(
    event: &Value,
    session_id: &str,
    run_id: &str,
    task_id: Option<&str>,
    workspace_id: &str,
    pool: &SqlitePool,
    secrets: &SecretStore,
    log_path: &Path,
    events: &EventBus,
    rest: &RestClient,
    permissions: &mut Vec<PermissionDecision>,
    cancel: &CancellationToken,
    notifier: Option<&dyn crate::notifier::RunNotifier>,
) -> bool {
    if !session_matches(event, session_id) {
        return false;
    }

    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    append_log(log_path, event);

    match event_type {
        "session.execution.started"
        | "session.execution.succeeded"
        | "session.execution.failed"
        | "session.step.started"
        | "session.step.ended"
        | "session.text.started"
        | "session.text.ended"
        | "session.reasoning.started"
        | "session.reasoning.ended" => {
            publish_chunk(events, run_id, task_id, "event", event_type.to_string());
        }
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
        | "session.tool.input.started"
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
                let created = permissions::create(
                    pool,
                    Some(run_id),
                    task_id,
                    workspace_id,
                    session_id,
                    &request_id,
                    &action,
                    &resources,
                )
                .await;
                if let Err(error) = &created {
                    append_log(
                        log_path,
                        &serde_json::json!({
                            "type": "opendesk.permission.error",
                            "error": format!("{error:#}"),
                        }),
                    );
                }
                events.publish(OpenDeskWsEvent::PermissionRequested {
                    request_id: request_id.clone(),
                    run_id: run_id.to_string(),
                    task_id: task_id.map(str::to_string),
                    workspace_id: workspace_id.to_string(),
                    action: action.clone(),
                    resources: resources.clone(),
                });

                if let Some(notifier) = notifier {
                    let _ = notifier
                        .permission_requested(crate::notifier::PermissionNotice {
                            pool,
                            secrets,
                            workspace_id,
                            run_id,
                            action: &action,
                            resources: &resources,
                            request_id: &request_id,
                        })
                        .await;
                }

                let minutes = settings::get(pool, "permission_timeout_minutes")
                    .await
                    .ok()
                    .flatten()
                    .and_then(|value| value.as_u64())
                    .unwrap_or(10)
                    .clamp(1, 60);
                let wait = permissions::wait_for_decision(
                    pool,
                    &request_id,
                    cancel,
                    Duration::from_secs(minutes * 60),
                )
                .await;
                let decision = match wait {
                    permissions::PermissionWait::Decided(decision) => decision,
                    permissions::PermissionWait::Cancelled => {
                        return false;
                    }
                    permissions::PermissionWait::TimedOut => {
                        let _ = rest
                            .reply_permission(session_id, &request_id, "reject")
                            .await;
                        return true;
                    }
                };

                match rest
                    .reply_permission(session_id, &request_id, &decision)
                    .await
                {
                    Ok(()) => permissions.push(PermissionDecision {
                        request_id,
                        action,
                        resources,
                        decision,
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
    false
}

pub(crate) fn publish_chunk(
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

pub(crate) fn append_log(path: &Path, value: &Value) {
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

pub(super) fn read_tail(path: &Path, max_bytes: usize) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let start = bytes.len().saturating_sub(max_bytes);
    Some(String::from_utf8_lossy(&bytes[start..]).to_string())
}
