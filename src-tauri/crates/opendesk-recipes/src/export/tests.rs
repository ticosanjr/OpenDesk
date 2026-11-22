use super::cache::cached_events;
use super::inputs::load_run_inputs;
use opendesk_engine::run_timeline::RunTimeline;

use super::*;
use crate::test_support;
use opendesk_storage::repos::runs::{CreateRunInput, RunUsage};
use std::io::Write;

#[test]
fn timeline_cache_reads_only_complete_appended_lines() {
    let dir = tempfile::TempDir::new().expect("temp");
    let path = dir.path().join("run.jsonl");
    std::fs::write(
        &path,
        "{\"created\":1,\"type\":\"a\"}\n{\"created\":2,\"type\":\"b\"}\n",
    )
    .expect("log");
    assert_eq!(cached_events(&path).expect("events").len(), 2);

    // A half-written line is not consumed.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append");
    write!(file, "{{\"created\":3,").expect("partial");
    file.flush().expect("flush");
    drop(file);
    assert_eq!(cached_events(&path).expect("events").len(), 2);

    // Completing the line appends exactly one event.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append");
    writeln!(file, "\"type\":\"c\"}}").expect("complete");
    drop(file);
    assert_eq!(cached_events(&path).expect("events").len(), 3);

    // A smaller rewrite forces a rebuild.
    std::fs::write(&path, "{\"created\":9,\"type\":\"z\"}\n").expect("rewrite");
    let rebuilt = cached_events(&path).expect("events");
    assert_eq!(rebuilt.len(), 1);
    assert_eq!(rebuilt[0]["type"], "z");
}

#[tokio::test]
async fn exposes_recipe_trigger_files_as_run_inputs() {
    let ctx = test_support::context().await;
    let yaml = "id: inputs-recipe\nname: Inputs\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo\n    prompt: oi\n";
    let recipe = crate::parser::parse(yaml).expect("recipe");
    sqlx::query(
        "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
             VALUES ('inputs-recipe', ?, 'Inputs', '/tmp/inputs.yaml', 'MANUAL')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.db)
    .await
    .expect("index");
    let task = opendesk_storage::repos::tasks::create(
        &ctx.db,
        &ctx.workspace_id,
        opendesk_storage::repos::tasks::CreateTaskInput {
            title: "Entrada".into(),
            description: "Processe o arquivo".into(),
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: None,
            preferred_model: None,
            preferred_harness: None,
            effort_level: None,
            care_level: None,
            notify_email: None,
            export_to_gdocs: false,
            tags: vec![],
            origin: None,
            parent_task_id: None,
            dedupe: Some(false),
        },
    )
    .await
    .expect("task");
    crate::store::create_run(
        &ctx.db,
        &recipe,
        &ctx.workspace_id,
        Some(&task.id),
        &serde_json::json!({
            "file_path": "input/dropzone/relatorio.pdf",
            "file_name": "relatorio.pdf",
            "file_size": 2048,
            "source_name": "Pasta monitorada",
            "trigger_type": "FOLDER_WATCH",
            "attachments": ["input/extra.txt"]
        }),
        "/tmp/out",
        false,
    )
    .await
    .expect("recipe run");
    let run = opendesk_storage::repos::runs::create(
        &ctx.db,
        CreateRunInput {
            id: "run-inputs".into(),
            queue_job_id: None,
            workspace_id: ctx.workspace_id.clone(),
            task_id: Some(task.id.clone()),
            schedule_id: None,
            parent_run_id: None,
            phase: Some("build".into()),
            agent_name: "build".into(),
            execution_mode: "build".into(),
            provider_id: None,
            model_id: "glm".into(),
            effort_level: "medium".into(),
            pricing_snapshot_json: None,
            log_path: None,
        },
    )
    .await
    .expect("run");
    let workspace = opendesk_storage::repos::workspaces::get(&ctx.db, &ctx.workspace_id)
        .await
        .expect("workspace");
    let inputs = load_run_inputs(&ctx.db, &run, Some(&workspace)).await;
    assert_eq!(inputs.len(), 2);
    let report = inputs
        .iter()
        .find(|input| input.name == "relatorio.pdf")
        .expect("report");
    assert_eq!(report.size, Some(2048));
    assert_eq!(report.trigger_type.as_deref(), Some("FOLDER_WATCH"));
    assert!(
        report
            .absolute_path
            .ends_with("input/dropzone/relatorio.pdf")
    );
    assert!(inputs.iter().any(|input| input.path == "input/extra.txt"));
}

