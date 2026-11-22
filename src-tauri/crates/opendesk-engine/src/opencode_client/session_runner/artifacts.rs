use super::FileWrite;
use crate::opencode_client::rest_client::ModelRefDto;
use opendesk_storage::repos::runs::RunUsage;
use serde_json::Value;

/// Signals for a single chat turn: whether any tool was used and which
/// write/edit/patch operations happened. Chat turns with tools or diffs become
/// first-class executions; text-only turns stay in the chat history.
pub fn chat_turn_signals(value: &Value, since_ms: i64) -> (bool, Vec<FileWrite>) {
    let mut has_tool = false;
    let mut writes = Vec::new();
    let Some(messages) = value.get("data").and_then(Value::as_array) else {
        return (has_tool, writes);
    };
    for message in messages {
        if message.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let created = message
            .pointer("/time/created")
            .and_then(Value::as_i64)
            .or_else(|| message.get("created").and_then(Value::as_i64))
            .unwrap_or(0);
        if created < since_ms {
            continue;
        }
        let Some(content) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in content {
            if part.get("type").and_then(Value::as_str) != Some("tool") {
                continue;
            }
            has_tool = true;
            let name = part.get("name").and_then(Value::as_str).unwrap_or("");
            if name.contains("chat_send_file") {
                let Some(path) = part.pointer("/state/input/path").and_then(Value::as_str) else {
                    continue;
                };
                writes.push(FileWrite {
                    path: path.to_string(),
                    action: "CREATED".into(),
                    diff_patch: None,
                });
                continue;
            }
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
            let Some(path) = input
                .and_then(|input| {
                    input
                        .get("path")
                        .or_else(|| input.get("filePath"))
                        .or_else(|| input.get("file_path"))
                })
                .and_then(Value::as_str)
            else {
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
    (has_tool, writes)
}

pub(super) fn parse_messages(value: &Value) -> (RunUsage, Option<String>, Vec<FileWrite>) {
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

pub(super) fn parse_model_ref(model: &str) -> Option<ModelRefDto> {
    ModelRefDto::parse(model)
}
