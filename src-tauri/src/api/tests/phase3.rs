use super::*;

#[tokio::test]
async fn task_watchers_replace_and_validate_channels() {
    let ctx = test_support::context().await;
    let (status, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "title": "Acompanhada",
            "description": "Avise quando concluir."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{task}");
    let task_id = task["id"].as_str().expect("task id");

    let (status, watchers) = call(
        &ctx,
        Method::PUT,
        &format!("/api/tasks/{task_id}/watchers"),
        Some(json!({ "channels": ["telegram", "signal", "telegram"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{watchers}");
    assert_eq!(watchers.as_array().map(Vec::len), Some(2));

    let (status, listed) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{task_id}/watchers"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed.as_array().is_some_and(|items| items.len() == 2));

    let (status, cleared) = call(
        &ctx,
        Method::PUT,
        &format!("/api/tasks/{task_id}/watchers"),
        Some(json!({ "channels": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared.as_array().is_some_and(Vec::is_empty));

    let (status, error) = call(
        &ctx,
        Method::PUT,
        &format!("/api/tasks/{task_id}/watchers"),
        Some(json!({ "channels": ["email"] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
}
