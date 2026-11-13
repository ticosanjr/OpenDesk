use super::*;

#[tokio::test]
async fn dedupe_settings_round_trip() {
    let ctx = test_support::context().await;

    let (status, initial) = call(&ctx, Method::GET, "/api/settings/dedupe", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["enabled"], true);

    let (status, updated) = call(
        &ctx,
        Method::PUT,
        "/api/settings/dedupe",
        Some(json!({ "enabled": false, "cooldown_minutes": 15 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["enabled"], false);
    assert_eq!(updated["cooldown_minutes"], 15);

    let (_, reloaded) = call(&ctx, Method::GET, "/api/settings/dedupe", None).await;
    assert_eq!(reloaded["cooldown_minutes"], 15);
}
