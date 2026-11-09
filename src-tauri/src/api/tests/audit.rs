use super::*;
use std::time::Duration;

#[tokio::test]
async fn sensitive_mutations_are_audited() {
    let ctx = test_support::context().await;
    let workspace = format!("?workspace_id={}", ctx.workspace_id);
    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks{workspace}"),
        Some(json!({
            "title": "Auditada",
            "description": "teste de auditoria",
            "execution_mode": "BUILD_DIRECT"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut entries = Vec::new();
    for _ in 0..20 {
        let (status, body) = call(
            &ctx,
            Method::GET,
            &format!("/api/audit{workspace}&limit=10"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        entries = body["entries"].as_array().cloned().unwrap_or_default();
        if !entries.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(!entries.is_empty());
    assert_eq!(entries[0]["action"], "task:POST");
    assert_eq!(entries[0]["status"], 200);
    assert_eq!(entries[0]["workspace_id"], ctx.workspace_id);
}
