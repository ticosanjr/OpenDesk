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
