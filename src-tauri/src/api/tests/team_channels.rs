use super::*;

#[tokio::test]
async fn discord_and_slack_configs_keep_tokens_secret() {
    let ctx = test_support::context().await;
    let (status, discord) = call(
        &ctx,
        Method::PUT,
        "/api/channels/discord",
        Some(json!({
            "enabled": false,
            "guild_allowlist": ["g1"],
            "channel_allowlist": ["c1"],
            "user_allowlist": ["u1"],
            "agent": "build",
            "notes": "",
            "token": "discord-secret"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{discord}");
    assert_eq!(discord["token_set"], true);
    assert!(!discord.to_string().contains("discord-secret"));

    let (status, slack) = call(
        &ctx,
        Method::PUT,
        "/api/channels/slack",
        Some(json!({
            "enabled": false,
            "channel_allowlist": ["C1"],
            "user_allowlist": ["U1"],
            "agent": "build",
            "notes": "",
            "app_token": "xapp-secret",
            "bot_token": "xoxb-secret"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{slack}");
    assert_eq!(slack["app_token_set"], true);
    assert_eq!(slack["bot_token_set"], true);
    assert!(!slack.to_string().contains("xapp-secret"));
    assert!(!slack.to_string().contains("xoxb-secret"));
}

#[tokio::test]
async fn whatsapp_qr_config_exposes_sidecar_without_starting_it() {
    let ctx = test_support::context().await;
    let (status, value) = call(
        &ctx,
        Method::PUT,
        "/api/channels/whatsapp-qr",
        Some(json!({
            "enabled": false,
            "sidecar_command": "node bridge.js",
            "allowlist": ["5511999999999"],
            "agent": "build",
            "notes": ""
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["config"]["sidecar_command"], "node bridge.js");
    assert_eq!(value["running"], false);
    assert!(value["last_qr"].is_null());
}

#[tokio::test]
async fn team_channel_actions_report_missing_configuration() {
    let ctx = test_support::context().await;
    let (status, _) = call(&ctx, Method::POST, "/api/channels/discord/start", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/channels/discord/send",
        Some(json!({ "channel_id": "c1", "text": "oi" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/channels/slack/send",
        Some(json!({ "channel_id": "C1", "text": "oi" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/channels/whatsapp-qr/send",
        Some(json!({ "to": "5511", "text": "oi" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn enabled_team_channel_without_token_reports_actionable_error() {
    let ctx = test_support::context().await;
    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/channels/discord",
        Some(json!({
            "enabled": true,
            "channel_allowlist": ["c1"],
            "guild_allowlist": [],
            "user_allowlist": [],
            "agent": "build",
            "notes": ""
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/channels/whatsapp-qr",
        Some(json!({
            "enabled": true,
            "sidecar_command": "",
            "allowlist": [],
            "agent": "build",
            "notes": ""
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn team_channel_stop_and_status_endpoints_work() {
    let ctx = test_support::context().await;
    for path in [
        "/api/channels/discord/stop",
        "/api/channels/slack/stop",
        "/api/channels/whatsapp-qr/stop",
    ] {
        let (status, value) = call(&ctx, Method::POST, path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {value}");
    }
    for path in [
        "/api/channels/discord",
        "/api/channels/slack",
        "/api/channels/whatsapp-qr",
    ] {
        let (status, _) = call(&ctx, Method::GET, path, None).await;
        assert_eq!(status, StatusCode::OK);
    }
}
