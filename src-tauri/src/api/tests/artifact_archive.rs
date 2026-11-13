use super::*;

async fn seed_artifact(ctx: &TestContext) -> String {
    let (status, body) = call(
        ctx,
        Method::POST,
        "/api/test/seed",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "run": { "status": "SUCCESS", "agent": "build", "model": "glm" },
            "artifacts": [{ "file_path": "relatorio.md", "content": "# ok" }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["artifact_ids"][0]
        .as_str()
        .expect("artifact id")
        .to_string()
}

#[tokio::test]
async fn artifact_archive_enforces_the_workspace_boundary() {
    let ctx = test_support::context().await;
    let id = seed_artifact(&ctx).await;

    let (status, body) = call(
        &ctx,
        Method::GET,
        &format!("/api/artifacts/{id}/archive"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("outside the workspace"),
        "{body}"
    );

    let (status, _) = call(
        &ctx,
        Method::GET,
        &format!("/api/artifacts/{id}/content"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
