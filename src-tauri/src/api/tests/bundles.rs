use super::*;

#[tokio::test]
async fn personal_context_and_memory_export_import_round_trip() {
    let ctx = test_support::context().await;
    let workspace = format!("workspace_id={}", ctx.workspace_id);
    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/personal-context?{workspace}"),
        Some(json!({
            "scope": "workspace",
            "workspace_id": ctx.workspace_id,
            "instructions": "Responda em português."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");

    let (status, exported) = call(
        &ctx,
        Method::GET,
        &format!("/api/personal-context/export?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    assert_eq!(exported["schema_version"], 1);
    assert_eq!(exported["kind"], "personal_context");
    assert!(
        exported["items"]["workspace"]
            .as_array()
            .is_some_and(|items| items.len() == 1)
    );

    let (status, imported) = call(
        &ctx,
        Method::POST,
        "/api/personal-context/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "mode": "merge",
            "bundle": exported,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["kind"], "personal_context");

    let (status, wrong_kind) = call(
        &ctx,
        Method::POST,
        "/api/personal-context/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "bundle": { "schema_version": 1, "kind": "structured_memory", "items": {} },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{wrong_kind}");
    assert!(
        wrong_kind["message"]
            .as_str()
            .unwrap_or_default()
            .contains("kind incompatível")
    );

    call(
        &ctx,
        Method::POST,
        &format!("/api/memory/namespaces?{workspace}"),
        Some(json!({ "name": "financeiro" })),
    )
    .await;
    call(
        &ctx,
        Method::POST,
        &format!("/api/memory/notes?{workspace}"),
        Some(json!({
            "namespace": "financeiro",
            "title": "Fechamento",
            "body": "Relatório mensal",
        })),
    )
    .await;
    let (status, exported) = call(
        &ctx,
        Method::GET,
        &format!("/api/memory/export?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    assert_eq!(exported["kind"], "structured_memory");
    assert!(
        exported["items"]["notes"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );

    let (status, imported) = call(
        &ctx,
        Method::POST,
        "/api/memory/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "mode": "merge",
            "bundle": exported,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["kind"], "structured_memory");
}

#[tokio::test]
async fn prompt_templates_import_rejects_unknown_keys() {
    let ctx = test_support::context().await;
    let (status, exported) = call(
        &ctx,
        Method::GET,
        &format!(
            "/api/prompt-templates/export?workspace_id={}",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    assert_eq!(exported["kind"], "prompt_templates");

    let mut invalid = exported.clone();
    invalid["items"]["workspace"]["not_a_template"] = json!("x");
    let (status, rejected) = call(
        &ctx,
        Method::POST,
        "/api/prompt-templates/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "bundle": invalid,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected}");
    assert!(
        rejected["message"]
            .as_str()
            .unwrap_or_default()
            .contains("not_a_template")
    );

    let (status, imported) = call(
        &ctx,
        Method::POST,
        "/api/prompt-templates/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "bundle": exported,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
}

#[tokio::test]
async fn starter_pack_exports_and_imports_files() {
    let ctx = test_support::context().await;
    let (status, pack) = call(
        &ctx,
        Method::GET,
        "/api/starter-packs/controle-opendesk/export",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pack}");
    assert_eq!(pack["kind"], "starter_pack");
    assert_eq!(pack["items"]["manifest"]["id"], "controle-opendesk");
    assert!(
        pack["items"]["files"]
            .as_array()
            .is_some_and(|files| !files.is_empty())
    );

    let (status, imported) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "bundle": pack,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert!(
        imported["imported"]["written"]
            .as_u64()
            .is_some_and(|written| written > 0)
    );

    let (status, rejected) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "bundle": { "schema_version": 1, "kind": "packs_bundle", "items": {} },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected}");
}

#[tokio::test]
async fn workspaces_bundle_exports_and_imports_config() {
    let ctx = test_support::context().await;
    let (status, bundle) = call(&ctx, Method::GET, "/api/bundles/workspaces/export", None).await;
    assert_eq!(status, StatusCode::OK, "{bundle}");
    assert_eq!(bundle["kind"], "workspaces_bundle");
    assert!(
        bundle["items"]["workspaces"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );

    let (status, imported) = call(
        &ctx,
        Method::POST,
        "/api/bundles/workspaces/import",
        Some(json!({
            "mode": "merge",
            "bundle": bundle,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["kind"], "workspaces_bundle");
}
