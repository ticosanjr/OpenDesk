use super::*;

#[tokio::test]
async fn exposes_the_task_session_packet() {
    let ctx = test_support::context().await;
    let (status, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "title": "Pacote de sessão",
            "description": "Continuar entre harnesses",
            "execution_mode": "BUILD_DIRECT"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{task}");
    let task_id = task["id"].as_str().expect("task id");
    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/tasks/{task_id}"),
        Some(json!({ "plan_markdown": "1. planejar\n2. executar" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");

    let (status, packet) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{task_id}/session-packet"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{packet}");
    assert_eq!(packet["task_id"], task_id);
    assert!(
        packet["plan"]
            .as_str()
            .unwrap_or_default()
            .contains("executar"),
        "{packet}"
    );
    assert!(packet["budget"].is_object(), "{packet}");
}
