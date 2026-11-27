use super::*;

#[tokio::test]
async fn stores_searches_and_restricts_notes_by_acl() {
    let ctx = test_support::context().await;
    let (status, namespace) = call(
        &ctx,
        Method::POST,
        "/api/memory/namespaces",
        Some(json!({ "name": "financeiro", "description": "Notas financeiras" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{namespace}");
    let namespace_id = namespace["id"].as_str().expect("namespace id");

    call(
        &ctx,
        Method::PUT,
        &format!("/api/memory/namespaces/{namespace_id}/acl"),
        Some(json!({ "agent": "review", "can_read": true, "can_write": true })),
    )
    .await;

    let (status, note) = call(
        &ctx,
        Method::POST,
        "/api/memory/notes?agent=review",
        Some(json!({
            "namespace": "financeiro",
            "title": "Fechamento",
            "body": "Relatório de fechamento mensal em português",
            "tags": ["relatorio"],
            "confidence": 0.9,
            "trust_level": "verified"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{note}");

    let (status, found) = call(
        &ctx,
        Method::GET,
        "/api/memory/search?q=fechamento&agent=review",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{found}");
    assert_eq!(found.as_array().map(Vec::len), Some(1));

    let (_, hidden) = call(&ctx, Method::GET, "/api/memory/notes?agent=build", None).await;
    assert_eq!(hidden.as_array().map(Vec::len), Some(0));

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/memory/notes?agent=build",
        Some(json!({
            "namespace": "financeiro",
            "title": "Bloqueada",
            "body": "não deveria entrar"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn updates_and_deletes_notes() {
    let ctx = test_support::context().await;
    let (_, note) = call(
        &ctx,
        Method::POST,
        "/api/memory/notes",
        Some(json!({ "namespace": "core", "title": "A", "body": "B" })),
    )
    .await;
    let id = note["id"].as_str().expect("id");
    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/memory/notes/{id}"),
        Some(json!({ "title": "A2", "confidence": 0.8 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["title"], "A2");

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/memory/notes/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn skill_eval_returns_an_empty_list_for_a_clean_workspace() {
    let ctx = test_support::context().await;
    let (status, list) = call(&ctx, Method::GET, "/api/skills/eval", None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn usage_scope_separates_task_and_chat_prompts() {
    let ctx = test_support::context().await;
    for (title, scope) in [
        ("Regra Normal", "normal"),
        ("Regra Chat", "chat"),
        ("Regra Ambos", "both"),
    ] {
        let (status, note) = call(
            &ctx,
            Method::POST,
            "/api/memory/notes",
            Some(json!({
                "namespace": "core",
                "title": title,
                "body": format!("{title} no prompt"),
                "usage_scope": scope
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{note}");
        assert_eq!(note["usage_scope"], scope);

        let (status, group) = call(
            &ctx,
            Method::POST,
            &format!("/api/personal-context?workspace_id={}", ctx.workspace_id),
            Some(json!({
                "scope": "workspace",
                "workspace_id": ctx.workspace_id,
                "name": title,
                "instructions": format!("{title} pessoal"),
                "is_enabled": true,
                "usage_scope": scope
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{group}");
        assert_eq!(group["usage_scope"], scope);
    }

    let (status, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({ "title": "Tarefa", "description": "Resumo do dia" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{task}");
    let task_id = task["id"].as_str().expect("task");
    let (status, preview) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{task_id}/prompt-preview"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let prompt = preview["prompt"].as_str().expect("prompt");
    assert!(prompt.contains("Regra Normal"), "{prompt}");
    assert!(prompt.contains("Regra Ambos"), "{prompt}");
    assert!(!prompt.contains("Regra Chat"), "{prompt}");

    let thread = crate::db::chat::create_thread(
        &ctx.state.db,
        crate::db::chat::CreateThreadInput {
            workspace_id: ctx.workspace_id.clone(),
            title: Some("Chat escopo".into()),
            agent: "build".into(),
            model: None,
            source: Some("desktop".into()),
            external_id: None,
        },
    )
    .await
    .expect("thread");
    let (status, preview) = call(
        &ctx,
        Method::GET,
        &format!("/api/chat/threads/{}/prompt-preview?content=Oi", thread.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let prompt = preview["prompt"].as_str().expect("prompt");
    assert!(prompt.contains("Regra Chat"), "{prompt}");
    assert!(prompt.contains("Regra Ambos"), "{prompt}");
    assert!(!prompt.contains("Regra Normal"), "{prompt}");
}

#[tokio::test]
async fn api_created_memory_defaults_to_normal() {
    let ctx = test_support::context().await;
    let (status, note) = call(
        &ctx,
        Method::POST,
        "/api/memory/notes",
        Some(json!({ "namespace": "core", "title": "Padrão", "body": "Normal" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{note}");
    assert_eq!(note["usage_scope"], "normal");

    let (status, group) = call(
        &ctx,
        Method::POST,
        &format!("/api/personal-context?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "scope": "workspace",
            "workspace_id": ctx.workspace_id,
            "instructions": "Regra padrão",
            "is_enabled": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{group}");
    assert_eq!(group["usage_scope"], "normal");
}
