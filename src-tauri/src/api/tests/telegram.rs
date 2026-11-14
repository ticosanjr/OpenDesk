use super::*;

fn token() -> &'static str {
    "123456:abcdefghijklmnopqrstuvwxyz"
}

#[tokio::test]
async fn telegram_config_persists_without_exposing_the_token() {
    let ctx = test_support::context().await;
    let (status, value) = call(&ctx, Method::GET, "/api/integrations/telegram", None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["token_configured"], false);
    assert_eq!(value["config"]["enabled"], false);

    let (status, value) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/telegram",
        Some(json!({
            "enabled": false,
            "token": token(),
            "allowlist": ["123456", "@rafa", "123456"],
            "agent": ""
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["token_configured"], true);
    assert_eq!(value["config"]["agent"], "build");
    assert_eq!(value["config"]["allowlist"].as_array().unwrap().len(), 2);
    assert!(value.get("token").is_none());

    let stored = ctx
        .state
        .secrets
        .get("telegram", "bot_token")
        .expect("secret")
        .expect("stored");
    assert_eq!(stored, token());

    let (status, value) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/telegram",
        Some(json!({
            "enabled": false,
            "clear_token": true,
            "allowlist": [],
            "agent": "build"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["token_configured"], false);
    assert!(
        ctx.state
            .secrets
            .get("telegram", "bot_token")
            .expect("secret")
            .is_none()
    );
}

#[tokio::test]
async fn telegram_start_requires_token_and_allowlist() {
    let ctx = test_support::context().await;
    let (status, value) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/telegram",
        Some(json!({
            "enabled": true,
            "token": token(),
            "allowlist": []
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["bridge_running"], false);
    assert!(
        value["last_error"]
            .as_str()
            .unwrap_or_default()
            .contains("allowlist"),
        "{value}"
    );

    let (status, value) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/telegram",
        Some(json!({
            "enabled": true,
            "clear_token": true,
            "allowlist": ["123456"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["bridge_running"], false);
    assert!(
        value["last_error"]
            .as_str()
            .unwrap_or_default()
            .contains("token"),
        "{value}"
    );
}

#[tokio::test]
async fn telegram_rejects_malformed_tokens() {
    let ctx = test_support::context().await;
    let (status, value) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/telegram",
        Some(json!({
            "enabled": false,
            "token": "not-a-token",
            "allowlist": []
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
}
