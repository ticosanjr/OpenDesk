#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppResult;
    use crate::runtime::{RuntimeCapabilities, RuntimeEventStream, RuntimeFileDiff};
    use async_trait::async_trait;
    use futures_util::stream;
    use opendesk_storage::events::EventBus;
    use opendesk_storage::secrets::SecretStore;
    use std::sync::Mutex;
    use tokio_util::sync::CancellationToken;

    enum Start {
        Ok,
        Fail,
    }

    struct FakeAdapter {
        start: Start,
        events: Mutex<Option<Vec<AppResult<RuntimeEvent>>>>,
        interrupted: Mutex<bool>,
    }

    #[async_trait]
    impl AgentRuntimeAdapter for FakeAdapter {
        fn id(&self) -> &str {
            "fake"
        }

        fn capabilities(&self) -> RuntimeCapabilities {
            RuntimeCapabilities::default()
        }

        async fn start(&self, _request: RuntimeRequest) -> AppResult<String> {
            match self.start {
                Start::Ok => Ok("session-1".into()),
                Start::Fail => Err(crate::error::AppError::BadRequest("no runtime".into())),
            }
        }

        async fn prompt(&self, _session_id: &str, _prompt: &str) -> AppResult<()> {
            Ok(())
        }

        async fn events(&self, _session_id: &str) -> AppResult<RuntimeEventStream> {
            let items = self
                .events
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .take()
                .unwrap_or_default();
            Ok(Box::pin(stream::iter(items)))
        }

        async fn interrupt(&self, _session_id: &str) -> AppResult<()> {
            *self
                .interrupted
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = true;
            Ok(())
        }
    }

    fn adapter(events: Vec<RuntimeEvent>) -> Arc<FakeAdapter> {
        Arc::new(FakeAdapter {
            start: Start::Ok,
            events: Mutex::new(Some(events.into_iter().map(Ok).collect())),
            interrupted: Mutex::new(false),
        })
    }

    #[tokio::test]
    async fn executes_a_runtime_stream_and_maps_every_event() {
        let ctx = crate::test_support::context().await;
        let secrets = SecretStore::new(&ctx.paths);
        let bus = EventBus::new(32);
        let log = ctx.temp.path().join("run.log");
        let cancel = CancellationToken::new();
        let fake = adapter(vec![
            RuntimeEvent::TextDelta {
                delta: "olá ".into(),
            },
            RuntimeEvent::TextDelta {
                delta: "mundo".into(),
            },
            RuntimeEvent::Usage {
                input_tokens: 10,
                output_tokens: 4,
                cache_read: 1,
                cache_write: 0,
                cost_usd: 0.01,
            },
            RuntimeEvent::Diff {
                files: vec![RuntimeFileDiff {
                    path: "src/a.rs".into(),
                    status: "modified".into(),
                    patch: "+x".into(),
                    additions: 1,
                    deletions: 0,
                }],
            },
            RuntimeEvent::Artifact {
                path: "output/a.md".into(),
                action: "create".into(),
            },
            RuntimeEvent::PermissionAsked {
                request_id: "perm-1".into(),
                action: "shell".into(),
                resources: vec!["ls".into()],
            },
            RuntimeEvent::RunFinished {
                status: "success".into(),
                summary: Some("concluído".into()),
            },
        ]);
        let outcome = execute(
            fake.clone(),
            RunRequest {
                pool: &ctx.db,
                secrets: &secrets,
                workspace_id: &ctx.workspace_id,
                run_id: "run-1",
                task_id: None,
                session_id: None,
                workspace_root: &ctx.workspace_root,
                agent: "build",
                model: "opencode-go/glm-5.3",
                phase: "BUILD",
                title: "Teste",
                prompt: "faça algo",
                log_path: &log,
                cancel,
                notifier: None,
            },
            &bus,
        )
        .await;

        assert_eq!(outcome.status, RunStatus::Success);
        assert_eq!(outcome.session_id, "session-1");
        assert_eq!(outcome.summary.as_deref(), Some("concluído"));
        assert_eq!(outcome.usage.tokens_input, 10);
        assert_eq!(outcome.usage.tokens_output, 4);
        assert_eq!(outcome.diffs.len(), 1);
        assert_eq!(outcome.diffs[0].file, "src/a.rs");
        assert_eq!(outcome.writes.len(), 1);
        assert_eq!(outcome.writes[0].path, "output/a.md");
        assert_eq!(outcome.permissions.len(), 1);
        assert_eq!(outcome.permissions[0].request_id, "perm-1");
        assert!(outcome.error.is_none());
        assert!(outcome.log_tail.unwrap_or_default().contains("opendesk"));
    }

    #[tokio::test]
    async fn reports_an_adapter_that_cannot_start() {
        let ctx = crate::test_support::context().await;
        let secrets = SecretStore::new(&ctx.paths);
        let bus = EventBus::new(8);
        let log = ctx.temp.path().join("run.log");
        let fake = Arc::new(FakeAdapter {
            start: Start::Fail,
            events: Mutex::new(None),
            interrupted: Mutex::new(false),
        });
        let outcome = execute(
            fake,
            RunRequest {
                pool: &ctx.db,
                secrets: &secrets,
                workspace_id: &ctx.workspace_id,
                run_id: "run-2",
                task_id: Some("task-1"),
                session_id: None,
                workspace_root: &ctx.workspace_root,
                agent: "build",
                model: "opencode-go/glm-5.3",
                phase: "BUILD",
                title: "Falha",
                prompt: "faça algo",
                log_path: &log,
                cancel: CancellationToken::new(),
                notifier: None,
            },
            &bus,
        )
        .await;
        assert_eq!(outcome.status, RunStatus::Failed);
        assert!(outcome.error.unwrap_or_default().contains("no runtime"));
    }
}
