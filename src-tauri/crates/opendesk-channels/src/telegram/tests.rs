//! Unit tests for the pure Telegram transport helpers.

use super::*;
use serde_json::json;

#[test]
fn parses_private_and_group_updates() {
    let value = json!({
        "ok": true,
        "result": [
            {
                "update_id": 10,
                "message": {
                    "message_id": 1,
                    "from": { "id": 42, "is_bot": false, "username": "rafa" },
                    "chat": { "id": 42, "type": "private" },
                    "text": "  olá  "
                }
            },
            {
                "update_id": 11,
                "message": {
                    "message_id": 2,
                    "from": { "id": 7, "is_bot": true },
                    "chat": { "id": 7, "type": "private" },
                    "text": "bot"
                }
            },
            {
                "update_id": 12,
                "message": {
                    "message_id": 3,
                    "from": { "id": 42, "is_bot": false },
                    "chat": { "id": -100123, "type": "group", "title": "Equipe" },
                    "text": "/help"
                }
            }
        ]
    });
    let updates = parse_updates(&value);
    assert_eq!(updates.len(), 2);
    assert_eq!(parse_updates(&value["result"]).len(), 2);
    let TelegramUpdate::Message(first) = &updates[0] else {
        panic!("expected message");
    };
    let TelegramUpdate::Message(second) = &updates[1] else {
        panic!("expected message");
    };
    assert_eq!(first.message_id, 1);
    assert_eq!(first.chat_id, "42");
    assert_eq!(first.text, "olá");
    assert_eq!(second.message_id, 3);
    assert_eq!(second.chat_id, "-100123");
    assert_eq!(second.chat_title.as_deref(), Some("Equipe"));
}

#[test]
fn parses_callback_queries() {
    let value = json!({
        "update_id": 20,
        "callback_query": {
            "id": "cb-1",
            "from": { "id": 42, "is_bot": false, "username": "rafa" },
            "message": {
                "message_id": 9,
                "chat": { "id": 42, "type": "private" }
            },
            "data": "ti:abc:1"
        }
    });
    let callback = parse_callback_update(&value).expect("callback");
    assert_eq!(callback.callback_id, "cb-1");
    assert_eq!(callback.chat_id, "42");
    assert_eq!(callback.message_id, 9);
    assert_eq!(callback.data, "ti:abc:1");
}

#[test]
fn ignores_messages_without_text() {
    let value = json!({
        "update_id": 1,
        "message": {
            "from": { "id": 1, "is_bot": false },
            "chat": { "id": 1, "type": "private" },
            "photo": []
        }
    });
    assert!(parse_update(&value).is_none());
}

#[test]
fn parses_documents_with_caption() {
    let value = json!({
        "update_id": 2,
        "message": {
            "message_id": 11,
            "from": { "id": 1, "is_bot": false },
            "chat": { "id": 1, "type": "private" },
            "caption": "Analise este PDF",
            "document": {
                "file_id": "file-1",
                "file_name": "contrato.pdf",
                "mime_type": "application/pdf",
                "file_size": 2048
            }
        }
    });
    let message = parse_update(&value).expect("message");
    assert_eq!(message.text, "Analise este PDF");
    let file = message.file.expect("file");
    assert_eq!(file.file_id, "file-1");
    assert_eq!(file.file_name, "contrato.pdf");
    assert_eq!(file.mime_type.as_deref(), Some("application/pdf"));
    assert_eq!(file.size, Some(2048));
}

#[test]
fn picks_the_largest_photo_without_caption() {
    let value = json!({
        "update_id": 3,
        "message": {
            "message_id": 12,
            "from": { "id": 1, "is_bot": false },
            "chat": { "id": 1, "type": "private" },
            "photo": [
                { "file_id": "small", "file_size": 100 },
                { "file_id": "large", "file_size": 900 }
            ]
        }
    });
    let message = parse_update(&value).expect("message");
    assert!(message.text.is_empty());
    let file = message.file.expect("file");
    assert_eq!(file.file_id, "large");
    assert_eq!(file.file_name, "photo-12.jpg");
}

#[test]
fn sanitizes_incoming_file_names() {
    assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
    assert_eq!(safe_file_name("relatório final.pdf"), "relat_rio final.pdf");
    assert_eq!(safe_file_name(""), "arquivo");
    assert!(safe_file_name(&"a".repeat(300)).chars().count() <= 100);
}

#[test]
fn allowlist_accepts_numeric_ids_and_usernames() {
    let allowlist = vec!["42".into(), "-100123".into(), "@Rafa".into()];
    assert!(allowed(&allowlist, "42", Some("other")));
    assert!(allowed(&allowlist, "-100123", None));
    assert!(allowed(&allowlist, "99", Some("rafa")));
    assert!(!allowed(&allowlist, "99", Some("someone")));
    assert!(!allowed(&[], "42", Some("rafa")));
}

#[test]
fn chunks_long_messages_at_line_boundaries() {
    let text = (1..=200)
        .map(|index| format!("linha {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let parts = chunks(&text, 200);
    assert!(parts.len() > 1);
    assert!(parts.iter().all(|part| part.chars().count() <= 200));
    assert_eq!(parts.concat().replace('\n', ""), text.replace('\n', ""));
}

#[test]
fn token_shape_rejects_obvious_paste_errors() {
    assert!(valid_token_shape("123456:abcdefghijklmnopqrstuvwxyz"));
    assert!(!valid_token_shape("not-a-token"));
    assert!(!valid_token_shape("123456:short"));
    assert!(!valid_token_shape("abc:abcdefghijklmnopqrstuvwxyz"));
}
