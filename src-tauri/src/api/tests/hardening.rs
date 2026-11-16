use super::*;

#[tokio::test]
async fn readyz_is_unauthenticated_and_reports_readiness() {
    let ctx = test_support::context().await;
    let response = router(ctx.state.clone())
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let body: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(body["ready"], true);
}

#[tokio::test]
async fn security_headers_are_present() {
    let ctx = test_support::context().await;
    let response = router(ctx.state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/meta")
                .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
    assert_eq!(
        response
            .headers()
            .get("x-frame-options")
            .and_then(|value| value.to_str().ok()),
        Some("DENY")
    );
}

#[tokio::test]
async fn pwa_responses_carry_csp_when_web_dir_is_configured() {
    let dir = tempfile::tempdir().expect("temp");
    std::fs::write(dir.path().join("index.html"), "<html>ok</html>").expect("index");
    // The router reads OPENDESK_WEB_DIR at build time; set it for this test only.
    // SAFETY: tests in this module do not rely on the previous value.
    unsafe {
        std::env::set_var("OPENDESK_WEB_DIR", dir.path());
    }
    let ctx = test_support::context().await;
    let response = router(ctx.state.clone())
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    unsafe {
        std::env::remove_var("OPENDESK_WEB_DIR");
    }
    assert!(response.status().is_success());
    assert!(response.headers().get("content-security-policy").is_some());
}
