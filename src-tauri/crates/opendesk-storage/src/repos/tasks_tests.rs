#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("tasks.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    fn input() -> CreateTaskInput {
        CreateTaskInput {
            title: "Relatório".into(),
            description: "Produza o relatório".into(),
            execution_mode: Some("PLAN_THEN_BUILD".into()),
            priority: Some(9),
            assigned_agent: Some("build".into()),
            preferred_model: None,
            preferred_harness: None,
            effort_level: Some("high".into()),
            care_level: Some("standard".into()),
            notify_email: Some("ops@example.com".into()),
            export_to_gdocs: true,
            tags: vec!["financeiro".into(), "mensal".into()],
            origin: None,
            parent_task_id: None,
            dedupe: None,
        }
    }

    #[tokio::test]
    async fn fork_can_pin_a_model() {
        let (_temp, pool) = setup().await;
        let task = create(&pool, "ws-1", input()).await.expect("create");
        let fork = fork_with_model(&pool, &task.id, Some("opencode-go/glm-5.3"))
            .await
            .expect("fork");
        assert_eq!(fork.forked_from_task_id.as_deref(), Some(task.id.as_str()));
        assert_eq!(fork.preferred_model.as_deref(), Some("opencode-go/glm-5.3"));
        assert!(fork.model_override);
        assert!(fork.title.contains("glm-5.3"));
    }

    #[tokio::test]
    async fn deduplicates_active_and_recent_tasks() {
        let (_temp, pool) = setup().await;
        let first = create(&pool, "ws-1", input()).await.expect("first");
        let duplicate = create(&pool, "ws-1", input()).await.expect("duplicate");
        assert!(duplicate.deduplicated);
        assert_eq!(duplicate.id, first.id);

        move_to(&pool, &first.id, "DONE", None).await.expect("done");
        let cooldown = create(&pool, "ws-1", input()).await.expect("cooldown");
        assert!(cooldown.deduplicated);
        assert_eq!(cooldown.id, first.id);

        crate::dedupe::set_settings(
            &pool,
            crate::dedupe::DedupeSettings {
                enabled: true,
                cooldown_minutes: 0,
            },
        )
        .await
        .expect("settings");
        let fresh = create(&pool, "ws-1", input()).await.expect("fresh");
        assert!(!fresh.deduplicated);
        assert_ne!(fresh.id, first.id);

        let mut different = input();
        different.description = "Outro escopo".into();
        let other = create(&pool, "ws-1", different).await.expect("other");
        assert!(!other.deduplicated);
        assert_ne!(other.id, fresh.id);
    }

    #[tokio::test]
    async fn forced_creation_bypasses_dedupe() {
        let (_temp, pool) = setup().await;
        let first = create(&pool, "ws-1", input()).await.expect("first");
        let mut forced = input();
        forced.dedupe = Some(false);
        let second = create(&pool, "ws-1", forced).await.expect("forced");
        assert!(!second.deduplicated);
        assert_ne!(second.id, first.id);
    }

    #[tokio::test]
    async fn task_lifecycle() {
        let (_temp, pool) = setup().await;
        let task = create(&pool, "ws-1", input()).await.expect("create");
        assert_eq!(task.status, "TODO");
        assert_eq!(task.priority, 4);
        assert_eq!(task.execution_mode, "PLAN_THEN_BUILD");
        assert_eq!(task.tags(), vec!["financeiro", "mensal"]);
        assert!(task.export_to_gdocs);

        let updated = update(
            &pool,
            &task.id,
            UpdateTaskInput {
                title: Some("Relatório v2".into()),
                priority: Some(1),
                assigned_agent: Some("explore".into()),
                preferred_model: Some("opencode-go/glm-5.3#high".into()),
                ..Default::default()
            },
        )
        .await
        .expect("update");
        assert_eq!(updated.title, "Relatório v2");
        assert_eq!(updated.priority, 1);
        assert_eq!(updated.assigned_agent.as_deref(), Some("explore"));
        assert_eq!(
            updated.preferred_model.as_deref(),
            Some("opencode-go/glm-5.3#high")
        );

        let moved = move_to(&pool, &task.id, "IN_PROGRESS", Some(2))
            .await
            .expect("move");
        assert_eq!(moved.status, "IN_PROGRESS");
        assert_eq!(moved.position_index, 2);

        assert!(
            set_plan(&pool, &task.id, "# Plano")
                .await
                .expect("plan")
                .plan_markdown
                .is_some()
        );
        set_phase_status(&pool, &task.id, Some("running"), Some("IN_PROGRESS"))
            .await
            .expect("phase");
        assert_eq!(
            get(&pool, &task.id)
                .await
                .expect("get")
                .phase_status
                .as_deref(),
            Some("running")
        );

        set_output_dir(&pool, &task.id, "/tmp/output")
            .await
            .expect("output");
        assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 1);
        delete(&pool, &task.id).await.expect("delete");
        assert!(get(&pool, &task.id).await.is_err());
    }

    fn workflow_task(status: &str, phase: Option<&str>, job: Option<&str>) -> Task {
        Task {
            id: "task-1".into(),
            workspace_id: "ws-1".into(),
            title: "Tarefa".into(),
            description: "Descrição".into(),
            status: status.into(),
            phase: None,
            phase_status: phase.map(str::to_string),
            priority: 2,
            execution_mode: "AUTO_DSH".into(),
            assigned_agent: None,
            preferred_model: None,
            preferred_harness: None,
            agent_override: false,
            model_override: false,
            effort_level: "medium".into(),
            care_level: "standard".into(),
            plan_markdown: None,
            output_dir_path: None,
            notify_email: None,
            export_to_gdocs: false,
            tags_json: "[]".into(),
            position_index: 0,
            last_run_id: None,
            last_run_status: None,
            forked_from_task_id: None,
            last_run_at: None,
            blocked_reason: None,
            cancelled_at: None,
            origin: "MANUAL".into(),
            parent_task_id: None,
            dedupe_key: None,
            held: false,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            deduplicated: false,
            active_job_status: job.map(str::to_string),
            active_run_status: None,
            workflow_state: String::new(),
            workflow_reason: None,
        }
    }

    #[test]
    fn derives_a_single_workflow_state() {
        assert_eq!(
            derive_workflow(&workflow_task("TODO", Some("forked"), None)),
            ("READY", Some("ready"))
        );
        assert_eq!(
            derive_workflow(&workflow_task("TODO", Some("queued"), Some("QUEUED"))),
            ("QUEUED", Some("queued"))
        );
        assert_eq!(
            derive_workflow(&workflow_task(
                "IN_PROGRESS",
                Some("running"),
                Some("RUNNING")
            )),
            ("RUNNING", None)
        );
        assert_eq!(
            derive_workflow(&workflow_task("IN_PROGRESS", None, None)),
            ("BLOCKED", Some("in_progress_without_run"))
        );
        assert_eq!(
            derive_workflow(&workflow_task("IN_REVIEW", Some("awaiting_approval"), None)),
            ("NEEDS_APPROVAL", Some("approval"))
        );
        assert_eq!(
            derive_workflow(&workflow_task(
                "IN_REVIEW",
                Some("awaiting_permission"),
                None
            )),
            ("WAITING_INPUT", Some("permission"))
        );
        assert_eq!(
            derive_workflow(&workflow_task("BLOCKED", Some("failed"), None)),
            ("BLOCKED", Some("failed"))
        );
        assert_eq!(
            derive_workflow(&workflow_task("BLOCKED", None, None)),
            ("BLOCKED", Some("manual_block"))
        );
        assert_eq!(
            derive_workflow(&workflow_task("DONE", None, None)),
            ("DONE", None)
        );
        assert_eq!(
            derive_workflow(&workflow_task("CANCELLED", None, None)),
            ("CANCELLED", None)
        );
    }

    #[tokio::test]
    async fn get_exposes_ready_workflow_state() {
        let (_temp, pool) = setup().await;
        let task = create(&pool, "ws-1", input()).await.expect("create");
        let loaded = get(&pool, &task.id).await.expect("get");
        assert_eq!(loaded.workflow_state, "READY");
        assert_eq!(loaded.workflow_reason.as_deref(), Some("ready"));
    }

    #[test]
    fn derives_a_title_from_the_description_when_missing() {
        assert_eq!(derive_title("  Primeira linha\nSegunda"), "Primeira linha");
        assert_eq!(derive_title("   "), "Nova tarefa");
        let long = "a".repeat(100);
        let derived = derive_title(&long);
        assert_eq!(derived.chars().count(), 81);
        assert!(derived.ends_with('…'));
    }

    #[tokio::test]
    async fn rejects_invalid_task_input() {
        let (_temp, pool) = setup().await;
        let mut value = input();
        value.title = "  ".into();
        value.description = "  ".into();
        assert!(create(&pool, "ws-1", value).await.is_err());

        let mut value = input();
        value.title = "  ".into();
        let created = create(&pool, "ws-1", value).await.expect("derived title");
        assert_eq!(created.title, "Produza o relatório");

        let mut value = input();
        value.execution_mode = Some("nonsense".into());
        assert!(create(&pool, "ws-1", value).await.is_err());

        let mut value = input();
        value.effort_level = Some("extreme".into());
        assert!(create(&pool, "ws-1", value).await.is_err());
    }
}