#[tokio::test]
async fn reads_task_description_inputs_when_no_recipe_context_exists() {
    let ctx = test_support::context().await;
    let task = opendesk_storage::repos::tasks::create(
        &ctx.db,
        &ctx.workspace_id,
        opendesk_storage::repos::tasks::CreateTaskInput {
            title: "Entrada manual".into(),
            description:
                "Processe os arquivos.\n\n## Arquivos de entrada\n\n- `input/manual.pdf`\n".into(),
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: None,
            preferred_model: None,
            preferred_harness: None,
            effort_level: None,
            care_level: None,
            notify_email: None,
            export_to_gdocs: false,
            tags: vec![],
            origin: None,
            parent_task_id: None,
            dedupe: Some(false),
        },
    )
    .await
    .expect("task");
    let run = opendesk_storage::repos::runs::create(
        &ctx.db,
        CreateRunInput {
            id: "run-task-inputs".into(),
            queue_job_id: None,
            workspace_id: ctx.workspace_id.clone(),
            task_id: Some(task.id.clone()),
            schedule_id: None,
            parent_run_id: None,
            phase: Some("build".into()),
            agent_name: "build".into(),
            execution_mode: "build".into(),
            provider_id: None,
            model_id: "glm".into(),
            effort_level: "medium".into(),
            pricing_snapshot_json: None,
            log_path: None,
        },
    )
    .await
    .expect("run");
    let workspace = opendesk_storage::repos::workspaces::get(&ctx.db, &ctx.workspace_id)
        .await
        .expect("workspace");
    let inputs = load_run_inputs(&ctx.db, &run, Some(&workspace)).await;
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].name, "manual.pdf");
    assert_eq!(inputs[0].source.as_deref(), Some("task"));
}

#[tokio::test]
async fn renders_all_formats_from_a_run() {
    let ctx = test_support::context().await;
    opendesk_storage::repos::runs::create(
        &ctx.db,
        CreateRunInput {
            id: "run-export".into(),
            queue_job_id: None,
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            schedule_id: None,
            parent_run_id: None,
            phase: None,
            agent_name: "build".into(),
            execution_mode: "build".into(),
            provider_id: Some("opencode-go".into()),
            model_id: "glm-5.3".into(),
            effort_level: "high".into(),
            pricing_snapshot_json: None,
            log_path: None,
        },
    )
    .await
    .expect("run");
    opendesk_storage::repos::runs::finish(
        &ctx.db,
        "run-export",
        "SUCCESS",
        &RunUsage::default(),
        1200,
        Some("resultado final"),
        None,
    )
    .await
    .expect("finish");

    let timeline = load_timeline(&ctx.db, "run-export")
        .await
        .expect("timeline");
    let md = render_markdown(&timeline);
    assert!(md.contains("resultado final"));
    assert!(md.contains("run-export"));
    let html = render_html(&timeline);
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("resultado final"));
    let (body, mime, ext) = render(&timeline, "json").expect("json");
    assert_eq!(mime, "application/json; charset=utf-8");
    assert_eq!(ext, "json");
    assert!(body.contains("\"run-export\""));
}

#[test]
fn rejects_unknown_formats() {
    let timeline = RunTimeline::default();
    assert!(render(&timeline, "pdf").is_err());
}
