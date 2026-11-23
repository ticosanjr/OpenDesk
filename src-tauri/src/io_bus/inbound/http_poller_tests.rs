use super::*;

fn config(format: &str) -> PollerConfig {
    PollerConfig {
        url: "https://example.com/feed".into(),
        format: format.into(),
        interval_secs: 60,
        selector: None,
        items_path: None,
        title_path: None,
        body_path: None,
        id_path: None,
        vault_ref: None,
        emit_mode: None,
        max_items: None,
        allow_private_network: false,
    }
}

#[test]
fn parses_rss_and_json_items() {
    let rss = r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>Feed</title><link>https://example.com</link><description>d</description><item><title>Oi</title><link>https://example.com/a</link><description>Corpo</description></item></channel></rss>"#;
    let items = parse_feed(rss).expect("rss");
    assert_eq!(items[0].title, "Oi");
    assert_eq!(items[0].id, "https://example.com/a");

    let json = r#"{"items":[{"id":"1","title":"A","body":"B"}]}"#;
    let items = parse_json(json, &config("json")).expect("json");
    assert_eq!(items[0].id, "1");
    assert_eq!(items[0].body, "B");
}

#[test]
fn content_hash_changes_with_content() {
    assert_ne!(content_hash("a"), content_hash("b"));
    assert_eq!(content_hash("a"), content_hash("a"));
}

#[tokio::test]
async fn polls_json_and_deduplicates_items() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buffer = [0u8; 2048];
            let _ = socket.read(&mut buffer).await;
            let body = r#"{"items":[{"id":"a","title":"A","body":"B"}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    let ctx = crate::test_support::context().await;
    let source = crate::db::io_bus::create_inbound(
        &ctx.state.db,
        &ctx.workspace_id,
        crate::db::io_bus::InboundInput {
            name: "Poll".into(),
            source_type: "HTTP_POLLER".into(),
            config: serde_json::json!({
                "url": format!("http://{address}/feed"),
                "format": "json",
                "allow_private_network": true
            }),
            target_agent: None,
            target_column: Some("TODO".into()),
            target_kind: None,
            target_id: None,
            auto_execute: false,
            is_enabled: true,
        },
    )
    .await
    .expect("source");
    let mut config = config("json");
    config.url = format!("http://{address}/feed");
    config.allow_private_network = true;
    let base = crate::io_bus::ssrf::validate_target(&config.url, true)
        .await
        .expect("url");
    let first = poll_once(&ctx.state, &source, &config, &base)
        .await
        .expect("first poll");
    assert_eq!(first, 1);
    let second = poll_once(&ctx.state, &source, &config, &base)
        .await
        .expect("second poll");
    assert_eq!(second, 0);
}

#[test]
fn parses_html_and_atom_items() {
    let base = reqwest::Url::parse("https://example.com").expect("url");
    let mut config = config("html");
    config.selector = Some("article".into());
    let html = r#"<article><h2>Notícia</h2><a href="/a">link</a><p>Corpo</p></article>"#;
    let items = parse_html(html, &config, &base).expect("html");
    assert_eq!(items[0].title, "Notícia");
    assert_eq!(items[0].id, "https://example.com/a");

    let atom = r#"<?xml version="1.0" encoding="utf-8"?><feed xmlns="http://www.w3.org/2005/Atom"><title>F</title><id>f</id><entry><title>A</title><id>tag:a</id><link href="https://example.com/a"/><summary>S</summary></entry></feed>"#;
    let items = parse_feed(atom).expect("atom");
    assert_eq!(items[0].title, "A");
}

#[tokio::test]
async fn run_rejects_invalid_scheme() {
    let ctx = crate::test_support::context().await;
    let source = crate::db::io_bus::create_inbound(
        &ctx.state.db,
        &ctx.workspace_id,
        crate::db::io_bus::InboundInput {
            name: "Bad".into(),
            source_type: "HTTP_POLLER".into(),
            config: serde_json::json!({ "url": "ftp://example.com/feed" }),
            target_agent: None,
            target_column: Some("TODO".into()),
            target_kind: None,
            target_id: None,
            auto_execute: false,
            is_enabled: true,
        },
    )
    .await
    .expect("source");
    assert!(
        run(ctx.state.clone(), source, CancellationToken::new())
            .await
            .is_err()
    );
}
