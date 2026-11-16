use super::*;

#[tokio::test]
async fn vapid_key_is_exposed_for_browser_subscriptions() {
    let ctx = test_support::context().await;
    let (status, body) = call(&ctx, Method::GET, "/api/push/vapid", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["public_key"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
}

#[tokio::test]
async fn registers_lists_and_removes_a_subscription() {
    let ctx = test_support::context().await;
    let (status, created) = call(
        &ctx,
        Method::POST,
        "/api/push/subscribe",
        Some(json!({
            "endpoint": "https://push.example/device-1",
            "keys": { "p256dh": "abc", "auth": "def" },
            "label": "phone"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["label"], "phone");
    assert!(created.get("p256dh").is_none(), "keys must not leak");

    let (status, list) = call(&ctx, Method::GET, "/api/push/subscriptions", None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().map(Vec::len), Some(1));

    let (status, removed) = call(
        &ctx,
        Method::POST,
        "/api/push/unsubscribe",
        Some(json!({ "endpoint": "https://push.example/device-1" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{removed}");
    assert_eq!(removed["removed"], true);
}

#[tokio::test]
async fn rejects_insecure_endpoints_and_empty_test_fanout() {
    let ctx = test_support::context().await;
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/push/subscribe",
        Some(json!({
            "endpoint": "http://push.example/device-1",
            "keys": { "p256dh": "abc", "auth": "def" }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, body) = call(&ctx, Method::POST, "/api/push/test", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
