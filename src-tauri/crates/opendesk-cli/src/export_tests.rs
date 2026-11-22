#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn v1_db(path: &Path, yaml_path: &Path) {
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
        sqlx::query("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("settings");
        sqlx::query("INSERT INTO app_settings (key, value) VALUES ('theme', 'dark')")
            .execute(&pool)
            .await
            .expect("setting");
        pool.close().await;
    }

    fn options(db: &Path, out: &Path, format: Format) -> Options {
        Options {
            db: db.to_path_buf(),
            out: out.to_path_buf(),
            format,
        }
    }

    #[tokio::test]
    async fn exports_workspaces_recipes_and_settings() {
        let temp = TempDir::new().expect("temp");
        let db = temp.path().join("v1.db");
        let yaml_path = temp.path().join("recipe.yaml");
        fs::write(&yaml_path, "id: recipe-1\nname: Receita\n").expect("yaml");
        v1_db(&db, &yaml_path).await;

        let out = temp.path().join("out");
        export(options(&db, &out, Format::Json))
            .await
            .expect("export");
        let bundle = fs::read_to_string(out.join("export.json")).expect("json");
        let value: serde_json::Value = serde_json::from_str(&bundle).expect("parse");
        assert_eq!(value["schema_version"], 59);
        assert_eq!(value["workspaces"][0]["name"], "Workspace");
        assert_eq!(value["recipes"][0]["id"], "recipe-1");
        assert_eq!(value["settings"][0]["key"], "theme");
        assert_eq!(
            fs::read_to_string(out.join("recipes/recipe-1.yaml")).expect("recipe file"),
            "id: recipe-1\nname: Receita\n"
        );
        let manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(out.join("manifest.json")).expect("manifest"))
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
        fs::write(&yaml_path, "id: recipe-1\n").expect("yaml");
        v1_db(&db, &yaml_path).await;

        let out = temp.path().join("yaml-out");
        export(options(&db, &out, Format::Yaml))
            .await
            .expect("yaml");
        let text = fs::read_to_string(out.join("export.yaml")).expect("yaml file");
        assert!(text.contains("Workspace"));

        let dirty = temp.path().join("dirty");
        fs::create_dir_all(&dirty).expect("dirty");
        fs::write(dirty.join("keep.txt"), "x").expect("keep");
        assert!(export(options(&db, &dirty, Format::Json)).await.is_err());

        let v2 = temp.path().join("v2.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&v2)
                    .create_if_missing(true),
            )
            .await
            .expect("v2");
        sqlx::query("CREATE TABLE _sqlx_migrations (version INTEGER PRIMARY KEY, description TEXT NOT NULL, installed_on TEXT NOT NULL DEFAULT '', success INTEGER NOT NULL, checksum BLOB NOT NULL, execution_time INTEGER NOT NULL)")
            .execute(&pool)
            .await
            .expect("migrations");
        sqlx::query("INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (1, 'baseline', 1, x'00', 0)")
            .execute(&pool)
            .await
            .expect("version");
        sqlx::query("CREATE TABLE v2_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("v2_meta");
        sqlx::query("INSERT INTO v2_meta (key, value) VALUES ('schema_version', '2')")
            .execute(&pool)
            .await
            .expect("marker");
        pool.close().await;
        let error = export(options(&v2, &temp.path().join("v2-out"), Format::Json))
            .await
            .expect_err("v2 refused");
        assert!(error.contains("V2"), "{error}");
    }
}
