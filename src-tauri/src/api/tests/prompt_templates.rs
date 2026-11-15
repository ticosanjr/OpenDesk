use super::*;

#[tokio::test]
async fn prompt_templates_are_editable_and_resettable() {
    let ctx = test_support::context().await;
    let (status, initial) = call(
        &ctx,
        Method::GET,
        &format!("/api/prompt/templates?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{initial}");
    assert!(
        initial["fields"]
            .as_array()
            .is_some_and(|items| items.len() >= 10)
    );
    assert!(
        initial["templates"]["output"]
            .as_str()
            .unwrap_or_default()
            .contains("{{output_directory}}")
    );

    let mut templates = initial["templates"].clone();
    templates["permanent"] = json!("PREFIXO CUSTOMIZADO");
    let (status, saved) = call(
        &ctx,
        Method::PUT,
        &format!("/api/prompt/templates?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "templates": templates
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["templates"]["permanent"], "PREFIXO CUSTOMIZADO");

    let (status, reloaded) = call(
        &ctx,
        Method::GET,
        &format!("/api/prompt/templates?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reloaded["templates"]["permanent"], "PREFIXO CUSTOMIZADO");

    let (status, reset) = call(
        &ctx,
        Method::POST,
        &format!(
            "/api/prompt/templates/reset?workspace_id={}",
            ctx.workspace_id
        ),
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reset}");
    assert_ne!(reset["templates"]["permanent"], "PREFIXO CUSTOMIZADO");
}

#[tokio::test]
async fn chat_prompt_preview_shows_channel_and_thread_context() {
    let ctx = test_support::context().await;
    let thread = crate::db::chat::create_thread(
        &ctx.state.db,
        crate::db::chat::CreateThreadInput {
            workspace_id: ctx.workspace_id.clone(),
            title: Some("Telegram · teste".into()),
            agent: "build".into(),
            model: None,
            source: Some("telegram".into()),
            external_id: Some("42".into()),
        },
    )
    .await
    .expect("thread");
    let (status, preview) = call(
        &ctx,
        Method::GET,
        &format!(
            "/api/chat/threads/{}/prompt-preview?content=Gere%20um%20arquivo",
            thread.id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let prompt = preview["prompt"].as_str().unwrap_or_default();
    assert!(prompt.contains("Contexto do canal Telegram"));
    assert!(prompt.contains(&thread.id));
    assert!(prompt.contains("chat_send_file"));
    assert!(
        preview["sections"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == "channel"))
    );
    assert!(
        preview["composed"]
            .as_str()
            .unwrap_or_default()
            .contains("## OpenCode")
    );
}
