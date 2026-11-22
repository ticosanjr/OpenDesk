use ::opendesk_storage::{
    init,
    paths::{AppPaths, test_paths},
    repos::settings,
    write_buffer::WriteBuffer,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

pub struct TestContext {
    pub temp: TempDir,
    pub db: SqlitePool,
    pub paths: AppPaths,
    pub workspace_id: String,
    pub workspace_root: std::path::PathBuf,
    pub write_buffer: Arc<WriteBuffer>,
    pub events: ::opendesk_storage::events::EventBus,
}

pub async fn context() -> TestContext {
    let temp = TempDir::new().expect("temp");
    let paths = test_paths(temp.path());
    let db = init(&paths.db_path).await.expect("db");
    let workspace_root = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace_root).expect("workspace");
    let workspace_id = "ws-test".to_string();
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES (?, 'Test', ?)")
        .bind(&workspace_id)
        .bind(workspace_root.to_string_lossy().to_string())
        .execute(&db)
        .await
        .expect("workspace");
    settings::set_active_workspace(&db, &workspace_id)
        .await
        .expect("active");
    let write_buffer = WriteBuffer::start(db.clone(), Duration::from_millis(20), 64);
    TestContext {
        temp,
        db,
        paths,
        workspace_id,
        workspace_root,
        write_buffer,
        events: ::opendesk_storage::events::EventBus::new(64),
    }
}
