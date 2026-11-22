//! Inbound `signal-cli` envelopes and allowlist policy.
//!
//! signal-cli emits one JSON object per line in both `daemon` and JSON-RPC
//! modes, so the parser accepts either shape. Nothing here touches the host
//! state: parsing and policy are pure.

use serde_json::Value;

/// A message accepted for delivery to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingSignal {
    pub source: String,
    pub text: String,
    pub reaction: Option<String>,
    pub quote: Option<String>,
    pub attachments: Vec<String>,
}

pub fn parse_incoming(line: &str) -> Option<IncomingSignal> {
    let value: Value = serde_json::from_str(line).ok()?;
    let envelope = value
        .pointer("/params/envelope")
        .or_else(|| value.get("envelope"))
        .unwrap_or(&value);
    let source = envelope
        .get("sourceNumber")
        .or_else(|| envelope.get("source"))
        .or_else(|| value.pointer("/params/source"))
        .and_then(Value::as_str)?
        .to_string();
    let message = envelope
        .pointer("/dataMessage/message")
        .or_else(|| value.pointer("/params/dataMessage/message"))
        .or_else(|| value.get("message"));
    let text = message
        .and_then(|message| {
            message
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| message.as_str())
        })
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let reaction = envelope
        .pointer("/dataMessage/reaction/emoji")
        .or_else(|| value.pointer("/params/dataMessage/reaction/emoji"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let quote = envelope
        .pointer("/dataMessage/quote/text")
        .or_else(|| value.pointer("/params/dataMessage/quote/text"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let attachments = envelope
        .pointer("/dataMessage/attachments")
        .or_else(|| value.pointer("/params/dataMessage/attachments"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("filename")
                        .or_else(|| item.get("path"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if text.is_empty() && reaction.is_none() && attachments.is_empty() {
        return None;
    }
    Some(IncomingSignal {
        source,
        text,
        reaction,
        quote,
        attachments,
    })
}

pub fn allowed(allowlist: &[String], source: &str) -> bool {
    allowlist
        .iter()
        .map(|value| normalize_number(value))
        .any(|value| value == normalize_number(source))
}

fn normalize_number(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_rpc_and_daemon_envelopes() {
        let json_rpc = r#"{"jsonrpc":"2.0","method":"receive","params":{"envelope":{"sourceNumber":"+15551234567","dataMessage":{"message":"Olá"}}}}"#;
        assert_eq!(
            parse_incoming(json_rpc),
            Some(IncomingSignal {
                source: "+15551234567".into(),
                text: "Olá".into(),
                reaction: None,
                quote: None,
                attachments: vec![]
            })
        );

        let daemon =
            r#"{"envelope":{"source":"+15557654321","dataMessage":{"message":{"message":"Oi"}}}}"#;
        assert_eq!(
            parse_incoming(daemon),
            Some(IncomingSignal {
                source: "+15557654321".into(),
                text: "Oi".into(),
                reaction: None,
                quote: None,
                attachments: vec![]
            })
        );
        assert!(parse_incoming("not json").is_none());
        assert!(parse_incoming(r#"{"envelope":{"sourceNumber":"+1"}}"#).is_none());
    }

    #[test]
    fn allowlist_compares_digits_only() {
        assert!(allowed(&["+55 (11) 99999-0000".into()], "+5511999990000"));
        assert!(!allowed(&["+5511999990000".into()], "+5511888880000"));
        assert!(!allowed(&[], "+5511999990000"));
    }
}
