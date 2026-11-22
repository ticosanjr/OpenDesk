use chrono::{DateTime, Utc};
use serde_json::Value;

pub(super) fn tool_output(data: &Value) -> String {
    if let Some(content) = data.get("content").and_then(Value::as_array) {
        let text = content
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return text;
        }
    }
    data.get("error")
        .map(|error| error.to_string())
        .unwrap_or_default()
}

pub(super) fn tool_summary(name: &str, input: &str) -> String {
    let parsed = serde_json::from_str::<Value>(input).ok();
    let field = |key: &str| {
        parsed
            .as_ref()
            .and_then(|value| value.get(key))
            .and_then(Value::as_str)
    };
    let target = field("command")
        .or_else(|| field("path"))
        .or_else(|| field("filePath"))
        .or_else(|| field("file_path"))
        .or_else(|| field("pattern"))
        .or_else(|| field("query"));
    match target {
        Some(value) => format!("{} · {}", name, shorten(value, 120)),
        None => format!("{} · {}", name, shorten(input, 120)),
    }
}

fn shorten(value: &str, max: usize) -> String {
    let single = value.replace('\n', " ");
    if single.chars().count() <= max {
        single
    } else {
        format!("{}…", single.chars().take(max).collect::<String>())
    }
}

pub(super) fn truncate(value: String, max: usize) -> (String, bool) {
    if value.len() <= max {
        return (value, false);
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}…", &value[..end]), true)
}

pub(super) fn duration(start: Option<i64>, end: Option<i64>) -> i64 {
    match (start, end) {
        (Some(start), Some(end)) => (end - start).max(0),
        _ => 0,
    }
}

pub(super) fn iso(millis: i64) -> Option<String> {
    DateTime::<Utc>::from_timestamp_millis(millis).map(|value| value.to_rfc3339())
}

pub(super) fn parse_iso_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.timestamp_millis())
}
