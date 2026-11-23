use super::calendar::scan_calendar_triggers;
use super::folder::{
    FolderTriggerOptions, already_processed, file_fingerprint, release_failed_runs,
};
use super::*;

#[tokio::test]
async fn calendar_trigger_fires_once_per_event() {
    let ctx = crate::test_support::context().await;
    let recipes = ctx.workspace_root.join(".opencode/recipes");
    std::fs::create_dir_all(&recipes).expect("recipes");
    let source = db::calendar::ensure_local_source(&ctx.state.db, &ctx.workspace_id)
        .await
        .expect("source");
    std::fs::write(
        recipes.join("receita-calendario.yaml"),
        format!(
            r#"id: receita-calendario
name: Receita Calendário
enabled: true
triggers:
  - type: CALENDAR_EVENT
    config:
      calendar_id: {source_id}
      match: "Reunião"
      lead_minutes: 0
      lookahead_minutes: 10
flow:
  - type: step
    id: passo-1
    name: Passo
    mode: build
    agent: build
    prompt: "Resuma {{{{trigger.calendar_event.title}}}}"
"#,
            source_id = source.id
        ),
    )
    .expect("recipe");
    store::sync_workspace(&ctx.state.db, &ctx.workspace_id, &ctx.workspace_root)
        .await
        .expect("sync");

    let now = chrono::Utc::now();
    db::calendar::create_event(
        &ctx.state.db,
        &ctx.workspace_id,
        &source.id,
        db::calendar::CreateEventInput {
            source_id: None,
            title: "Reunião de planejamento".into(),
            description: None,
            location: None,
            start_at: now.to_rfc3339(),
            end_at: (now + chrono::Duration::hours(1)).to_rfc3339(),
            all_day: false,
            timezone: Some("UTC".into()),
            status: None,
            color_hex: None,
        },
        &now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        &(now + chrono::Duration::hours(1)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "UTC",
        "CONFIRMED",
    )
    .await
    .expect("event");

    scan_calendar_triggers(&ctx.state)
        .await
        .expect("first scan");
    scan_calendar_triggers(&ctx.state)
        .await
        .expect("second scan");

    let runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM recipe_runs WHERE recipe_id = 'receita-calendario'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .expect("count");
    assert_eq!(runs, 1);
}

#[test]
fn folder_options_default_and_custom_values() {
    let defaults = FolderTriggerOptions::from_config(&serde_json::json!({}));
    assert_eq!(defaults.settle_secs, 2);
    assert!(!defaults.process_empty);
    assert!(defaults.process_existing);
    assert_eq!(defaults.retry_minutes, 1);

    let custom = FolderTriggerOptions::from_config(&serde_json::json!({
        "settle_secs": 500,
        "process_empty": true,
        "process_existing": false,
        "retry_minutes": 0,
        "ignore_patterns": ["backup", ""]
    }));
    assert_eq!(custom.settle_secs, 300);
    assert!(custom.process_empty);
    assert!(!custom.process_existing);
    assert_eq!(custom.retry_minutes, 1);
}

#[tokio::test]
async fn failed_runs_keep_the_file_fingerprint_and_do_not_retry() {
    let ctx = crate::test_support::context().await;
    sqlx::query(
        "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
         VALUES ('retry-recipe', ?, 'Retry', '/tmp/retry.yaml', 'FOLDER_WATCH')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("index");
    sqlx::query(
        "INSERT INTO recipe_runs (id, recipe_id, workspace_id, status, trigger_context_json, output_dir_path)
         VALUES ('retry-run', 'retry-recipe', ?, 'FAILED', '{}', '/tmp/out')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("run");
    db::recipe_files::upsert(
        &ctx.state.db,
        "retry-recipe",
        "input",
        "/tmp/input/relatorio.pdf",
        "100:10",
        "RUNNING",
        Some("retry-run"),
    )
    .await
    .expect("state");

    let runtime = Arc::new(RecipeRuntime::default());
    let path = PathBuf::from("/tmp/input/relatorio.pdf");
    runtime
        .run_checks
        .lock()
        .await
        .insert(path.clone(), "retry-run".into());

    release_failed_runs(&ctx.state, &runtime).await;

    assert!(runtime.retry_at.lock().await.is_empty());
    assert!(runtime.run_checks.lock().await.is_empty());
    let states = db::recipe_files::list_for_trigger(&ctx.state.db, "retry-recipe", "input")
        .await
        .expect("list");
    assert_eq!(
        states
            .get("/tmp/input/relatorio.pdf")
            .expect("state")
            .status,
        "FAILED"
    );
}

#[test]
fn skips_only_the_same_fingerprint_across_restarts() {
    let state = db::recipe_files::RecipeFileState {
        recipe_id: "recipe".into(),
        trigger_path: "input".into(),
        file_path: "/tmp/input/a.pdf".into(),
        fingerprint: "100:10".into(),
        status: "COMPLETED".into(),
        run_id: Some("run-1".into()),
        processed_at: String::new(),
    };
    assert!(already_processed(Some(&state), "100:10"));
    assert!(!already_processed(Some(&state), "101:10"));
    assert!(!already_processed(None, "100:10"));
}

#[test]
fn file_fingerprint_changes_with_mtime_or_size() {
    let temp = tempfile::TempDir::new().expect("temp");
    let file = temp.path().join("a.txt");
    std::fs::write(&file, "a").expect("write");
    let first = file_fingerprint(&std::fs::metadata(&file).expect("meta"));
    std::fs::write(&file, "ab").expect("write");
    let second = file_fingerprint(&std::fs::metadata(&file).expect("meta"));
    assert_ne!(first, second);
}

#[test]
fn folder_options_ignore_patterns_and_temporary_files() {
    let options = FolderTriggerOptions::from_config(&serde_json::json!({
        "ignore_patterns": ["backup", "rascunho"]
    }));
    assert!(options.ignores(".hidden"));
    assert!(options.ignores("a.tmp"));
    assert!(options.ignores("Backup-2026.pdf"));
    assert!(options.ignores("meu-rascunho.pdf"));
    assert!(!options.ignores("relatorio.pdf"));
}

#[tokio::test]
async fn folder_trigger_scans_existing_file_once() {
    let ctx = crate::test_support::context().await;
    let dropzone = ctx.workspace_root.join("input/dropzone");
    std::fs::create_dir_all(&dropzone).expect("dropzone");
    std::fs::write(dropzone.join("brief.txt"), "conteudo").expect("file");
    let yaml = r#"id: folder-test
name: Folder Test
enabled: true
triggers:
  - type: FOLDER_WATCH
    config:
      path: input/dropzone
      process_existing: true
      settle_secs: 0
flow:
  - type: step
    id: s1
    name: Passo
    mode: build
    prompt: "Resuma o arquivo"
"#;
    crate::recipes::store::save_yaml(&ctx.state.db, &ctx.workspace_id, "folder-test", yaml)
        .await
        .expect("save");
    crate::recipes::store::set_enabled(&ctx.state.db, &ctx.workspace_id, "folder-test", true)
        .await
        .expect("enable");
    let runtime = std::sync::Arc::new(RecipeRuntime::default());
    scan_folder_triggers(&ctx.state, &runtime)
        .await
        .expect("scan");
    let runs = crate::recipes::store::list_runs(&ctx.state.db, &ctx.workspace_id, 10)
        .await
        .expect("runs");
    assert!(runs.iter().any(|run| run.recipe_id == "folder-test"));
}

#[tokio::test]
async fn folder_trigger_is_skipped_when_io_source_is_materialized() {
    let ctx = crate::test_support::context().await;
    let yaml = r#"id: folder-materialized
name: Folder Materialized
enabled: true
triggers:
  - type: FOLDER_WATCH
    config:
      path: input/dropzone
flow:
  - type: step
    id: s1
    name: Passo
    prompt: "oi"
"#;
    crate::recipes::store::save_yaml(
        &ctx.state.db,
        &ctx.workspace_id,
        "folder-materialized",
        yaml,
    )
    .await
    .expect("save");
    crate::recipes::store::set_enabled(
        &ctx.state.db,
        &ctx.workspace_id,
        "folder-materialized",
        true,
    )
    .await
    .expect("enable");
    crate::db::io_bus::create_inbound(
        &ctx.state.db,
        &ctx.workspace_id,
        crate::db::io_bus::InboundInput {
            name: "Folder source".into(),
            source_type: "FOLDER_WATCH".into(),
            config: serde_json::json!({
                "recipe_id": "folder-materialized",
                "path": "input/dropzone"
            }),
            target_agent: None,
            target_column: Some("TODO".into()),
            target_kind: Some("RECIPE".into()),
            target_id: Some("folder-materialized".into()),
            auto_execute: false,
            is_enabled: true,
        },
    )
    .await
    .expect("source");
    let runtime = std::sync::Arc::new(RecipeRuntime::default());
    scan_folder_triggers(&ctx.state, &runtime)
        .await
        .expect("scan");
    let runs = crate::recipes::store::list_runs(&ctx.state.db, &ctx.workspace_id, 10)
        .await
        .expect("runs");
    assert!(
        !runs
            .iter()
            .any(|run| run.recipe_id == "folder-materialized")
    );
}
