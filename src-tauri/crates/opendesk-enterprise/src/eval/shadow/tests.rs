#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::cases::CreateSkillCase;

    struct FakeExecutor;

    #[async_trait]
    impl ShadowExecutor for FakeExecutor {
        async fn execute(
            &self,
            _skill: &skills::SkillFile,
            case: &SkillEvalCase,
        ) -> AppResult<ShadowOutput> {
            Ok(ShadowOutput {
                output: if case.name == "bad" {
                    "sem contrato".into()
                } else {
                    r#"{"status":"ok"}"#.into()
                },
                tokens: 10,
                cost_usd: 0.01,
            })
        }
    }

    async fn fixture() -> crate::test_support::TestContext {
        let ctx = crate::test_support::context().await;
        let dir = ctx.workspace_root.join(".opencode/skills/demo");
        std::fs::create_dir_all(&dir).expect("skill dir");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: Demo\ndescription: Demo\n---\nUse a skill.\n",
        )
        .expect("skill");
        skill_cases::create(
            &ctx.db,
            &ctx.workspace_id,
            "demo",
            CreateSkillCase {
                name: "ok".into(),
                prompt: "faça".into(),
                expected_contains: Some("ok".into()),
                expected_schema: Some(serde_json::json!({
                    "type": "object",
                    "required": ["status"]
                })),
            },
        )
        .await
        .expect("case");
        skill_cases::create(
            &ctx.db,
            &ctx.workspace_id,
            "demo",
            CreateSkillCase {
                name: "bad".into(),
                prompt: "faça".into(),
                expected_contains: Some("ok".into()),
                expected_schema: None,
            },
        )
        .await
        .expect("case");
        ctx
    }

    #[tokio::test]
    async fn rollback_restores_the_previous_skill_file() {
        let ctx = crate::test_support::context().await;
        let paths = StudioPaths::new(Some(ctx.workspace_root.clone()));
        skills::write(
            &paths,
            "workspace",
            "rollback-demo",
            skills::SkillWriteInput {
                name: Some("Rollback".into()),
                description: Some("v1".into()),
                body: "primeira versão".into(),
                files: Vec::new(),
                expected_sha256: None,
            },
            &ctx.paths.backups_dir,
        )
        .expect("first");
        skills::write(
            &paths,
            "workspace",
            "rollback-demo",
            skills::SkillWriteInput {
                name: Some("Rollback".into()),
                description: Some("v2".into()),
                body: "segunda versão".into(),
                files: Vec::new(),
                expected_sha256: None,
            },
            &ctx.paths.backups_dir,
        )
        .expect("second");
        let restored = rollback(&ctx.workspace_root, &ctx.paths.backups_dir, "rollback-demo")
            .await
            .expect("rollback");
        assert!(restored.body.contains("primeira versão"));
    }

    #[tokio::test]
    async fn replays_cases_and_blocks_regression() {
        let ctx = fixture().await;
        let sentinel = ctx.workspace_root.join("sentinel.txt");
        std::fs::write(&sentinel, "keep").expect("sentinel");
        let first = run(
            &ctx.db,
            &ctx.workspace_id,
            &ctx.workspace_root,
            "demo",
            None,
            ShadowThresholds {
                min_success_rate: 0.5,
                max_regression: 0.1,
            },
            &FakeExecutor,
        )
        .await
        .expect("first");
        assert_eq!(first.status, "PASS");
        assert_eq!(first.total, 2);
        assert_eq!(first.passed, 1);
        let second = run(
            &ctx.db,
            &ctx.workspace_id,
            &ctx.workspace_root,
            "demo",
            None,
            ShadowThresholds {
                min_success_rate: 0.9,
                max_regression: 0.1,
            },
            &FakeExecutor,
        )
        .await
        .expect("second");
        assert_eq!(second.status, "BLOCKED");
        assert!(
            promote(&ctx.db, &ctx.workspace_id, &second.id, true, None)
                .await
                .is_err()
        );
        assert!(
            promote(&ctx.db, &ctx.workspace_id, &first.id, false, None)
                .await
                .is_err()
        );
        assert!(
            !results(&ctx.db, &first.id)
                .await
                .expect("results")
                .is_empty()
        );
    }
}
