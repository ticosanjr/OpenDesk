use crate::{app_state::AppState, paths::AppPaths};
use std::{path::PathBuf, sync::Arc};
use tempfile::TempDir;

pub struct TestContext {
    pub temp: TempDir,
    pub state: Arc<AppState>,
    pub workspace_id: String,
    pub workspace_root: PathBuf,
}

/// Builds an isolated [`AppPaths`] rooted at `root`, creating the directories.
pub fn paths_in(root: &std::path::Path) -> AppPaths {
    let paths = AppPaths {
        data_dir: root.join("data"),
        config_dir: root.join("config"),
        cache_dir: root.join("cache"),
        log_dir: root.join("logs"),
        db_path: root.join("data").join("opendesk.db"),
        backups_dir: root.join("backups"),
        artifacts_dir: root.join("artifacts"),
        run_logs_dir: root.join("runs"),
        daemon_lock_path: root.join("daemon.json"),
        api_lock_path: root.join("api.json"),
        allow_temp_workspaces: true,
    };
    for dir in [
        &paths.data_dir,
        &paths.config_dir,
        &paths.cache_dir,
        &paths.log_dir,
        &paths.backups_dir,
        &paths.artifacts_dir,
        &paths.run_logs_dir,
    ] {
        std::fs::create_dir_all(dir).expect("dir");
    }
    paths
}

pub async fn context() -> TestContext {
    let temp = TempDir::new().expect("temp");
    let root = temp.path();
    let paths = paths_in(root);
    let workspace_root = root.join("workspace");
    std::fs::create_dir_all(&workspace_root).expect("workspace root");
    let state = Arc::new(AppState::initialize(paths).await.expect("state"));
    state.attach_runtime_ports();
    let workspace_id = "ws-test".to_string();
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES (?, 'Test', ?)")
        .bind(&workspace_id)
        .bind(workspace_root.to_string_lossy().to_string())
        .execute(&state.db)
        .await
        .expect("workspace");
    crate::db::settings::set_active_workspace(&state.db, &workspace_id)
        .await
        .expect("active workspace");
    TestContext {
        temp,
        state,
        workspace_id,
        workspace_root,
    }
}
