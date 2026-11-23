use super::checks::{probe, service_state_paths};
use super::live::system::{probe_http_pollers, probe_recipes, probe_vault};
use super::live::{
    probe_signal, probe_telegram, probe_whatsapp,
    system::{probe_discord, probe_qr, probe_slack},
};
use crate::opencode_client::rest_client::RestClient;
use crate::test_support;

#[test]
fn service_state_paths_target_opencode_service_json() {
    let paths = service_state_paths();
    assert!(!paths.is_empty());
    assert!(
        paths
            .iter()
            .all(|path| path.ends_with("opencode/service.json"))
    );
}

#[tokio::test]
async fn probe_reports_unreachable_services() {
    let client = RestClient::new_with_url("http://127.0.0.1:1", "secret".into());
    let probe = probe(&client, "test".into()).await;
    assert!(!probe.online);
    assert!(probe.error.is_some());
    assert_eq!(probe.source, "test");
}

#[tokio::test]
async fn live_probes_degrade_to_not_configured() {
    assert_eq!(probe_telegram(None, None).await.status, "not-configured");
    assert_eq!(probe_signal(None).await.status, "not-configured");
    assert_eq!(probe_whatsapp(None, None).await.status, "not-configured");
}

#[tokio::test]
async fn new_channel_probes_are_not_configured_without_config() {
    assert_eq!(probe_discord(None, None).await.status, "not-configured");
    assert_eq!(probe_slack(None, None, None).await.status, "not-configured");
    assert_eq!(probe_qr(None, None).status, "not-configured");
}

#[tokio::test]
async fn system_probes_read_vault_pollers_and_recipes() {
    let ctx = test_support::context().await;
    let empty = probe_vault(&ctx.state.db).await;
    assert_eq!(empty.status, "ok");

    crate::db::vault::create_entry(
        &ctx.state.db,
        crate::db::vault::VaultEntryInput {
            id: "doctor-entry".into(),
            workspace_id: Some(ctx.workspace_id.clone()),
            kind: "api_key".into(),
            label: "Doctor".into(),
            username: None,
            allowed_domains: Vec::new(),
            secret: "doctor-secret-value".into(),
            requires_hitl: true,
            expires_at: None,
            acls: Vec::new(),
        },
    )
    .await
    .expect("vault entry");
    assert_eq!(probe_vault(&ctx.state.db).await.status, "warning");

    crate::db::io_bus::create_inbound(
        &ctx.state.db,
        &ctx.workspace_id,
        crate::db::io_bus::InboundInput {
            name: "Poll".into(),
            source_type: "HTTP_POLLER".into(),
            config: serde_json::json!({ "url": "https://example.com/feed" }),
            target_agent: None,
            target_column: Some("TODO".into()),
            target_kind: None,
            target_id: None,
            auto_execute: false,
            is_enabled: true,
        },
    )
    .await
    .expect("poller");
    assert_eq!(probe_http_pollers(&ctx.state.db).await.status, "ok");
    assert_eq!(probe_recipes(&ctx.state.db).await.status, "not-configured");
}

#[test]
fn qr_probe_reports_missing_sidecar_and_bad_status() {
    let missing = probe_qr(
        Some(opendesk_channels::whatsapp::WhatsAppQrConfig {
            enabled: true,
            sidecar_command: String::new(),
            ..Default::default()
        }),
        None,
    );
    assert_eq!(missing.status, "error");
    let warning = probe_qr(
        Some(opendesk_channels::whatsapp::WhatsAppQrConfig {
            enabled: true,
            sidecar_command: "node bridge".into(),
            ..Default::default()
        }),
        Some("disconnected".into()),
    );
    assert_eq!(warning.status, "warning");
}

#[tokio::test]
async fn connector_probes_detect_missing_tokens() {
    let discord = probe_discord(
        Some(opendesk_channels::discord::DiscordConfig {
            enabled: true,
            ..Default::default()
        }),
        None,
    )
    .await;
    assert_eq!(discord.status, "error");
    let slack = probe_slack(
        Some(opendesk_channels::slack::SlackConfig {
            enabled: true,
            ..Default::default()
        }),
        None,
        None,
    )
    .await;
    assert_eq!(slack.status, "error");
}
