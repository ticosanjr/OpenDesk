use super::*;

#[tokio::test]
async fn passkey_status_is_public_and_login_requires_a_registered_credential() {
    let ctx = test_support::context().await;
    let (status, body) = call(&ctx, Method::GET, "/api/auth/passkey/status", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["registered"], 0);
    assert_eq!(body["enabled"], false);

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/passkey/login/start")
        .header("origin", "http://localhost:1420")
        .body(Body::empty())
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn registration_start_requires_authentication_and_returns_options() {
    let ctx = test_support::context().await;
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/passkey/register/start")
        .header("origin", "http://localhost:1420")
        .body(Body::empty())
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let (status, body) = call(&ctx, Method::POST, "/api/auth/passkey/register/start", None).await;
    // The test request has no Origin/Host header, so the fallback is localhost.
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["ceremony_id"].is_string());
    assert!(body["options"]["publicKey"].is_object());
}

#[tokio::test]
async fn rejects_origin_without_https_or_hostname() {
    let ctx = test_support::context().await;
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/passkey/login/start")
        .header("origin", "http://192.168.0.10:1420");
    let response = router(ctx.state.clone())
        .oneshot(builder.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    // Public endpoint, but the origin is rejected before looking for credentials.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    builder = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/passkey/login/start")
        .header("origin", "http://example.com");
    let response = router(ctx.state.clone())
        .oneshot(builder.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
