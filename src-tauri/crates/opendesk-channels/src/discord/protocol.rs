//! Discord gateway event parsing and allowlist policy.

use super::config::DiscordConfig;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct IncomingDiscord {
    pub message_id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub user_id: String,
    pub username: String,
    pub content: String,
    pub is_dm: bool,
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DiscordInteraction {
    pub id: String,
    pub channel_id: String,
    pub user_id: String,
    pub custom_id: String,
    pub response: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DiscordEvent {
    Message(IncomingDiscord),
    Interaction(DiscordInteraction),
}

pub fn parse_dispatch(value: &Value) -> Option<DiscordEvent> {
    match value.get("t").and_then(Value::as_str)? {
        "MESSAGE_CREATE" => parse_message(value.get("d")?).map(DiscordEvent::Message),
        "INTERACTION_CREATE" => parse_interaction(value.get("d")?).map(DiscordEvent::Interaction),
        _ => None,
    }
}

pub fn parse_message(data: &Value) -> Option<IncomingDiscord> {
    if data
        .pointer("/author/bot")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let content = data.get("content")?.as_str()?.trim().to_string();
    if content.is_empty() {
        return None;
    }
    Some(IncomingDiscord {
        message_id: data.get("id")?.as_str()?.to_string(),
        channel_id: data.get("channel_id")?.as_str()?.to_string(),
        guild_id: data
            .get("guild_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        user_id: data.pointer("/author/id")?.as_str()?.to_string(),
        username: data
            .pointer("/author/username")
            .and_then(Value::as_str)
            .unwrap_or("discord")
            .to_string(),
        content,
        is_dm: data.get("guild_id").is_none(),
        attachments: data
            .get("attachments")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("url").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

pub fn parse_interaction(data: &Value) -> Option<DiscordInteraction> {
    let component = data
        .pointer("/data/components/0/components/0")
        .or_else(|| data.pointer("/data/components/0/0"));
    let custom_id = component
        .and_then(|component| component.get("custom_id"))
        .and_then(Value::as_str)
        .or_else(|| data.pointer("/data/custom_id").and_then(Value::as_str))?
        .to_string();
    Some(DiscordInteraction {
        id: data.get("id")?.as_str()?.to_string(),
        channel_id: data.get("channel_id")?.as_str()?.to_string(),
        user_id: data
            .pointer("/member/user/id")
            .or_else(|| data.pointer("/user/id"))
            .and_then(Value::as_str)?
            .to_string(),
        custom_id,
        response: component
            .and_then(|component| component.get("value"))
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

pub fn allowed(config: &DiscordConfig, incoming: &IncomingDiscord) -> bool {
    if !config.user_allowlist.is_empty() && !config.user_allowlist.contains(&incoming.user_id) {
        return false;
    }
    if let Some(guild) = incoming.guild_id.as_deref()
        && !config.guild_allowlist.is_empty()
        && !config.guild_allowlist.contains(&guild.to_string())
    {
        return false;
    }
    if !config.channel_allowlist.is_empty()
        && !config.channel_allowlist.contains(&incoming.channel_id)
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_messages_and_interactions() {
        let message = json!({
            "id": "m1",
            "channel_id": "c1",
            "guild_id": "g1",
            "content": "oi",
            "author": { "id": "u1", "username": "ana", "bot": false }
        });
        let parsed = parse_message(&message).expect("message");
        assert_eq!(parsed.content, "oi");
        assert_eq!(parsed.user_id, "u1");

        let interaction = json!({
            "id": "i1",
            "channel_id": "c1",
            "member": { "user": { "id": "u1" } },
            "data": { "components": [{ "components": [{ "custom_id": "hitl:1:approve" }] }] }
        });
        let parsed = parse_interaction(&interaction).expect("interaction");
        assert_eq!(parsed.custom_id, "hitl:1:approve");
    }

    #[test]
    fn allowlist_is_fail_closed() {
        let config = DiscordConfig {
            user_allowlist: vec!["u1".into()],
            channel_allowlist: vec!["c1".into()],
            ..Default::default()
        };
        let incoming = IncomingDiscord {
            message_id: "m".into(),
            channel_id: "c1".into(),
            guild_id: Some("g".into()),
            user_id: "u2".into(),
            username: "x".into(),
            content: "x".into(),
            is_dm: false,
            attachments: Vec::new(),
        };
        assert!(!allowed(&config, &incoming));
    }
}
