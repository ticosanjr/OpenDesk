use crate::runtime::{RuntimeEvent, RuntimeFileDiff};

use serde_json::Value;

use super::SUMMARY_LIMIT;

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
