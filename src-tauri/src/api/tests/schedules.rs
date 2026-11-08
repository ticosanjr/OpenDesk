use super::*;

#[tokio::test]
async fn routine_source_is_created_and_removed_by_api() {
    let ctx = test_support::context().await;
    let routine = json!({
        "workspace_id": ctx.workspace_id,
        "name": "Rotina pasta",
        "description": "teste",
        "cron_expression": "",
        "timezone": "America/Sao_Paulo",
        "agent_name": null,
        "command_or_prompt": "teste",
        "trigger_type": "FOLDER_WATCH",
        "trigger_config_json": "{\"path\":\"input\"}",
        "target_kind": "PROMPT",
        "recipe_id": null,
        "router_policy": "COST_BENEFIT",
        "output_folder_template": "output/{YYYY}-{MM}/{schedule_slug}",
        "notify_email_recipients": null,
        "notify_signal": false,
        "notify_signal_recipients": null,
        "notify_only_on_change": false,
        "recurrence_json": null,
        "one_time": false,
        "misfire_policy": "SKIP_TO_NEXT",
        "safe_readonly_mode": true,
        "sync_to_google_docs": false,
        "sync_to_google_calendar": false,
        "is_active": true
    });
    let (status, schedule) = call(&ctx, Method::POST, "/api/schedules", Some(routine)).await;
    assert_eq!(status, StatusCode::OK);
    let id = schedule["id"].as_str().expect("id").to_string();
    let (_, sources) = call(
        &ctx,
        Method::GET,
        &format!("/api/io/inbound?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(sources["sources"].as_array().unwrap().len(), 1);

    let (status, _) = call(&ctx, Method::DELETE, &format!("/api/schedules/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, sources) = call(
        &ctx,
        Method::GET,
        &format!("/api/io/inbound?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert!(sources["sources"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn schedule_lifecycle_preview_toggle_and_runs() {
    let ctx = test_support::context().await;
    let (status, created) = call(
        &ctx,
        Method::POST,
        "/api/schedules",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "name": "Rotina Trimestral",
            "description": "fechamento",
            "cron_expression": "0 9 * * 1",
            "timezone": "UTC",
            "command_or_prompt": "Gerar resumo trimestral",
            "trigger_type": "CRON",
            "target_kind": "PROMPT"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = created["id"].as_str().expect("schedule id").to_string();
    assert!(created["next_run_at"].is_string());

    let (status, list) = call(&ctx, Method::GET, "/api/schedules", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().expect("schedules").len(), 1);

    let (status, fetched) = call(&ctx, Method::GET, &format!("/api/schedules/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["name"], "Rotina Trimestral");

    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/schedules/{id}"),
        Some(json!({ "description": "fechamento mensal", "priority": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["description"], "fechamento mensal");

    let (status, toggled) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{id}/active"),
        Some(json!({ "active": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(toggled["is_active"], false);

    let (status, preview) = call(
        &ctx,
        Method::POST,
        "/api/schedules/preview",
        Some(json!({ "recurrence": { "frequency": "daily", "time": "08:00" }, "timezone": "UTC" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        preview["next_runs"]
            .as_array()
            .is_some_and(|runs| !runs.is_empty())
    );
    assert!(preview["cron_expression"].is_string());

    let (status, runs) = call(
        &ctx,
        Method::GET,
        &format!("/api/schedules/{id}/runs"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(runs.is_array());

    let (status, _dry) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{id}/dry-run"),
        Some(json!({ "target_kind": "PROMPT" })),
    )
    .await;
    assert!(matches!(
        status,
        StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
    ));

    let (status, _) = call(&ctx, Method::DELETE, &format!("/api/schedules/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn schedule_dry_run_notifications_and_engine_guards() {
    let ctx = test_support::context().await;
    let (_, schedule) = call(
        &ctx,
        Method::POST,
        "/api/schedules",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "name": "Dry Run",
            "command_or_prompt": "executar",
            "cron_expression": "0 9 * * *",
            "timezone": "UTC"
        })),
    )
    .await;
    let id = schedule["id"].as_str().expect("schedule");

    let (status, result) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{id}/dry-run"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["ok"], false);
    assert!(result["checks"].as_array().is_some_and(|checks| {
        checks
            .iter()
            .any(|check| check["name"] == "Prompt" && check["ok"] == true)
    }));

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{id}/notify-test"),
        Some(json!({ "channel": "email" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{id}/notify-test"),
        Some(json!({ "channel": "signal" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{id}/run"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn schedule_trigger_sources_follow_the_trigger_type() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);
    for trigger in ["FOLDER_WATCH", "IMAP_IDLE", "WEBHOOK", "SQL_POLL"] {
        let (status, schedule) = call(
            &ctx,
            Method::POST,
            "/api/schedules",
            Some(json!({
                "workspace_id": ctx.workspace_id,
                "name": format!("Rotina {trigger}"),
                "command_or_prompt": "executar",
                "cron_expression": "",
                "timezone": "UTC",
                "trigger_type": trigger,
                "trigger_config_json": "{\"path\":\"input\"}"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{trigger}");
        let id = schedule["id"].as_str().expect("schedule");
        let (_, sources) = call(&ctx, Method::GET, &format!("/api/io/inbound{query}"), None).await;
        assert!(
            sources["sources"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["config_json"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(id))),
            "{trigger} source"
        );

        let (status, _) = call(
            &ctx,
            Method::PATCH,
            &format!("/api/schedules/{id}"),
            Some(json!({ "trigger_type": "CRON", "cron_expression": "0 9 * * *" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (_, sources) = call(&ctx, Method::GET, &format!("/api/io/inbound{query}"), None).await;
        assert!(
            sources["sources"]
                .as_array()
                .is_some_and(|items| items.iter().all(|item| !item["config_json"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(id))),
            "{trigger} source removed"
        );

        let (status, _) = call(&ctx, Method::DELETE, &format!("/api/schedules/{id}"), None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
}
