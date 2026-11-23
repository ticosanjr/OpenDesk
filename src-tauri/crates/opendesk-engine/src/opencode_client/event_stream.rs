//! Shared parsing for OpenCode's SSE `/api/event` stream.

use serde_json::Value;

/// True when the event belongs to the given OpenCode session.
pub fn session_matches(event: &Value, session_id: &str) -> bool {
    event
        .pointer("/data/sessionID")
        .and_then(Value::as_str)
        .map(|value| value == session_id)
        .unwrap_or(false)
}

/// Extracts every complete `data:` block currently buffered.
pub fn extract_events(buffer: &mut String) -> Vec<Value> {
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

/// Byte-buffer variant for streaming readers: a UTF-8 code point may be split
/// across TCP chunks, so only complete `\n\n` blocks are decoded.
pub fn extract_events_bytes(buffer: &mut Vec<u8>) -> Vec<Value> {
    let mut events = Vec::new();
    while let Some(position) = buffer.windows(2).position(|window| window == b"\n\n") {
        let block = String::from_utf8_lossy(&buffer[..position]).to_string();
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
