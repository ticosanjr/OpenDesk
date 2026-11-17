use super::*;

#[tokio::test]
async fn shadow_replay_requires_cases() {
    let ctx = test_support::context().await;
    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!("/api/skills/demo/shadow?workspace_id={}", ctx.workspace_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("casos curados"),
        "{body}"
    );
}

#[tokio::test]
async fn rollback_without_backup_is_actionable() {
    let ctx = test_support::context().await;
    let dir = ctx.workspace_root.join(".opencode/skills/demo");
    std::fs::create_dir_all(&dir).expect("skill dir");
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: Demo\ndescription: Demo\n---\nUse a skill.\n",
    )
    .expect("skill");
    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!(
            "/api/skills/demo/rollback?workspace_id={}",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("nenhum backup"),
        "{body}"
    );
}
