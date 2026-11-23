//! Slack Socket Mode envelope parsing and allowlist policy.

use super::config::SlackConfig;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct IncomingSlack {
    pub envelope_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub text: String,
    pub thread_ts: Option<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SlackAction {
    pub envelope_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub action_id: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SlackEvent {
    Message(IncomingSlack),
    Action(SlackAction),
}

pub fn parse_envelope(value: &Value) -> Option<SlackEvent> {
    let envelope_id = value.get("envelope_id")?.as_str()?.to_string();
    let payload = value.get("payload")?;
    match value.get("type").and_then(Value::as_str)? {
        "events_api" => {
            let event = payload.get("event")?;
            if event.get("type").and_then(Value::as_str) != Some("message") {
                return None;
            }
            if event.get("bot_id").is_some() || event.get("subtype").is_some() {
                return None;
            }
            Some(SlackEvent::Message(IncomingSlack {
                envelope_id,
                channel_id: event.get("channel")?.as_str()?.to_string(),
                user_id: event
                    .get("user")
                    .and_then(Value::as_str)
                    .unwrap_or("slack")
                    .to_string(),
                text: event.get("text")?.as_str()?.trim().to_string(),
                thread_ts: event
                    .get("thread_ts")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                files: event
                    .get("files")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| {
                                item.get("url_private")
                                    .or_else(|| item.get("permalink"))
                                    .and_then(Value::as_str)
                            })
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
            }))
        }
        "interactive" => {
            let action = payload.pointer("/actions/0")?;
            Some(SlackEvent::Action(SlackAction {
                envelope_id,
                channel_id: payload.pointer("/channel/id")?.as_str()?.to_string(),
                user_id: payload.pointer("/user/id")?.as_str()?.to_string(),
                action_id: action.get("action_id")?.as_str()?.to_string(),
                value: action
                    .get("value")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            }))
        }
        _ => None,
    }
}

pub fn ack(envelope_id: &str) -> String {
    json!({ "envelope_id": envelope_id }).to_string()
}

pub fn allowed(config: &SlackConfig, event: &SlackEvent) -> bool {
    let (channel, user) = match event {
        SlackEvent::Message(message) => (&message.channel_id, &message.user_id),
        SlackEvent::Action(action) => (&action.channel_id, &action.user_id),
    };
    if !config.channel_allowlist.is_empty() && !config.channel_allowlist.contains(channel) {
        return false;
    }
    if !config.user_allowlist.is_empty() && !config.user_allowlist.contains(user) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_message_and_action() {
        let message = json!({
            "type": "events_api",
            "envelope_id": "e1",
            "payload": { "event": { "type": "message", "channel": "C1", "user": "U1", "text": "oi" } }
        });
        match parse_envelope(&message).expect("message") {
            SlackEvent::Message(message) => assert_eq!(message.text, "oi"),
            _ => panic!("expected message"),
        }
        let action = json!({
            "type": "interactive",
            "envelope_id": "e2",
            "payload": {
                "channel": { "id": "C1" },
                "user": { "id": "U1" },
                "actions": [{ "action_id": "hitl:1:approve", "value": "approve" }]
            }
        });
        match parse_envelope(&action).expect("action") {
            SlackEvent::Action(action) => assert_eq!(action.action_id, "hitl:1:approve"),
            _ => panic!("expected action"),
        }
    }
}
