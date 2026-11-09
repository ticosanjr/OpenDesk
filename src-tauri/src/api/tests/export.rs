use super::*;
use crate::db::runs::{CreateRunInput, RunUsage};

#[tokio::test]
async fn exports_runs_as_markdown_html_and_json() {
    let ctx = test_support::context().await;
    let log = ctx.state.paths.run_logs_dir.join("run-export.jsonl");
    std::fs::write(
        &log,
        concat!(
            "{\"created\":1000,\"type\":\"session.execution.started\",\"data\":{}}\n",
            "{\"created\":1100,\"type\":\"session.text.delta\",\"data\":{\"delta\":\"resultado exportado\"}}\n"
        ),
    )
    .expect("log");
    runs::create(
        &ctx.state.db,
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
            log_path: Some(log.to_string_lossy().to_string()),
        },
    )
    .await
    .expect("run");
    runs::finish(
        &ctx.state.db,
        "run-export",
        "SUCCESS",
        &RunUsage::default(),
        1200,
        Some("resultado exportado"),
        None,
    )
    .await
    .expect("finish");

    let (status, body) = call(
        &ctx,
        Method::GET,
        "/api/runs/run-export/export?format=md",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let text = body.as_str().unwrap_or_default();
    assert!(text.contains("# Execução") || text.contains("Execução"));
    assert!(text.contains("resultado exportado"));
    assert!(text.contains("run-export"));

    let (status, body) = call(
        &ctx,
        Method::GET,
        "/api/runs/run-export/export?format=html",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.as_str()
            .unwrap_or_default()
            .starts_with("<!doctype html>")
    );

    let (status, body) = call(
        &ctx,
        Method::GET,
        "/api/runs/run-export/export?format=json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["run"]["id"], "run-export");
    assert_eq!(body["status"], "RUNNING");

    let (status, _) = call(
        &ctx,
        Method::GET,
        "/api/runs/run-export/export?format=pdf",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
