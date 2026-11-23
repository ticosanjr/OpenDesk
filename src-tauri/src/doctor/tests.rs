use super::checks::{probe, service_state_paths};
use super::live::{probe_signal, probe_telegram, probe_whatsapp};
use crate::opencode_client::rest_client::RestClient;

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
