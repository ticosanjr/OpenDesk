//! Pure parsing helpers for chat turns.
//!
//! Kept out of `chat.rs` so the orchestration file stays reviewable.

use crate::db::runs::RunUsage;
use crate::opencode_client::rest_client::ModelRefDto;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChatUsage {
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub tokens_cache_read: i64,
    pub tokens_cache_write: i64,
    pub estimated_cost_usd: f64,
}

pub(crate) fn run_usage(usage: &ChatUsage) -> RunUsage {
    RunUsage {
        tokens_input: usage.tokens_input,
        tokens_output: usage.tokens_output,
        tokens_reasoning: usage.tokens_reasoning,
        tokens_cache_read: usage.tokens_cache_read,
        tokens_cache_write: usage.tokens_cache_write,
        estimated_cost_usd: usage.estimated_cost_usd,
    }
}

pub(crate) fn turn_model(value: &Value, since_ms: i64) -> Option<(Option<String>, String)> {
    let messages = value.get("data").and_then(Value::as_array)?;
    for message in messages.iter().rev() {
        if message.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let created = message
            .pointer("/time/created")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if created < since_ms {
            continue;
        }
        let Some(model_id) = message.get("modelID").and_then(Value::as_str) else {
            continue;
        };
        return Some((
            message
                .get("providerID")
                .and_then(Value::as_str)
                .map(str::to_string),
            model_id.to_string(),
        ));
    }
    None
}

pub(crate) fn is_missing_session_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("404")
        || (lower.contains("session") && (lower.contains("not found") || lower.contains("unknown")))
}

pub(crate) fn parse_model_ref(model: &str) -> Option<ModelRefDto> {
    ModelRefDto::parse(model)
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) fn message_created(message: &Value) -> Option<i64> {
    message
        .pointer("/time/created")
        .and_then(Value::as_i64)
        .or_else(|| message.get("created").and_then(Value::as_i64))
}

pub(crate) fn message_text(message: &Value) -> String {
    message
        .get("content")
        .and_then(Value::as_array)
        .map(|content| {
            content
                .iter()
                .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

pub(crate) fn turn_has_idle(value: &Value, since_ms: i64) -> bool {
    value
        .get("data")
        .and_then(Value::as_array)
        .map(|messages| {
            messages.iter().any(|message| {
                message.get("type").and_then(Value::as_str) == Some("idle")
                    && message_created(message).is_some_and(|created| created >= since_ms)
            })
        })
        .unwrap_or(false)
}

pub(crate) fn turn_has_text(value: &Value, since_ms: i64) -> bool {
    value
        .get("data")
        .and_then(Value::as_array)
        .map(|messages| {
            messages.iter().any(|message| {
                message.get("type").and_then(Value::as_str) == Some("assistant")
                    && message_created(message).is_some_and(|created| created >= since_ms)
                    && !message_text(message).trim().is_empty()
            })
        })
        .unwrap_or(false)
}

pub(crate) fn turn_signature(value: &Value, since_ms: i64) -> String {
    value
        .get("data")
        .and_then(Value::as_array)
        .map(|messages| {
            messages
                .iter()
                .filter(|message| {
                    message.get("type").and_then(Value::as_str) == Some("assistant")
                        && message_created(message).is_some_and(|created| created >= since_ms)
                })
                .map(|message| {
                    let id = message.get("id").and_then(Value::as_str).unwrap_or("");
                    let text = message_text(message);
                    format!("{id}:{}", text.len())
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .unwrap_or_default()
}

pub(crate) fn parse_assistant_since(value: &Value, since_ms: i64) -> (Option<String>, ChatUsage) {
    let mut usage = ChatUsage::default();
    let mut answer = None;
    let Some(messages) = value.get("data").and_then(Value::as_array) else {
        return (answer, usage);
    };
    for message in messages {
        if message.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        if !message_created(message).is_some_and(|created| created >= since_ms) {
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
        let text = message_text(message);
        if !text.trim().is_empty() {
            answer = Some(text);
        }
    }
    (answer, usage)
}
