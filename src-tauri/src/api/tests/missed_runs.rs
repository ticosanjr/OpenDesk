use super::*;
use crate::db::missed_runs;

async fn schedule(ctx: &TestContext, id: &str) {
    sqlx::query(
        "INSERT INTO schedules (id, workspace_id, name, cron_expression, command_or_prompt)
         VALUES (?, ?, ?, '0 9 * * *', 'resumo')",
    )
    .bind(id)
    .bind(&ctx.workspace_id)
    .bind(format!("Rotina {id}"))
    .execute(&ctx.state.db)
    .await
    .expect("schedule");
}

#[tokio::test]
async fn lists_and_ignores_missed_runs() {
    let ctx = test_support::context().await;
    schedule(&ctx, "schedule-1").await;
    let missed = missed_runs::create_or_ignore(
        &ctx.state.db,
        &ctx.workspace_id,
        "schedule-1",
        "2026-11-10T09:00:00Z",
        "OFFLINE",
    )
    .await
    .expect("missed");
    let workspace = format!("?workspace_id={}", ctx.workspace_id);

    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/missed-runs{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().expect("list").len(), 1);
    assert_eq!(list[0]["status"], "PENDING");

    let (status, ignored) = call(
        &ctx,
        Method::POST,
        &format!("/api/missed-runs/{}/ignore", missed.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ignored["status"], "IGNORED");

    let (status, pending) = call(
        &ctx,
        Method::POST,
        &format!("/api/missed-runs/run-all{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(pending.as_array().expect("pending").is_empty());
}

#[tokio::test]
async fn ignore_all_closes_every_pending_entry() {
    let ctx = test_support::context().await;
    for (id, reason) in [("s1", "OFFLINE"), ("s2", "LATE")] {
        schedule(&ctx, id).await;
        missed_runs::create_or_ignore(
            &ctx.state.db,
            &ctx.workspace_id,
            id,
            "2026-11-10T09:00:00Z",
            reason,
        )
        .await
        .expect("missed");
    }
    let workspace = format!("?workspace_id={}", ctx.workspace_id);
    let (status, ignored) = call(
        &ctx,
        Method::POST,
        &format!("/api/missed-runs/ignore-all{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ignored.as_array().expect("array").len(), 2);
    assert!(
        ignored
            .as_array()
            .expect("array")
            .iter()
            .all(|row| row["status"] == "IGNORED")
    );
}
