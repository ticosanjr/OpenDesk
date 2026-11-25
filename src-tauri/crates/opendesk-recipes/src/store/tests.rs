use super::*;
use tempfile::TempDir;

#[tokio::test]
async fn indexes_saves_and_runs_recipe() {
    let temp = TempDir::new().expect("temp");
    let pool = opendesk_storage::init(&temp.path().join("r.db"))
        .await
        .expect("db");
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
        .bind(temp.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .expect("ws");
    let yaml = "id: teste\nname: Teste\ntrigger:\n  type: MANUAL\n  config: {}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n";
    let index = save_yaml(&pool, "ws-1", "teste", yaml).await.expect("save");
    assert_eq!(index.id, "teste");
    let recipe = crate::parser::parse(yaml).expect("parse");
    let run = create_run(
        &pool,
        &recipe,
        "ws-1",
        None,
        &serde_json::json!({}),
        "/tmp/out",
        false,
    )
    .await
    .expect("run");
    assert_eq!(run.status, "RUNNING_STEP");
    assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 1);

    let hitl = create_hitl(
        &pool,
        "8F2A",
        &run,
        "h1",
        "BINARY_GATE",
        "Receita teste",
        "Aprovar o checkpoint?",
        &serde_json::json!({ "recipe_name": "Receita teste" }),
        &[],
        "PAUSE_AND_ALERT",
        "2999-01-01T00:00:00Z",
    )
    .await
    .expect("hitl");
    assert_eq!(hitl.status, "PENDING");
    assert_eq!(hitl.title.as_deref(), Some("Receita teste"));
    assert_eq!(
        hitl.question_markdown.as_deref(),
        Some("Aprovar o checkpoint?")
    );
    assert!(
        hitl.context_json.contains("Receita teste"),
        "context: {}",
        hitl.context_json
    );
    dispatch_channel(&pool, "8F2A", "SIGNAL", "ts-1")
        .await
        .expect("dispatch");
    assert_eq!(pending_hitl(&pool, "ws-1").await.expect("pending").len(), 1);
    let resolved = resolve_hitl(&pool, "8F2A", "SIGNAL", Some("+1"), "APPROVED", None)
        .await
        .expect("resolve")
        .expect("resolved");
    assert_eq!(resolved.status, "RESOLVED");
    assert!(
        resolve_hitl(&pool, "8F2A", "EMAIL", None, "REJECTED", None)
            .await
            .expect("second")
            .is_none()
    );
}

#[tokio::test]
async fn same_recipe_id_is_scoped_per_workspace() {
    let temp = TempDir::new().expect("temp");
    let pool = opendesk_storage::init(&temp.path().join("r.db"))
        .await
        .expect("db");
    for workspace in ["ws-1", "ws-2"] {
        let root = temp.path().join(workspace);
        std::fs::create_dir_all(root.join(".opencode/recipes")).expect("dir");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES (?, ?, ?)")
            .bind(workspace)
            .bind(workspace)
            .bind(root.to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("ws");
        std::fs::write(
                root.join(".opencode/recipes/shared.yaml"),
                "id: shared\nname: Compartilhada\ntrigger:\n  type: MANUAL\n  config: {}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n",
            )
            .expect("yaml");
        sync_workspace(&pool, workspace, &root).await.expect("sync");
    }

    let first = list(&pool, "ws-1").await.expect("list 1");
    let second = list(&pool, "ws-2").await.expect("list 2");
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].file_path, second[0].file_path);
    assert!(first[0].file_path.contains("ws-1"));
    assert!(
        get(&pool, "ws-1", "shared").await.is_ok(),
        "each workspace keeps its own shared id"
    );
}

#[tokio::test]
async fn concurrent_reconciliation_keeps_all_recipes() {
    let temp = TempDir::new().expect("temp");
    let pool = opendesk_storage::init(&temp.path().join("r.db"))
        .await
        .expect("db");
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
        .bind(temp.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .expect("ws");
    let dir = temp.path().join(".opencode/recipes");
    std::fs::create_dir_all(&dir).expect("dir");
    for id in ["um", "dois", "tres"] {
        std::fs::write(
                dir.join(format!("{id}.yaml")),
                format!(
                    "id: {id}\nname: {id}\ntrigger:\n  type: MANUAL\n  config: {{}}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n"
                ),
            )
            .expect("yaml");
    }
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        let root = temp.path().to_path_buf();
        tasks.push(tokio::spawn(async move {
            sync_workspace(&pool, "ws-1", &root).await
        }));
    }
    for task in tasks {
        task.await.expect("join").expect("sync");
    }
    assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 3);
}

