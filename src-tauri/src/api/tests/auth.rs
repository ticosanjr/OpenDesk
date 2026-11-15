use super::*;

async fn call_with_token(
    ctx: &TestContext,
    method: Method,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(AUTHORIZATION, format!("Bearer {token}"));
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let response = router(ctx.state.clone())
        .oneshot(
            builder
                .body(
                    body.map(|value| Body::from(value.to_string()))
                        .unwrap_or_else(Body::empty),
                )
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

#[tokio::test]
async fn session_token_authenticates_and_can_be_revoked() {
    let ctx = test_support::context().await;
    let (status, created) = call_with_token(
        &ctx,
        Method::POST,
        "/api/auth/session",
        &ctx.state.api_token,
        Some(json!({ "label": "test", "ttl_days": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let token = created["token"].as_str().expect("session token");

    let (status, _) = call_with_token(&ctx, Method::GET, "/api/meta", token, None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, revoked) =
        call_with_token(&ctx, Method::POST, "/api/auth/logout", token, None).await;
    assert_eq!(status, StatusCode::OK, "{revoked}");
    assert_eq!(revoked["revoked"], true);

    let (status, _) = call_with_token(&ctx, Method::GET, "/api/meta", token, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
