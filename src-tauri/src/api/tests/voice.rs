use super::*;

#[tokio::test]
async fn voice_settings_roundtrip_and_missing_engine_errors() {
    let ctx = test_support::context().await;
    let (status, body) = call(&ctx, Method::GET, "/api/voice/status", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["settings"].is_object());

    let (status, saved) = call(
        &ctx,
        Method::PUT,
        "/api/voice/settings",
        Some(json!({
            "stt_command": "definitely-missing-stt",
            "stt_args": ["{audio}"],
            "stt_provider": "openai",
            "tts_command": "definitely-missing-tts",
            "tts_args": ["{text}"],
            "tts_provider": "piper",
            "language": "pt",
            "voice": ""
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["settings"]["stt_command"], "definitely-missing-stt");
    assert_eq!(saved["capabilities"]["available"], false);

    let (status, transcribe) = call(
        &ctx,
        Method::POST,
        "/api/voice/transcribe",
        Some(json!({ "audio_base64": "AAAA", "mime": "audio/wav" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{transcribe}");
    assert!(transcribe["install_hint"].is_string(), "{transcribe}");

    let (status, speak) = call(
        &ctx,
        Method::POST,
        "/api/voice/speak",
        Some(json!({ "text": "olá" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{speak}");

    let (status, tts) = call(
        &ctx,
        Method::POST,
        "/api/voice/tts",
        Some(json!({ "text": "olá" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{tts}");
    assert!(tts["install_hint"].is_string(), "{tts}");

    let audio = "A".repeat(4096);
    let (status, stt) = call(
        &ctx,
        Method::POST,
        "/api/voice/stt",
        Some(json!({ "audio_base64": audio, "mime": "audio/webm" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{stt}");
    assert!(stt["install_hint"].is_string(), "{stt}");
}
