use super::logs::tail;
use super::models::classify_model_error;

#[test]
fn tail_reads_only_the_requested_lines() {
    let temp = tempfile::TempDir::new().expect("temp");
    let path = temp.path().join("opendesk.log");
    let content = (0..1000)
        .map(|index| format!("line-{index}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, content).expect("log");

    let value = tail(&path, 5);
    let lines = value.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines.last().copied(), Some("line-999"));
}

#[test]
fn tail_returns_empty_for_missing_files() {
    assert_eq!(tail(std::path::Path::new("/nope/missing.log"), 10), "");
}

#[test]
fn classifies_provider_test_failures() {
    assert_eq!(
        classify_model_error("provider.quota: Insufficient balance"),
        "provider_quota"
    );
    assert_eq!(
        classify_model_error(r#"{"reason":"rate_limit","status":429}"#),
        "rate_limited"
    );
    assert_eq!(classify_model_error("401 Unauthorized"), "auth_failed");
    assert_eq!(
        classify_model_error(r#"{"_tag":"ModelNotFoundError"}"#),
        "model_not_found"
    );
    assert_eq!(
        classify_model_error("HTTP 404 Not Found"),
        "engine_unsupported"
    );
    assert_eq!(classify_model_error("request timed out"), "timeout");
    assert_eq!(classify_model_error("boom"), "provider_error");
}
