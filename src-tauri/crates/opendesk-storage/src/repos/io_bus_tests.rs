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
}
