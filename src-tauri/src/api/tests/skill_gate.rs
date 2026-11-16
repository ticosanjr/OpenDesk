use super::*;

#[tokio::test]
async fn skill_gate_route_is_fail_closed_without_samples() {
    let ctx = test_support::context().await;
    let dir = ctx.workspace_root.join(".opencode/skills/gate-route");
    std::fs::create_dir_all(&dir).expect("skill dir");
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: Gate Route\ndescription: Route\n---\nUse.\n",
    )
    .expect("skill");

    let path = format!(
        "/api/skills/gate-route/gate?workspace_id={}",
        ctx.workspace_id
    );
    let (status, gate) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({ "days": 30, "min_samples": 1, "min_success_rate": 0.8 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{gate}");
    assert_eq!(gate["status"], "INSUFFICIENT");

    let (status, latest) = call(&ctx, Method::GET, &path, None).await;
    assert_eq!(status, StatusCode::OK, "{latest}");
    assert_eq!(latest["id"], gate["id"]);

    let gate_id = gate["id"].as_str().expect("id");
    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!(
            "/api/skills/gate-route/gate/{gate_id}/decide?workspace_id={}",
            ctx.workspace_id
        ),
        Some(json!({ "approved": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
