use crate::{
    events::EventBus,
    paths::{AppPaths, test_paths},
    pool,
    repos::settings,
    write_buffer::WriteBuffer,
};
use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tempfile::TempDir;

pub struct TestContext {
    pub temp: TempDir,
    pub db: SqlitePool,
    pub paths: AppPaths,
    pub workspace_id: String,
    pub workspace_root: PathBuf,
    pub write_buffer: Arc<WriteBuffer>,
    pub events: EventBus,
}

pub async fn context() -> TestContext {
    let temp = TempDir::new().expect("temp");
    let paths = test_paths(temp.path());
    let db = pool::init(&paths.db_path).await.expect("db");
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
        events: EventBus::new(64),
    }
}
