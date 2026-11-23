use super::calendar::calendar_event_payload;
use super::upload::{build_gmail_raw, extract_doc_text};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::json;

#[test]
fn timed_and_all_day_calendar_payloads_match_google_contract() {
    let timed = calendar_event_payload(
        "Planejamento",
        Some("Sala 1"),
        Some("Sala 2"),
        "2026-11-10T12:00:00Z",
        "2026-11-10T13:00:00Z",
        Some("America/Sao_Paulo"),
        false,
    );
    assert_eq!(timed["start"]["dateTime"], "2026-11-10T12:00:00Z");
    assert_eq!(timed["start"]["timeZone"], "America/Sao_Paulo");
    assert!(timed["start"].get("date").is_none());

    let all_day = calendar_event_payload(
        "Feriado",
        None,
        None,
        "2026-11-10T00:00:00Z",
        "2026-11-11T00:00:00Z",
        None,
        true,
    );
    assert_eq!(all_day["start"]["date"], "2026-11-10");
    assert_eq!(all_day["end"]["date"], "2026-11-11");
    assert!(all_day["start"].get("dateTime").is_none());
}

#[test]
fn builds_a_decodable_gmail_raw_message() {
    let raw = build_gmail_raw(
        "sender@example.com",
        "recipient@example.com",
        "Resumo",
        "<p>ok</p>",
        vec![(
            "report.txt".into(),
            b"conteudo".to_vec(),
            "text/plain".into(),
        )],
    )
    .expect("raw");
    let decoded = String::from_utf8(URL_SAFE_NO_PAD.decode(raw).expect("base64")).expect("utf8");
    assert!(decoded.contains("From: sender@example.com"));
    assert!(decoded.contains("To: recipient@example.com"));
    assert!(decoded.contains("Subject: Resumo"));
    assert!(decoded.contains("<p>ok</p>"));
    assert!(decoded.contains("report.txt"));
}

#[test]
fn extracts_nested_document_text() {
    let document = json!({
        "content": [{
            "paragraph": {
                "elements": [
                    { "textRun": { "content": "Olá " } },
                    { "textRun": { "content": "mundo" } }
                ]
            }
        }]
    });
    assert_eq!(extract_doc_text(&document), "Olá mundo");
}
