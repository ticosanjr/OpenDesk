use super::*;
use crate::test_support;

fn input(id: &str, requires_hitl: bool) -> VaultEntryInput {
    VaultEntryInput {
        id: id.into(),
        workspace_id: None,
        kind: "api_key".into(),
        label: "Stripe".into(),
        username: Some("ops".into()),
        allowed_domains: vec!["api.example.com".into()],
        secret: "super-secret-value".into(),
        requires_hitl,
        expires_at: None,
        acls: vec![VaultAclInput {
            subject_kind: "agent".into(),
            subject_id: "build".into(),
            can_inject: true,
        }],
    }
}

#[tokio::test]
async fn resolves_only_with_acl_domain_and_hitl() {
    let ctx = test_support::context().await;
    create_entry(&ctx.db, input("stripe", false))
        .await
        .expect("entry");

    let secret = resolve_for_edge(
        &ctx.db,
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

    let wrong_actor = resolve_for_edge(
        &ctx.db,
        "stripe",
        "agent",
        "other",
        None,
        Some("api.example.com"),
        false,
    )
    .await;
    assert!(wrong_actor.is_err());

    let wrong_domain = resolve_for_edge(
        &ctx.db,
        "stripe",
        "agent",
        "build",
        None,
        Some("evil.example.com"),
        false,
    )
    .await;
    assert!(wrong_domain.is_err());

    let audit = list_audit(&ctx.db, Some("stripe"), 10)
        .await
        .expect("audit");
    let outcomes: Vec<&str> = audit.iter().map(|row| row.outcome.as_str()).collect();
    assert!(outcomes.contains(&"INJECTED"));
    assert!(outcomes.contains(&"DENIED_ACL"));
    assert!(outcomes.contains(&"DENIED_DOMAIN"));
}

#[tokio::test]
async fn requires_hitl_and_rejects_expired_entries() {
    let ctx = test_support::context().await;
    create_entry(&ctx.db, input("hitl", true))
        .await
        .expect("entry");
    let denied = resolve_for_edge(
        &ctx.db,
        "hitl",
        "agent",
        "build",
        None,
        Some("api.example.com"),
        false,
    )
    .await;
    assert!(denied.is_err());
    let allowed = resolve_for_edge(
        &ctx.db,
        "hitl",
        "agent",
        "build",
        None,
        Some("api.example.com"),
        true,
    )
    .await;
    assert!(allowed.is_ok());

    let mut expired = input("expired", false);
    expired.expires_at = Some("2020-01-01T00:00:00Z".into());
    create_entry(&ctx.db, expired).await.expect("expired");
    let denied = resolve_for_edge(
        &ctx.db,
        "expired",
        "agent",
        "build",
        None,
        Some("api.example.com"),
        false,
    )
    .await;
    assert!(denied.is_err());
}

#[tokio::test]
async fn metadata_never_exposes_the_payload_and_update_rotates_it() {
    let ctx = test_support::context().await;
    create_entry(&ctx.db, input("rotate", false))
        .await
        .expect("entry");
    let listed = list_entries(&ctx.db, Some(&ctx.workspace_id))
        .await
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "rotate");

    update_entry(
        &ctx.db,
        "rotate",
        VaultMetadataInput {
            label: "Stripe rotated".into(),
            username: Some("ops".into()),
            allowed_domains: vec!["api.example.com".into()],
            requires_hitl: false,
            expires_at: None,
            secret: Some("rotated-secret-value".into()),
        },
    )
    .await
    .expect("update");
    let secret = resolve_for_edge(
        &ctx.db,
        "rotate",
        "agent",
        "build",
        None,
        Some("api.example.com"),
        false,
    )
    .await
    .expect("resolve");
    assert_eq!(secret, "rotated-secret-value");
}

#[tokio::test]
async fn wildcard_acl_requires_hitl_and_env_import_skips_duplicates() {
    let ctx = test_support::context().await;
    let mut wildcard = input("wildcard", false);
    wildcard.acls = vec![VaultAclInput {
        subject_kind: "agent".into(),
        subject_id: "*".into(),
        can_inject: true,
    }];
    assert!(create_entry(&ctx.db, wildcard).await.is_err());

    let imported = import_env(
        &ctx.db,
        Some(&ctx.workspace_id),
        "STRIPE_KEY=abcdefgh12345678\n# comment\nSHORT=abc\nSTRIPE_KEY=ignored\n",
    )
    .await
    .expect("import");
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].id, "STRIPE_KEY");
    let again = import_env(
        &ctx.db,
        Some(&ctx.workspace_id),
        "STRIPE_KEY=abcdefgh12345678\n",
    )
    .await
    .expect("second import");
    assert!(again.is_empty());
}

#[tokio::test]
async fn imports_legacy_io_refs_once_and_grants_the_source_actor() {
    let ctx = test_support::context().await;
    crate::secrets::SecretStore::new(&ctx.paths)
        .set("io_bus", "legacy-db", "postgres://legacy-secret")
        .expect("secret");
    let source = crate::repos::io_bus::create_inbound(
        &ctx.db,
        &ctx.workspace_id,
        crate::repos::io_bus::InboundInput {
            name: "Legacy".into(),
            source_type: "SQL_POLLER".into(),
            config: serde_json::json!({ "database_url_ref": "legacy-db" }),
            target_agent: None,
            target_column: Some("TODO".into()),
            target_kind: None,
            target_id: None,
            auto_execute: false,
            is_enabled: false,
        },
    )
    .await
    .expect("source");

    let imported = migrate_legacy_io_secrets(&ctx.db, &ctx.paths)
        .await
        .expect("migrate");
    assert_eq!(imported, 1);
    let secret = resolve_for_edge(&ctx.db, "legacy-db", "sink", &source.id, None, None, false)
        .await
        .expect("resolve");
    assert_eq!(secret, "postgres://legacy-secret");

    let again = migrate_legacy_io_secrets(&ctx.db, &ctx.paths)
        .await
        .expect("second migrate");
    assert_eq!(again, 0);
}
