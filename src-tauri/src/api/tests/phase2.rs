use super::*;

#[tokio::test]
async fn personal_context_crud_and_prompt_preview() {
    let ctx = test_support::context().await;
    let (status, group) = call(
        &ctx,
        Method::POST,
        &format!("/api/personal-context?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "scope": "workspace",
            "workspace_id": ctx.workspace_id,
            "name": "Tom",
            "instructions": "Responda de forma direta e em português.",
            "is_enabled": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{group}");
    let id = group["id"].as_str().expect("id");

    let (status, listed) = call(
        &ctx,
        Method::GET,
        &format!("/api/personal-context?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed.as_array().is_some_and(|items| items.len() == 1));

    let (status, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "title": "Prévia",
            "description": "Produza um resumo executivo."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{task}");
    let task_id = task["id"].as_str().expect("task id");
    let (status, preview) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{task_id}/prompt-preview"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert!(
        preview["prompt"]
            .as_str()
            .unwrap_or_default()
            .contains("Responda de forma direta")
    );
    assert!(
        preview["sections"]
            .as_array()
            .is_some_and(|items| items.iter().any(|section| section["id"] == "personal"))
    );

    let (status, updated) = call(
        &ctx,
        Method::PUT,
        &format!("/api/personal-context/{id}"),
        Some(json!({
            "scope": "global",
            "name": "Tom global",
            "instructions": "Prefira respostas curtas.",
            "is_enabled": true,
            "position": 0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["scope"], "global");

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/personal-context/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, listed) = call(
        &ctx,
        Method::GET,
        &format!("/api/personal-context?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert!(listed.as_array().is_some_and(|items| items.is_empty()));
}
