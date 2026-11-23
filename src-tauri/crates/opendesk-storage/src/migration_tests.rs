//! Migration coverage for the F0 schema (`0002_vault_and_triggers.sql`).

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tempfile::TempDir;

const BASELINE: &str = include_str!("../migrations/0001_v2_baseline.sql");
const FECHAMENTO: &str = include_str!("../migrations/0002_vault_and_triggers.sql");

async fn legacy_pool() -> (TempDir, SqlitePool) {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("legacy.db");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("pool");
    sqlx::raw_sql(BASELINE)
        .execute(&pool)
        .await
        .expect("baseline");
    (temp, pool)
}

#[tokio::test]
async fn upgrade_preserves_io_rows_and_accepts_new_types() {
    let (_temp, pool) = legacy_pool().await;
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws','WS','/tmp/ws')")
        .execute(&pool)
        .await
        .expect("workspace");
    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, title, description) VALUES ('t1','ws','T','D')",
    )
    .execute(&pool)
    .await
    .expect("task");
    sqlx::query(
        "INSERT INTO io_inbound_sources
            (id, workspace_id, name, source_type, config_json, target_column, auto_execute, is_enabled)
         VALUES ('src','ws','Fonte','FOLDER_WATCH','{\"recipe_id\":\"rec-1\"}','TODO',1,1)",
    )
    .execute(&pool)
    .await
    .expect("source");
    sqlx::query(
        "INSERT INTO io_outbound_sinks
            (id, workspace_id, name, sink_type, config_json, is_enabled)
         VALUES ('sink','ws','Sink','TELEGRAM_MSG','{}',1)",
    )
    .execute(&pool)
    .await
    .expect("sink");
    sqlx::query(
        "INSERT INTO task_sink_bindings (id, task_id, sink_id, execution_order)
         VALUES ('bind','t1','sink',1)",
    )
    .execute(&pool)
    .await
    .expect("binding");

    sqlx::raw_sql(FECHAMENTO)
        .execute(&pool)
        .await
        .expect("migration");

    let (kind, target): (String, Option<String>) =
        sqlx::query_as("SELECT target_kind, target_id FROM io_inbound_sources WHERE id='src'")
            .fetch_one(&pool)
            .await
            .expect("source row");
    assert_eq!(kind, "RECIPE");
    assert_eq!(target.as_deref(), Some("rec-1"));

    let bindings: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM task_sink_bindings WHERE id='bind'")
            .fetch_one(&pool)
            .await
            .expect("bindings");
    assert_eq!(bindings, 1);

    sqlx::query(
        "INSERT INTO io_inbound_sources
            (id, workspace_id, name, source_type, config_json, target_column, target_kind, is_enabled)
         VALUES ('poll','ws','Poller','HTTP_POLLER','{}','TODO','TASK',1)",
    )
    .execute(&pool)
    .await
    .expect("poller");
    sqlx::query(
        "INSERT INTO io_outbound_sinks
            (id, workspace_id, name, sink_type, config_json, is_enabled)
         VALUES ('hook','ws','Hook','HTTP_WEBHOOK','{}',1)",
    )
    .execute(&pool)
    .await
    .expect("hook");
    sqlx::query(
        "INSERT INTO vault_entries
            (id, kind, label, secret_ciphertext, secret_nonce)
         VALUES ('v1','api_key','Stripe',X'00',X'000000000000000000000000')",
    )
    .execute(&pool)
    .await
    .expect("vault");
}

#[tokio::test]
async fn fresh_database_has_vault_and_poller_tables() {
    let temp = TempDir::new().expect("temp");
    let db = crate::pool::init(&temp.path().join("fresh.db"))
        .await
        .expect("db");
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master
         WHERE type='table' AND name IN
            ('vault_entries','vault_acls','vault_audit','io_poller_state','io_poller_seen')
         ORDER BY name",
    )
    .fetch_all(&db)
    .await
    .expect("tables");
    assert_eq!(
        tables,
        vec![
            "io_poller_seen",
            "io_poller_state",
            "vault_acls",
            "vault_audit",
            "vault_entries"
        ]
    );
}
