use super::*;

#[tokio::test]
async fn seed_populates_the_operational_fixture() {
    let ctx = test_support::context().await;
    let payload = json!({
        "workspace_id": ctx.workspace_id,
        "task": {
            "title": "Tarefa semeada",
            "description": "Fixture do contrato",
            "agent": "build",
            "tags": ["seed"],
            "status": "NEEDS_APPROVAL",
            "phase_status": "plan_ready"
        },
        "run": {
            "status": "FAILED",
            "agent": "build",
            "model": "opencode-go/glm-5.3",
            "duration_ms": 1200,
            "tokens_input": 10,
            "tokens_output": 5,
            "cost_usd": 0.001,
            "summary": "Resumo semeado",
            "error": "falha controlada",
            "error_code": "seed_failure",
            "events": [
                { "created": 1000, "type": "session.execution.started", "data": {} },
                {
                    "created": 1200,
                    "type": "session.reasoning.delta",
                    "data": { "delta": "Vou analisar o briefing." }
                }
            ]
        },
        "artifacts": [
            {
                "file_path": "output/seed/relatorio.md",
                "content": "# Relatório",
                "action_type": "CREATED",
                "agent": "build",
                "reason": "seed"
            }
        ],
        "hitl": {
            "checkpoint_id": "cp-seed",
            "title": "Aprovar?",
            "question": "Seguir?",
            "options": ["sim", "não"],
            "on_timeout": "PAUSE_AND_ALERT"
        },
        "proposal": {
            "kind": "SKILL",
            "action": "CREATE",
            "title": "Skill semeada",
            "description": "proposta",
            "payload": { "id": "semeada" }
        },
        "permission": {
            "action": "bash",
            "resources": ["rm -rf build"],
            "session_id": "session-seed"
        }
    });

    let (status, body) = call(&ctx, Method::POST, "/api/test/seed", Some(payload)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["workspace_id"], ctx.workspace_id);
    assert!(body["task_id"].is_string());
    assert!(body["run_id"].is_string());

    let workspace = format!("?workspace_id={}", ctx.workspace_id);
    let (status, attention) = call(
        &ctx,
        Method::GET,
        &format!("/api/attention{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(attention["hitl"].as_array().expect("hitl").len(), 1);
    assert_eq!(
        attention["proposals"].as_array().expect("proposals").len(),
        1
    );
    assert_eq!(attention["requests"].as_array().expect("requests").len(), 1);

    let (status, artifacts) = call(
        &ctx,
        Method::GET,
        &format!("/api/artifacts{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(artifacts.as_array().expect("artifacts").len(), 1);

    let run_id = body["run_id"].as_str().expect("run id");
    let (status, thread) = call(
        &ctx,
        Method::GET,
        &format!("/api/runs/{run_id}/thread"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!thread["events"].as_array().expect("events").is_empty());
}

#[tokio::test]
async fn seed_requires_a_workspace() {
    let ctx = test_support::context().await;
    let (status, body) = call(
        &ctx,
        Method::POST,
        "/api/test/seed",
        Some(json!({ "workspace_id": "missing" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}