#[tokio::test]
async fn new_recipes_start_disabled_and_edits_preserve_the_flag() {
    let temp = TempDir::new().expect("temp");
    let pool = opendesk_storage::init(&temp.path().join("r.db"))
        .await
        .expect("db");
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
        .bind(temp.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .expect("ws");
    let yaml = "id: nova\nname: Nova\nenabled: true\ntriggers: []\nflow:\n  - type: step\n    id: s1\n    name: Um\n    prompt: oi\n";
    let index = save_yaml(&pool, "ws-1", "nova", yaml).await.expect("save");
    assert!(!index.is_enabled, "new recipes must start disabled");
    let stored = yaml_for(&pool, "ws-1", "nova").await.expect("yaml");
    assert!(stored.contains("enabled: false"), "{stored}");

    let enabled = set_enabled(&pool, "ws-1", "nova", true)
        .await
        .expect("enable");
    assert!(enabled.is_enabled);

    let edited = yaml.replace("Nova", "Nova editada");
    let saved = save_yaml(&pool, "ws-1", "nova", &edited)
        .await
        .expect("edit");
    assert!(saved.is_enabled, "editing must preserve the enabled flag");
}

#[tokio::test]
async fn persists_nodes_and_cancel_flag() {
    let ctx = crate::test_support::context().await;
    let yaml = "id: durable\nname: Durable\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo 1\n    prompt: oi\n  - type: hitl\n    id: h1\n    interaction_type: BINARY_GATE\n    prompt_template: ok?\n  - type: sink\n    sink_type: DOC_COMPILER\n    config: {}\n";
    let recipe = crate::parser::parse(yaml).expect("recipe");
    save_yaml(&ctx.db, &ctx.workspace_id, "durable", yaml)
        .await
        .expect("index");
    let run = create_run(
        &ctx.db,
        &recipe,
        &ctx.workspace_id,
        None,
        &serde_json::json!({}),
        "/tmp/out",
        false,
    )
    .await
    .expect("run");
    sync_run_nodes(&ctx.db, &run.id, &recipe.flow)
        .await
        .expect("nodes");
    let nodes = list_run_nodes(&ctx.db, &run.id).await.expect("list");
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].kind, "step");
    assert_eq!(nodes[1].kind, "hitl");
    assert_eq!(nodes[2].kind, "sink");

    mark_node_running(&ctx.db, &run.id, 0)
        .await
        .expect("running");
    mark_node_done(
        &ctx.db,
        &run.id,
        0,
        Some(&serde_json::json!({"ok": true})),
        Some("s"),
    )
    .await
    .expect("done");
    let nodes = list_run_nodes(&ctx.db, &run.id).await.expect("list");
    assert_eq!(nodes[0].status, "DONE");
    assert_eq!(nodes[0].attempts, 1);

    assert_eq!(resumable_runs(&ctx.db).await.expect("resume").len(), 1);
    request_cancel(&ctx.db, &run.id).await.expect("cancel");
    assert!(is_cancel_requested(&ctx.db, &run.id).await.expect("flag"));
    assert!(resumable_runs(&ctx.db).await.expect("resume").is_empty());

    reset_for_retry(&ctx.db, &run.id).await.expect("retry");
    assert!(!is_cancel_requested(&ctx.db, &run.id).await.expect("flag"));
    let nodes = list_run_nodes(&ctx.db, &run.id).await.expect("list");
    assert_eq!(nodes[0].status, "DONE");
    assert_eq!(nodes[1].status, "PENDING");
}

#[test]
fn toggles_enabled_preserving_comment_and_final_newline() {
    let yaml = "id: qa\nenabled: true # liga/desliga\nflow: []\n";
    let output = set_yaml_enabled(yaml, false);
    assert!(output.contains("enabled: false # liga/desliga"));
    assert!(output.ends_with('\n'));
    assert!(output.contains("flow: []"));
}

#[test]
fn adds_enabled_when_missing_and_keeps_missing_final_newline() {
    let yaml = "id: qa\nflow: []";
    let output = set_yaml_enabled(yaml, true);
    assert!(output.starts_with("enabled: true\n"));
    assert!(!output.ends_with('\n'));
}

