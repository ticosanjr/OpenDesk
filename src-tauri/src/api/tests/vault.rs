use super::*;
use crate::db::vault;

fn task_input() -> crate::db::tasks::CreateTaskInput {
    crate::db::tasks::CreateTaskInput {
        title: "Vault HITL".into(),
        description: "pede preenchimento".into(),
        execution_mode: None,
        priority: None,
        assigned_agent: None,
        preferred_model: None,
        preferred_harness: None,
        effort_level: None,
        care_level: Some("standard".into()),
        notify_email: None,
        export_to_gdocs: false,
        tags: vec![],
        origin: None,
        parent_task_id: None,
        dedupe: None,
    }
}

#[tokio::test]
async fn vault_crud_never_returns_the_payload_and_audits_injection() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);
    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/vault{query}"),
        Some(json!({
            "id": "stripe",
            "kind": "api_key",
            "label": "Stripe",
            "username": "ops",
            "allowed_domains": ["api.example.com"],
            "secret": "super-secret-value",
            "requires_hitl": false,
            "acls": [{ "subject_kind": "agent", "subject_id": "build" }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["entry"]["id"], "stripe");
    assert!(!created.to_string().contains("super-secret-value"));
    assert!(created["entry"].get("secret_ciphertext").is_none());

    let (status, listed) = call(&ctx, Method::GET, &format!("/api/vault{query}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["entries"].as_array().map(Vec::len), Some(1));

    let (status, detail) = call(&ctx, Method::GET, "/api/vault/stripe", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["acls"].as_array().map(Vec::len), Some(1));
    assert!(!detail.to_string().contains("super-secret-value"));

    let secret = vault::resolve_for_edge(
        &ctx.state.db,
        "stripe",
        "agent",
        "build",
        Some("run-1"),
        Some("https://api.example.com/v1"),
        false,
    )
    .await
    .expect("resolve");
    assert_eq!(secret, "super-secret-value");

    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        "/api/vault/stripe",
        Some(json!({
            "label": "Stripe rotated",
            "allowed_domains": ["api.example.com"],
            "requires_hitl": false,
            "secret": "rotated-secret-value"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let rotated = vault::resolve_for_edge(
        &ctx.state.db,
        "stripe",
        "agent",
        "build",
        None,
        Some("api.example.com"),
        false,
    )
    .await
    .expect("rotated");
    assert_eq!(rotated, "rotated-secret-value");

    let (status, audit) = call(&ctx, Method::GET, "/api/vault/audit?limit=20", None).await;
    assert_eq!(status, StatusCode::OK);
    let outcomes: Vec<&str> = audit["audit"]
        .as_array()
        .expect("audit")
        .iter()
        .filter_map(|row| row["outcome"].as_str())
        .collect();
    assert!(outcomes.contains(&"INJECTED"));
}

#[tokio::test]
async fn vault_acl_replacement_and_request_fill_create_hitl() {
    let ctx = test_support::context().await;
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/vault",
        Some(json!({
            "id": "portal",
            "kind": "login",
            "label": "Portal",
            "allowed_domains": ["portal.example.com"],
            "secret": "portal-secret-value",
            "requires_hitl": true,
            "acls": [{ "subject_kind": "agent", "subject_id": "build" }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, acls) = call(
        &ctx,
        Method::PUT,
        "/api/vault/portal/acl",
        Some(json!({
            "acls": [{ "subject_kind": "agent", "subject_id": "audit", "can_inject": true }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{acls}");
    assert_eq!(acls["acls"][0]["subject_id"], "audit");

    let denied = vault::resolve_for_edge(
        &ctx.state.db,
        "portal",
        "agent",
        "build",
        None,
        Some("portal.example.com"),
        true,
    )
    .await;
    assert!(denied.is_err());

    let task = crate::db::tasks::create(&ctx.state.db, &ctx.workspace_id, task_input())
        .await
        .expect("task");
    let (status, fill) = call(
        &ctx,
        Method::POST,
        "/api/vault/portal/request-fill",
        Some(json!({ "task_id": task.id, "reason": "login no portal" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fill}");
    assert!(fill["interaction"]["id"].is_string());
    assert!(!fill.to_string().contains("portal-secret-value"));
}

#[tokio::test]
async fn vault_rejects_invalid_kinds_and_short_secrets() {
    let ctx = test_support::context().await;
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/vault",
        Some(json!({
            "id": "bad-kind",
            "kind": "plaintext",
            "label": "Bad",
            "secret": "long-enough-secret"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/vault",
        Some(json!({
            "id": "short",
            "kind": "api_key",
            "label": "Short",
            "secret": "abc"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn vault_import_env_creates_metadata_only_entries() {
    let ctx = test_support::context().await;
    let (status, imported) = call(
        &ctx,
        Method::POST,
        "/api/vault/import-env",
        Some(json!({
            "content": "STRIPE_KEY=abcdefgh12345678\n# ignored\nSHORT=abc\n",
            "workspace_id": ctx.workspace_id
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["imported"].as_array().map(Vec::len), Some(1));
    assert!(!imported.to_string().contains("abcdefgh12345678"));
}

#[tokio::test]
async fn bitwarden_import_requires_a_session() {
    let ctx = test_support::context().await;
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/vault/import-bitwarden",
        Some(json!({ "session": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
