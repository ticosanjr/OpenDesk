use super::*;
use opendesk_storage::secrets::SecretStore;
use serde_json::json;

#[test]
fn extracts_meta_text_and_interactive_messages() {
    let payload = json!({
        "object": "whatsapp_business_account",
        "entry": [{
            "changes": [{
                "value": {
                    "contacts": [{ "wa_id": "5511999", "profile": { "name": "Ana" } }],
                    "messages": [
                        { "from": "5511999", "id": "wamid.1", "type": "text", "text": { "body": "Oi" } },
                        { "from": "5511888", "id": "wamid.2", "type": "button", "button": { "text": "Sim" } },
                        { "from": "5511777", "id": "wamid.3", "type": "image", "image": { "id": "media-1", "mime_type": "image/jpeg", "caption": "foto" } }
                    ]
                }
            }]
        }]
    });
    let messages = extract_meta_messages(&payload);
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].identity, "5511999");
    assert_eq!(messages[0].name.as_deref(), Some("Ana"));
    assert_eq!(messages[1].text, "Sim");
    assert_eq!(messages[2].media_id.as_deref(), Some("media-1"));
    assert_eq!(messages[2].media_mime.as_deref(), Some("image/jpeg"));
    assert_eq!(messages[2].text, "foto");
}

#[test]
fn derives_safe_media_extensions() {
    assert_eq!(media_extension(Some("image/jpeg"), None), "jpg");
    assert_eq!(
        media_extension(Some("application/pdf"), Some("NF.PDF")),
        "pdf"
    );
    assert_eq!(
        media_extension(Some("application/octet-stream"), Some("../../x.sh")),
        "sh"
    );
    assert_eq!(media_extension(None, None), "bin");
}

#[test]
fn verifies_signature() {
    // RFC 4231 test case 1: 20 bytes of 0x0b and "Hi There".
    let secret = "\u{0b}".repeat(20);
    let body = b"Hi There";
    let encoded = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
    assert!(verify_signature(
        &secret,
        body,
        &format!("sha256={encoded}")
    ));
    assert!(!verify_signature(
        "other",
        body,
        &format!("sha256={encoded}")
    ));
    assert!(!verify_signature(&secret, body, "sha1=abc"));
}

#[test]
fn decodes_hex_signatures() {
    assert_eq!(super::webhook::hex_decode("0aff"), Some(vec![0x0a, 0xff]));
    assert_eq!(super::webhook::hex_decode("abc"), None);
    assert_eq!(super::webhook::hex_decode("zz"), None);
}

#[tokio::test]
async fn config_roundtrip_and_connection_requires_token() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = opendesk_storage::db::init(&dir.path().join("whatsapp.db"))
        .await
        .expect("db");
    let secrets = SecretStore::with_dir(dir.path().join("secrets"));

    let defaults = load_config(&pool).await.expect("defaults");
    assert!(!defaults.enabled);
    assert_eq!(defaults.provider, "structure");
    assert_eq!(defaults.api_version, "v21.0");
    assert!(!connected(&secrets, &defaults).expect("connected"));

    let config = WhatsAppConfig {
        enabled: true,
        provider: "cloud-api".into(),
        phone_number_id: "1234".into(),
        ..WhatsAppConfig::default()
    };
    assert!(validate_config(&config).is_ok());
    save_config(&pool, &config).await.expect("save");
    let loaded = load_config(&pool).await.expect("load");
    assert!(loaded.enabled);
    assert_eq!(loaded.provider, "cloud-api");
    assert_eq!(loaded.phone_number_id, "1234");
    assert!(!connected(&secrets, &loaded).expect("connected"));

    apply_credentials(&secrets, Some(" token "), None).expect("token");
    assert_eq!(
        secrets
            .get(WHATSAPP_SECRET_NAMESPACE, "access_token")
            .expect("get")
            .as_deref(),
        Some("token")
    );
    assert!(connected(&secrets, &loaded).expect("connected"));

    apply_credentials(&secrets, Some(""), None).expect("clear");
    assert!(
        secrets
            .get(WHATSAPP_SECRET_NAMESPACE, "access_token")
            .expect("get")
            .is_none()
    );
}

#[test]
fn rejects_unknown_providers_and_cloud_api_without_phone_number_id() {
    let unknown = WhatsAppConfig {
        provider: "other".into(),
        ..WhatsAppConfig::default()
    };
    assert!(validate_config(&unknown).is_err());

    let incomplete = WhatsAppConfig {
        provider: "cloud-api".into(),
        ..WhatsAppConfig::default()
    };
    assert!(validate_config(&incomplete).is_err());
}
