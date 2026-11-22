use super::*;
use sqlx::sqlite::SqliteConnectOptions;
use tempfile::TempDir;

async fn v1_db(path: &Path, yaml_path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent");
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("db");
    sqlx::query(
        "CREATE TABLE _sqlx_migrations (
            version INTEGER PRIMARY KEY, description TEXT NOT NULL,
            installed_on TEXT NOT NULL DEFAULT '', success INTEGER NOT NULL,
            checksum BLOB NOT NULL, execution_time INTEGER NOT NULL
         )",
    )
    .execute(&pool)
    .await
    .expect("migrations");
    sqlx::query(
        "INSERT INTO _sqlx_migrations
            (version, description, success, checksum, execution_time)
         VALUES (59, 'skill shadow', 1, x'00', 0)",
    )
    .execute(&pool)
    .await
    .expect("version");
    sqlx::query(
        "CREATE TABLE workspaces (id TEXT PRIMARY KEY, name TEXT NOT NULL, root_path TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .expect("workspaces");
    sqlx::query(
        "INSERT INTO workspaces (id, name, root_path)
         VALUES ('ws-1', 'Workspace', '/tmp/ws-1')",
    )
    .execute(&pool)
    .await
    .expect("workspace");
    sqlx::query(
        "CREATE TABLE recipes_index (
            id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, name TEXT NOT NULL,
            file_path TEXT, trigger_type TEXT, is_enabled INTEGER NOT NULL DEFAULT 1
         )",
    )
    .execute(&pool)
    .await
    .expect("recipes");
    sqlx::query(
        "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type, is_enabled)
         VALUES ('recipe-1', 'ws-1', 'Receita', ?, 'MANUAL', 1)",
    )
    .bind(yaml_path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .expect("recipe");
    sqlx::query("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT '')")
        .execute(&pool)
        .await
        .expect("settings");
    sqlx::query("INSERT INTO app_settings (key, value_json) VALUES ('theme', '\"dark\"')")
        .execute(&pool)
        .await
        .expect("setting");
    pool.close().await;
}

#[tokio::test]
async fn exports_workspaces_recipes_and_settings() {
    let temp = TempDir::new().expect("temp");
    let db = temp.path().join("v1.db");
    let yaml_path = temp.path().join("recipe.yaml");
    std::fs::write(&yaml_path, "id: recipe-1\nname: Receita\n").expect("yaml");
    v1_db(&db, &yaml_path).await;

    let out = temp.path().join("out");
    let summary = export_v1(&db, &out, false).await.expect("export");
    assert_eq!(summary.schema_version, 59);
    assert_eq!(summary.workspaces, 1);
    let bundle = std::fs::read_to_string(out.join("export.json")).expect("json");
    let value: serde_json::Value = serde_json::from_str(&bundle).expect("parse");
    assert_eq!(value["schema_version"], 59);
    assert_eq!(value["workspaces"][0]["name"], "Workspace");
    assert_eq!(value["recipes"][0]["id"], "recipe-1");
    assert_eq!(value["settings"][0]["key"], "theme");
    assert_eq!(
        std::fs::read_to_string(out.join("recipes/recipe-1.yaml")).expect("recipe file"),
        "id: recipe-1\nname: Receita\n"
    );
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("manifest.json")).expect("manifest"),
    )
    .expect("manifest json");
    assert_eq!(manifest["counts"]["workspaces"], 1);
    assert!(manifest["files"]["export.json"].is_string());
    assert!(!bundle.contains("secret"), "{bundle}");
}

#[tokio::test]
async fn exports_yaml_and_refuses_v2_or_dirty_output() {
    let temp = TempDir::new().expect("temp");
    let db = temp.path().join("v1.db");
    let yaml_path = temp.path().join("recipe.yaml");
    std::fs::write(&yaml_path, "id: recipe-1\n").expect("yaml");
    v1_db(&db, &yaml_path).await;

    let out = temp.path().join("yaml-out");
    export_v1(&db, &out, true).await.expect("yaml");
    let text = std::fs::read_to_string(out.join("export.yaml")).expect("yaml file");
    assert!(text.contains("Workspace"));

    let dirty = temp.path().join("dirty");
    std::fs::create_dir_all(&dirty).expect("dirty");
    std::fs::write(dirty.join("keep.txt"), "x").expect("keep");
    assert!(export_v1(&db, &dirty, false).await.is_err());

    let v2 = temp.path().join("v2.db");
    let _pool = crate::pool::init(&v2).await.expect("v2");
    let error = export_v1(&v2, &temp.path().join("v2-out"), false)
        .await
        .expect_err("v2 refused");
    assert!(format!("{error:#}").contains("V2") || format!("{error:#}").contains("V1"));
}

#[tokio::test]
async fn legacy_version_detects_v1_only() {
    let temp = TempDir::new().expect("temp");
    let v1 = temp.path().join("v1.db");
    let yaml = temp.path().join("r.yaml");
    std::fs::write(&yaml, "id: r\n").expect("yaml");
    v1_db(&v1, &yaml).await;
    assert_eq!(legacy_version(&v1).await.expect("v1"), Some(59));
    assert_eq!(
        legacy_version(&temp.path().join("missing.db"))
            .await
            .expect("missing"),
        None
    );
    let v2 = temp.path().join("v2.db");
    let _pool = crate::pool::init(&v2).await.expect("v2");
    assert_eq!(legacy_version(&v2).await.expect("v2"), None);
}

#[tokio::test]
async fn migrate_v1_backs_up_and_allows_fresh_v2() {
    let temp = TempDir::new().expect("temp");
    let paths = crate::paths::test_paths(temp.path());
    std::fs::create_dir_all(&paths.data_dir).expect("data");
    let yaml = temp.path().join("r.yaml");
    std::fs::write(&yaml, "id: r\n").expect("yaml");
    v1_db(&paths.db_path, &yaml).await;

    let out = paths.data_dir.join("v1-export");
    let summary = migrate_v1(&paths, &out).await.expect("migrate");
    assert_eq!(summary.schema_version, 59);
    assert!(!paths.db_path.exists(), "V1 database must be moved aside");
    assert!(out.join("export.json").exists());
    let backup = std::fs::read_dir(&paths.data_dir)
        .expect("data")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .find(|name| name.starts_with("opendesk.db.v1-"))
        .expect("V1 backup");
    assert!(backup.contains("opendesk.db.v1-"));

    let pool = crate::pool::init(&paths.db_path).await.expect("fresh v2");
    let version: String =
        sqlx::query_scalar("SELECT value FROM v2_meta WHERE key = 'schema_version'")
            .fetch_one(&pool)
            .await
            .expect("v2_meta");
    assert_eq!(version, "2");
    pool.close().await;
}
