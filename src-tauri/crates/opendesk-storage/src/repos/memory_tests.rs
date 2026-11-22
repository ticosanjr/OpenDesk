#[cfg(test)]
mod tests {
    use super::*;

    async fn note(ctx: &crate::test_support::TestContext, namespace: &str) -> MemoryNote {
        create_note(
            &ctx.db,
            &ctx.workspace_id,
            CreateNoteInput {
                namespace: namespace.into(),
                kind: "semantic".into(),
                title: "Preferência".into(),
                body: "Relatórios em português".into(),
                tags: vec!["relatorio".into()],
                confidence: 0.9,
                trust_level: "verified".into(),
                expires_at: None,
                source: Some("test".into()),
            },
        )
        .await
        .expect("note")
    }

    #[tokio::test]
    async fn acl_restricts_reading_and_writing() {
        let ctx = crate::test_support::context().await;
        let created = note(&ctx, "private").await;
        assert!(
            can_write(&ctx.db, &ctx.workspace_id, "private", "build")
                .await
                .expect("write")
        );
        let namespace = ensure_namespace(&ctx.db, &ctx.workspace_id, "private", None)
            .await
            .expect("namespace");
        set_acl(&ctx.db, &namespace.id, "review", true, false)
            .await
            .expect("acl");
        assert!(
            !can_write(&ctx.db, &ctx.workspace_id, "private", "review")
                .await
                .expect("write")
        );
        assert!(
            list_notes(&ctx.db, &ctx.workspace_id, None, Some("build"), 10)
                .await
                .expect("list")
                .is_empty()
        );
        assert_eq!(
            list_notes(&ctx.db, &ctx.workspace_id, None, Some("review"), 10)
                .await
                .expect("list")
                .len(),
            1
        );
        assert_eq!(created.namespace, "private");
    }

    #[tokio::test]
    async fn consolidates_duplicates_and_archives_expired() {
        let ctx = crate::test_support::context().await;
        note(&ctx, "core").await;
        note(&ctx, "core").await;
        let changed = consolidate(&ctx.db).await.expect("consolidate");
        assert!(changed >= 1);
        assert_eq!(
            list_notes(&ctx.db, &ctx.workspace_id, None, None, 10)
                .await
                .expect("list")
                .len(),
            1
        );

        let old = create_note(
            &ctx.db,
            &ctx.workspace_id,
            CreateNoteInput {
                namespace: "core".into(),
                kind: "semantic".into(),
                title: "Antiga".into(),
                body: "sem confirmação".into(),
                tags: vec![],
                confidence: 0.2,
                trust_level: "observed".into(),
                expires_at: None,
                source: None,
            },
        )
        .await
        .expect("old");
        sqlx::query(
            "UPDATE memory_notes SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now','-60 days')
             WHERE id = ?",
        )
        .bind(&old.id)
        .execute(&ctx.db)
        .await
        .expect("age");
        consolidate(&ctx.db).await.expect("decay");
        let decayed = get_note(&ctx.db, &old.id).await.expect("get");
        assert!(decayed.confidence < 0.2);
        assert!(decayed.archived_at.is_none());
    }

    #[tokio::test]
    async fn search_and_expiry() {
        let ctx = crate::test_support::context().await;
        note(&ctx, "core").await;
        let found = search(&ctx.db, &ctx.workspace_id, "português", None, 10)
            .await
            .expect("search");
        assert_eq!(found.len(), 1);
        let context = render_context(&ctx.db, &ctx.workspace_id, "build", "relatório", 5)
            .await
            .expect("context");
        assert!(context.contains("Relatórios em português"));

        create_note(
            &ctx.db,
            &ctx.workspace_id,
            CreateNoteInput {
                namespace: "core".into(),
                kind: "episodic".into(),
                title: "Temporária".into(),
                body: "expira".into(),
                tags: vec![],
                confidence: 0.1,
                trust_level: "inferred".into(),
                expires_at: Some("2000-01-01T00:00:00.000Z".into()),
                source: None,
            },
        )
        .await
        .expect("expired note");
        assert_eq!(delete_expired(&ctx.db).await.expect("cleanup"), 1);
    }
}
