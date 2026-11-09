use super::*;

#[tokio::test]
async fn metrics_endpoint_returns_runs_failures_and_agents() {
    let ctx = test_support::context().await;
    sqlx::query(
        "INSERT INTO task_runs (
            id, workspace_id, agent_name, execution_mode, model_id, effort_level,
            status, estimated_cost_usd, duration_ms, tokens_input, tokens_output,
            error_code, started_at
         ) VALUES
         ('m1', ?, 'build', 'build', 'm', 'high', 'SUCCESS', 1.0, 1000, 10, 20, NULL,
          strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 day')),
         ('m2', ?, 'build', 'build', 'm', 'high', 'FAILED', 2.0, 5000, 10, 20, 'timeout',
          strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
    )
    .bind(&ctx.workspace_id)
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("runs");

    let (status, body) = call(
        &ctx,
        Method::GET,
        &format!("/api/metrics?workspace_id={}&days=30", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["runs_total"], 2);
    assert_eq!(body["runs_failed"], 1);
    assert_eq!(body["failure_codes"][0]["code"], "timeout");
    assert_eq!(body["agents"][0]["agent_id"], "build");
    assert_eq!(body["daily"].as_array().expect("daily").len(), 2);
    assert!(body["p95_duration_ms"].as_i64().unwrap_or(0) >= 1000);
}
