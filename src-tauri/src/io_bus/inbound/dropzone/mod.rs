use crate::{
    app_state::AppState, db::io_bus::InboundSource, error::AppResult, io_bus::source_config,
};
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

mod process;
mod rsync;
mod watch;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DropzoneConfig {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub processor: Option<String>,
    #[serde(default)]
    pub languages: Option<String>,
    #[serde(default)]
    pub whisper_model: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub rsync_source: Option<String>,
    #[serde(default)]
    pub interval_secs: Option<u64>,
    #[serde(default)]
    pub ssh_key_ref: Option<String>,
    #[serde(default)]
    pub ssh_key_path: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub delete: bool,
    #[serde(default)]
    pub checksum: bool,
    /// Wait before processing so a half-written file is not read.
    #[serde(default)]
    pub settle_secs: Option<u64>,
    /// Include files with zero bytes in the task.
    #[serde(default)]
    pub process_empty: bool,
    /// Process files that already existed when the watcher started.
    #[serde(default)]
    pub process_existing: Option<bool>,
    /// Delay before retrying a failed file.
    #[serde(default)]
    pub retry_minutes: Option<u64>,
    /// Simple substring/extension patterns ignored by the watcher.
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
}

pub async fn run(
    state: Arc<AppState>,
    source: InboundSource,
    cancel: CancellationToken,
) -> AppResult<()> {
    let config: DropzoneConfig = source_config(&source)?;
    let workspace = crate::db::workspaces::get(&state.db, &source.workspace_id).await?;
    let root = PathBuf::from(&workspace.root_path);
    let dropzone = config
        .path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("input").join("dropzone"));
    if !dropzone.is_absolute() {
        // Relative paths are always under the workspace input directory.
        let relative = config.path.as_deref().unwrap_or("input/dropzone");
        let candidate = root.join(relative);
        std::fs::create_dir_all(&candidate)?;
        return watch::watch(state, source, config, root, candidate, cancel).await;
    }
    std::fs::create_dir_all(&dropzone)?;
    watch::watch(state, source, config, root, dropzone, cancel).await
}

#[cfg(test)]
mod tests {
    use super::process::{is_ignored, process_file, process_with_retry};
    use super::*;
    use std::path::Path;

    fn source(workspace_id: &str) -> InboundSource {
        InboundSource {
            id: "source-1".into(),
            workspace_id: workspace_id.into(),
            name: "Pasta de teste".into(),
            source_type: "FOLDER_WATCH".into(),
            config_json: "{}".into(),
            target_agent: None,
            target_column: "TODO".into(),
            target_kind: "TASK".into(),
            target_id: None,
            auto_execute: false,
            is_enabled: true,
            last_triggered_at: None,
            last_status: None,
            last_error: None,
            created_at: "2026-11-13T00:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn processes_a_file_into_a_durable_task() {
        let ctx = crate::test_support::context().await;
        let dir = ctx.workspace_root.join("input/dropzone");
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("relatorio.txt");
        std::fs::write(&file, "conteudo do relatorio").expect("file");
        let source = source(&ctx.workspace_id);
        let config = DropzoneConfig::default();

        process_file(&ctx.state, &source, &config, &ctx.workspace_root, &file)
            .await
            .expect("process");

        let tasks = crate::db::tasks::list(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("tasks");
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].title.contains("Pasta de teste"));
        assert!(tasks[0].description.contains("Origem do gatilho"));
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn retry_wrapper_processes_a_file() {
        let ctx = crate::test_support::context().await;
        let dir = ctx.workspace_root.join("input/dropzone");
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("retry.txt");
        std::fs::write(&file, "conteudo").expect("file");
        let source = source(&ctx.workspace_id);
        let config = DropzoneConfig {
            settle_secs: Some(0),
            ..DropzoneConfig::default()
        };

        process_with_retry(
            ctx.state.clone(),
            source,
            config,
            ctx.workspace_root.clone(),
            file.clone(),
        )
        .await;

        assert!(!file.exists());
        let tasks = crate::db::tasks::list(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("tasks");
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn empty_files_are_skipped_by_default() {
        let ctx = crate::test_support::context().await;
        let dir = ctx.workspace_root.join("input/dropzone");
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("vazio.txt");
        std::fs::write(&file, b"").expect("file");
        let source = source(&ctx.workspace_id);
        let config = DropzoneConfig::default();

        process_file(&ctx.state, &source, &config, &ctx.workspace_root, &file)
            .await
            .expect("process");
        assert!(file.exists());
        let tasks = crate::db::tasks::list(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("tasks");
        assert!(tasks.is_empty());
    }

    #[test]
    fn parses_the_folder_watch_options() {
        let config: DropzoneConfig = serde_json::from_value(serde_json::json!({
            "path": "input/dropzone",
            "settle_secs": 5,
            "process_empty": true,
            "process_existing": false,
            "retry_minutes": 15,
            "ignore_patterns": ["tmp", "backup"]
        }))
        .expect("config");
        assert_eq!(config.settle_secs, Some(5));
        assert!(config.process_empty);
        assert_eq!(config.process_existing, Some(false));
        assert_eq!(config.retry_minutes, Some(15));
        assert_eq!(config.ignore_patterns, vec!["tmp", "backup"]);
    }

    #[test]
    fn ignores_temporary_and_hidden_files() {
        assert!(is_ignored(Path::new("/tmp/.hidden"), &[]));
        assert!(is_ignored(Path::new("/tmp/a.tmp"), &[]));
        assert!(!is_ignored(Path::new("/tmp/report.pdf"), &[]));
        assert!(is_ignored(
            Path::new("/tmp/ignorar.pdf"),
            &["ignorar".into()]
        ));
    }
}
