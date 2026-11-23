#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("io.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    #[tokio::test]
    async fn inbound_and_outbound_crud_and_bindings() {
        let (_temp, pool) = setup().await;
        let inbound = create_inbound(
            &pool,
            "ws-1",
            InboundInput {
                name: "Dropzone".into(),
                source_type: "FOLDER_WATCH".into(),
                config: serde_json::json!({"path":"input/dropzone"}),
                target_agent: Some("build".into()),
                target_column: Some("TODO".into()),
                target_kind: None,
                target_id: None,
                auto_execute: true,
                is_enabled: true,
            },
        )
        .await
        .expect("inbound");
        let sink = create_outbound(
            &pool,
            "ws-1",
            OutboundInput {
                name: "PDF".into(),
                sink_type: "DOC_COMPILER".into(),
                config: serde_json::json!({"format":"pdf"}),
                is_enabled: true,
            },
        )
        .await
        .expect("sink");
        let task = crate::db::tasks::create(
            &pool,
            "ws-1",
            crate::db::tasks::CreateTaskInput {
                title: "Tarefa IO".into(),
                description: "x".into(),
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
            },
        )
        .await
        .expect("task");
        set_task_bindings(&pool, &task.id, std::slice::from_ref(&sink.id))
            .await
            .expect("bind");
        assert_eq!(
            sinks_for_run(&pool, Some(&task.id), None)
                .await
                .expect("sinks")
                .len(),
            1
        );
        assert_eq!(list_inbound(&pool, "ws-1").await.expect("list").len(), 1);
        assert_eq!(inbound.source_type, "FOLDER_WATCH");
    }

    #[tokio::test]
    async fn resolves_typed_target_and_accepts_new_io_types() {
        let (_temp, pool) = setup().await;
        let recipe = create_inbound(
            &pool,
            "ws-1",
            InboundInput {
                name: "Poller".into(),
                source_type: "HTTP_POLLER".into(),
                config: serde_json::json!({"url":"https://example.com/feed","format":"rss"}),
                target_agent: None,
                target_column: Some("TODO".into()),
                target_kind: Some("RECIPE".into()),
                target_id: Some("rec-1".into()),
                auto_execute: true,
                is_enabled: true,
            },
        )
        .await
        .expect("poller");
        assert_eq!(recipe.source_type, "HTTP_POLLER");
        assert_eq!(recipe.target_kind, "RECIPE");
        assert_eq!(recipe.target_id.as_deref(), Some("rec-1"));

        let inferred = create_inbound(
            &pool,
            "ws-1",
            InboundInput {
                name: "Inferida".into(),
                source_type: "FOLDER_WATCH".into(),
                config: serde_json::json!({"recipe_id":"rec-2"}),
                target_agent: None,
                target_column: Some("TODO".into()),
                target_kind: None,
                target_id: None,
                auto_execute: true,
                is_enabled: true,
            },
        )
        .await
        .expect("inferred");
        assert_eq!(inferred.target_kind, "RECIPE");
        assert_eq!(inferred.target_id.as_deref(), Some("rec-2"));

        let sink = create_outbound(
            &pool,
            "ws-1",
            OutboundInput {
                name: "Hook".into(),
                sink_type: "HTTP_WEBHOOK".into(),
                config: serde_json::json!({"url":"https://example.com/hook"}),
                is_enabled: true,
            },
        )
        .await
        .expect("hook");
        assert_eq!(sink.sink_type, "HTTP_WEBHOOK");

        let missing_id = create_inbound(
            &pool,
            "ws-1",
            InboundInput {
                name: "Sem id".into(),
                source_type: "FOLDER_WATCH".into(),
                config: serde_json::json!({}),
                target_agent: None,
                target_column: Some("TODO".into()),
                target_kind: Some("RECIPE".into()),
                target_id: None,
                auto_execute: true,
                is_enabled: true,
            },
        )
        .await;
        assert!(missing_id.is_err());
    }
}
