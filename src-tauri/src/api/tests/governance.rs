use super::*;

#[tokio::test]
async fn governance_profiles_goals_and_delegations_round_trip() {
    let ctx = test_support::context().await;
    let workspace = format!("?workspace_id={}", ctx.workspace_id);
    std::fs::create_dir_all(ctx.workspace_root.join(".opencode/agents")).expect("agents dir");
    std::fs::write(
        ctx.workspace_root.join(".opencode/agents/global-agent.md"),
        "---\ndescription: Agente de teste\nmode: subagent\n---\nVocê é um agente de teste.\n",
    )
    .expect("agent");

    let (status, overview) = call(
        &ctx,
        Method::GET,
        &format!("/api/governance{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!overview["agents"].as_array().expect("agents").is_empty());
    assert_eq!(overview["profiles"].as_array().expect("profiles").len(), 0);

    let (status, profile) = call(
        &ctx,
        Method::PUT,
        &format!("/api/governance/agents/global-agent{workspace}"),
        Some(json!({
            "role": "Pesquisador",
            "team": "Inteligência",
            "monthly_budget_usd": 25.0,
            "concurrency_quota": 2,
            "heartbeat_enabled": false,
            "heartbeat_minutes": 30
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "profile: {profile}");
    assert_eq!(profile["role"], "Pesquisador");
    assert_eq!(profile["concurrency_quota"], 2);

    let (status, goal) = call(
        &ctx,
        Method::POST,
        &format!("/api/governance/goals{workspace}"),
        Some(json!({
            "agent_id": "global-agent",
            "title": "Publicar 4 relatórios",
            "target_value": 4,
            "current_value": 1,
            "unit": "relatórios"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "goal: {goal}");
    assert_eq!(goal["current_value"], 1.0);

    let (status, delegation) = call(
        &ctx,
        Method::POST,
        &format!("/api/governance/delegations{workspace}"),
        Some(json!({
            "from_agent": "build",
            "to_agent": "global-agent",
            "title": "Revisar fontes",
            "prompt": "Revise as fontes e proponha um resumo."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "delegation: {delegation}");
    assert_eq!(delegation["status"], "PENDING");
    let id = delegation["id"].as_str().expect("id");

    let (status, rejected) = call(
        &ctx,
        Method::POST,
        &format!("/api/governance/delegations/{id}/resolve{workspace}"),
        Some(json!({ "approved": false, "resolved_by": "test" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rejected["status"], "REJECTED");

    let (status, overview) = call(
        &ctx,
        Method::GET,
        &format!("/api/governance{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(overview["goals"].as_array().expect("goals").len(), 1);
    assert_eq!(
        overview["profiles"][0]["profile"]["agent_id"],
        "global-agent"
    );
    assert_eq!(overview["profiles"][0]["monthly_spend_usd"], 0.0);
}

#[tokio::test]
async fn governance_blocks_dispatch_when_agent_budget_is_exhausted() {
    let ctx = test_support::context().await;
    let workspace = format!("?workspace_id={}", ctx.workspace_id);
    call(
        &ctx,
        Method::PUT,
        &format!("/api/governance/agents/build{workspace}"),
        Some(json!({ "monthly_budget_usd": 0.01 })),
    )
    .await;
    sqlx::query(
        "INSERT INTO task_runs (
            id, workspace_id, agent_name, execution_mode, model_id, effort_level,
            status, estimated_cost_usd, duration_ms
         ) VALUES ('run-budget', ?, 'build', 'build', 'm', 'high', 'SUCCESS', 1.0, 1000)",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("run");

    let created = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks{workspace}"),
        Some(json!({
            "title": "Tarefa bloqueada por budget",
            "description": "teste",
            "execution_mode": "BUILD_DIRECT",
            "assigned_agent": "build"
        })),
    )
    .await;
    let task = created.1;
    let id = task["id"].as_str().expect("task id");
    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/dispatch{workspace}"),
        Some(json!({ "manual_agent": "build" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("budget")
    );
}