#[tokio::test]
async fn versions_recipes_and_rolls_back() {
    let ctx = crate::test_support::context().await;
    let v1 = "id: versionada\nname: Versionada\nflow:\n  - type: step\n    id: s1\n    name: S1\n    prompt: v1\n";
    let v2 = "id: versionada\nname: Versionada\nflow:\n  - type: step\n    id: s1\n    name: S1\n    prompt: v2\n";
    save_yaml(&ctx.db, &ctx.workspace_id, "versionada", v1)
        .await
        .expect("v1");
    save_yaml(&ctx.db, &ctx.workspace_id, "versionada", v1)
        .await
        .expect("same");
    save_yaml(&ctx.db, &ctx.workspace_id, "versionada", v1)
        .await
        .expect("same again");
    let after_same = list_versions(&ctx.db, &ctx.workspace_id, "versionada")
        .await
        .expect("list");
    assert_eq!(after_same.len(), 2, "identical content must not duplicate");

    save_yaml(&ctx.db, &ctx.workspace_id, "versionada", v2)
        .await
        .expect("v2");
    let versions = list_versions(&ctx.db, &ctx.workspace_id, "versionada")
        .await
        .expect("list");
    assert_eq!(versions.len(), 3);
    let first = versions.last().expect("first");
    let diff = diff_version(&ctx.db, &ctx.workspace_id, "versionada", &first.id, None)
        .await
        .expect("diff");
    assert!(diff.contains("prompt: v2"), "{diff}");
    assert!(
        diff.contains("prompt: v1") || diff.contains("-    prompt: v1"),
        "{diff}"
    );

    rollback_version(&ctx.db, &ctx.workspace_id, "versionada", &first.id)
        .await
        .expect("rollback");
    let current = yaml_for(&ctx.db, &ctx.workspace_id, "versionada")
        .await
        .expect("yaml");
    assert!(current.contains("prompt: v1"), "{current}");
    assert_eq!(
        list_versions(&ctx.db, &ctx.workspace_id, "versionada")
            .await
            .expect("list")
            .len(),
        4
    );
}

#[tokio::test]
async fn seeds_path_addressed_nodes_for_dag_groups() {
    let ctx = crate::test_support::context().await;
    let yaml = r#"
id: dag-paths
name: DAG paths
flow:
  - type: branch
    id: decidir
    when: "true"
    then:
      - type: step
        id: t1
        name: T1
        prompt: a
    else:
      - type: step
        id: e1
        name: E1
        prompt: b
  - type: parallel
    id: lote
    branches:
      - - type: step
          id: p1
          name: P1
          prompt: a
      - - type: step
          id: p2
          name: P2
          prompt: b
  - type: map
    id: arquivos
    items: "[]"
    flow:
      - type: step
        id: analisar
        name: Analisar
        prompt: c
"#;
    save_yaml(&ctx.db, &ctx.workspace_id, "dag-paths", yaml)
        .await
        .expect("save");
    let recipe = crate::parser::parse(yaml).expect("recipe");
    let run = create_run(
        &ctx.db,
        &recipe,
        &ctx.workspace_id,
        None,
        &serde_json::json!({}),
        "/tmp/out",
        false,
    )
    .await
    .expect("run");
    sync_run_nodes(&ctx.db, &run.id, &recipe.flow)
        .await
        .expect("sync");
    let nodes = list_run_nodes(&ctx.db, &run.id).await.expect("list");
    let paths: Vec<&str> = nodes
        .iter()
        .filter_map(|node| node.node_path.as_deref())
        .collect();
    for expected in [
        "decidir",
        "decidir/t1",
        "decidir/e1",
        "lote",
        "lote/b0/p1",
        "lote/b1/p2",
        "arquivos",
    ] {
        assert!(paths.contains(&expected), "missing {expected}: {paths:?}");
    }

    ensure_dag_node(
        &ctx.db,
        &run.id,
        "arquivos/0000/analisar",
        Some("arquivos/0000"),
        "analisar",
        "step",
        Some("0000"),
        None,
    )
    .await
    .expect("dynamic node");
    let node = node_by_path(&ctx.db, &run.id, "arquivos/0000/analisar")
        .await
        .expect("query")
        .expect("node");
    assert_eq!(node.item_key.as_deref(), Some("0000"));
    assert_eq!(node.parent_path.as_deref(), Some("arquivos/0000"));
}

#[tokio::test]
async fn concurrent_saves_keep_one_valid_recipe_file() {
    let temp = TempDir::new().expect("temp");
    let pool = opendesk_storage::init(&temp.path().join("r.db"))
        .await
        .expect("db");
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
        .bind(temp.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .expect("ws");
    let yaml = "id: corrida\nname: Corrida\ntrigger:\n  type: MANUAL\n  config: {}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n";
    let (first, second) = tokio::join!(
        save_yaml(&pool, "ws-1", "corrida", yaml),
        save_yaml(&pool, "ws-1", "corrida", yaml),
    );
    first.expect("first");
    second.expect("second");
    let path = temp.path().join(".opencode/recipes/corrida.yaml");
    let content = std::fs::read_to_string(&path).expect("file");
    crate::parser::parse(&content).expect("parse");
    assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 1);
    let versions = super::versions::list_versions(&pool, "ws-1", "corrida")
        .await
        .expect("versions");
    assert!(!versions.is_empty());
}
