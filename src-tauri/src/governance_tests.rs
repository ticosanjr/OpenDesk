use super::*;
use crate::test_support;

#[tokio::test]
async fn profile_goals_performance_and_budget() {
    let ctx = test_support::context().await;
    let profile = upsert_profile(
        &ctx.state.db,
        &ctx.workspace_id,
        "build",
        AgentProfileInput {
            role: Some("Executor".into()),
            team: Some("Entrega".into()),
            manager_id: Some("plan".into()),
            monthly_budget_usd: Some(1.0),
            concurrency_quota: Some(2),
            heartbeat_enabled: Some(false),
            heartbeat_minutes: Some(30),
            notes: Some("teste".into()),
        },
    )
    .await
    .expect("profile");
    assert_eq!(profile.role.as_deref(), Some("Executor"));
    assert_eq!(profile.concurrency_quota, 2);

    let goal = create_goal(
        &ctx.state.db,
        &ctx.workspace_id,
        AgentGoalInput {
            agent_id: "build".into(),
            title: "Entregar 10 tarefas".into(),
            description: None,
            metric: Some("tarefas".into()),
            target_value: Some(10.0),
            current_value: Some(3.0),
            unit: Some("tarefas".into()),
            due_at: None,
            status: None,
        },
    )
    .await
    .expect("goal");
    assert_eq!(goal.current_value, 3.0);
    assert!(
        active_goal_summary(&ctx.state.db, &ctx.workspace_id, "build")
            .await
            .expect("summary")
            .contains("10")
    );

    let budget = check_budget(&ctx.state.db, &ctx.workspace_id, "build").await;
    assert!(budget.is_ok(), "budget: {budget:?}");
    sqlx::query(
        "INSERT INTO task_runs (
            id, workspace_id, agent_name, execution_mode, model_id, effort_level,
            status, estimated_cost_usd, duration_ms
         ) VALUES ('run-gov', ?, 'build', 'build', 'm', 'high', 'SUCCESS', 2.0, 1000)",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("run");
    assert!(
        check_budget(&ctx.state.db, &ctx.workspace_id, "build")
            .await
            .is_err()
    );
    let spends = monthly_spend_by_agent(&ctx.state.db, &ctx.workspace_id)
        .await
        .expect("spends");
    assert_eq!(spends.len(), 1);
    assert!((spends.get("build").copied().unwrap_or_default() - 2.0).abs() < f64::EPSILON);
    let rows = performance(&ctx.state.db, &ctx.workspace_id)
        .await
        .expect("performance");
    assert_eq!(rows[0].agent_id, "build");
    assert_eq!(rows[0].success, 1);
    assert!((rows[0].success_rate - 1.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn delegation_lifecycle() {
    let ctx = test_support::context().await;
    let delegation = create_delegation(
        &ctx.state.db,
        &ctx.workspace_id,
        DelegationInput {
            from_agent: "plan".into(),
            to_agent: "build".into(),
            title: "Implementar".into(),
            prompt: "Faça o build.".into(),
            requested_by: Some("test".into()),
        },
    )
    .await
    .expect("delegation");
    assert_eq!(delegation.status, "PENDING");
    let resolved = resolve_delegation(
        &ctx.state.db,
        &ctx.workspace_id,
        &delegation.id,
        "APPROVED",
        None,
        Some("test"),
        None,
    )
    .await
    .expect("resolve");
    assert_eq!(resolved.status, "APPROVED");
}
