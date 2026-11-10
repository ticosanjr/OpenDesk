use super::*;

#[tokio::test]
async fn calendar_events_crud_and_agenda_round_trip() {
    let ctx = test_support::context().await;
    let query = format!("workspace_id={}", ctx.workspace_id);
    let (status, event) = call(
        &ctx,
        Method::POST,
        &format!("/api/calendar/events?{query}"),
        Some(json!({
            "title": "Planejamento",
            "description": "Semana de QA",
            "location": "Sala 1",
            "start_at": "2026-11-10T12:00:00Z",
            "end_at": "2026-11-10T13:00:00Z",
            "all_day": false,
            "timezone": "UTC"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = event["id"].as_str().expect("event id").to_string();
    assert_eq!(event["title"], "Planejamento");

    let (status, events) = call(
        &ctx,
        Method::GET,
        &format!("/api/calendar/events?{query}&from=2026-11-09&to=2026-11-11"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(events.as_array().expect("events").len(), 1);

    let (status, sources) = call(
        &ctx,
        Method::GET,
        &format!("/api/calendar/sources?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        sources
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["provider"] == "LOCAL"))
    );

    let (status, agenda) = call(
        &ctx,
        Method::GET,
        &format!("/api/calendar/agenda?{query}&from=2026-11-09&to=2026-11-11"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        agenda["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["kind"] == "event"))
    );

    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/calendar/events/{id}"),
        Some(json!({
            "title": "Planejamento v2",
            "end_at": "2026-11-10T14:00:00Z"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["title"], "Planejamento v2");
    assert_eq!(updated["end_at"], "2026-11-10T14:00:00Z");

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/calendar/events/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, after) = call(
        &ctx,
        Method::GET,
        &format!("/api/calendar/events?{query}&from=2026-11-09&to=2026-11-11"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        after
            .as_array()
            .is_some_and(|items| items.iter().all(|item| item["id"] != id))
    );
}

#[tokio::test]
async fn calendar_event_validation_rejects_bad_input() {
    let ctx = test_support::context().await;
    let uri = format!("/api/calendar/events?workspace_id={}", ctx.workspace_id);
    for body in [
        json!({
            "title": "",
            "start_at": "2026-11-10T12:00:00Z",
            "end_at": "2026-11-10T13:00:00Z"
        }),
        json!({
            "title": "Invertido",
            "start_at": "2026-11-10T14:00:00Z",
            "end_at": "2026-11-10T13:00:00Z"
        }),
        json!({
            "title": "Fuso",
            "start_at": "2026-11-10T12:00:00Z",
            "end_at": "2026-11-10T13:00:00Z",
            "timezone": "Marte/Olympus"
        }),
        json!({
            "title": "Cor",
            "start_at": "2026-11-10T12:00:00Z",
            "end_at": "2026-11-10T13:00:00Z",
            "color_hex": "azul"
        }),
    ] {
        let (status, _) = call(&ctx, Method::POST, &uri, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, _) = call(
        &ctx,
        Method::PATCH,
        "/api/calendar/events/missing",
        Some(json!({ "title": "Nada" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn calendar_agenda_unifies_routines_recipes_and_runs() {
    let ctx = test_support::context().await;
    let query = format!("workspace_id={}", ctx.workspace_id);

    let (status, routine) = call(
        &ctx,
        Method::POST,
        "/api/schedules",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "name": "Rotina Calendário",
            "command_or_prompt": "executar",
            "cron_expression": "0 9 * * *",
            "timezone": "UTC",
            "trigger_type": "CRON"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(routine["is_active"], false);
    let routine_id = routine["id"].as_str().expect("routine id");
    let (status, activated) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{routine_id}/active"),
        Some(json!({ "active": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(activated["is_active"], true);

    let recipes_dir = ctx.workspace_root.join(".opencode/recipes");
    std::fs::create_dir_all(&recipes_dir).expect("recipes dir");
    std::fs::write(
        recipes_dir.join("receita-calendario.yaml"),
        r#"id: receita-calendario
name: Receita Calendário
enabled: true
triggers:
  - type: CRON
    config: {}
flow:
  - type: step
    id: passo-1
    name: Passo
    mode: build
    agent: build
    prompt: "Resuma"
"#,
    )
    .expect("recipe");
    let (status, _) = call(&ctx, Method::POST, "/api/recipes/sync", None).await;
    assert_eq!(status, StatusCode::OK);

    runs::create(
        &ctx.state.db,
        runs::CreateRunInput {
            id: "run-calendar".into(),
            queue_job_id: None,
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            schedule_id: None,
            parent_run_id: None,
            phase: Some("build".into()),
            agent_name: "build".into(),
            execution_mode: "BUILD_DIRECT".into(),
            provider_id: Some("opencode-go".into()),
            model_id: "glm-5.3".into(),
            effort_level: "low".into(),
            pricing_snapshot_json: None,
            log_path: None,
        },
    )
    .await
    .expect("run");

    let (status, agenda) = call(
        &ctx,
        Method::GET,
        &format!("/api/calendar/agenda?{query}&from=2026-11-01&to=2026-12-01"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = agenda["items"].as_array().expect("items");
    let kinds = items
        .iter()
        .filter_map(|item| item["kind"].as_str())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&"routine"), "{kinds:?}");
    assert!(kinds.contains(&"recipe"), "{kinds:?}");
    assert!(kinds.contains(&"run"), "{kinds:?}");
    assert!(
        items
            .iter()
            .filter(|item| item["kind"] == "routine")
            .all(|item| item["href"]
                .as_str()
                .unwrap_or_default()
                .contains("/routines"))
    );
}

#[tokio::test]
async fn calendar_sources_can_be_configured_and_removed() {
    let ctx = test_support::context().await;
    let query = format!("workspace_id={}", ctx.workspace_id);
    let google = calendar_db::upsert_google_source(
        &ctx.state.db,
        &ctx.workspace_id,
        "team@example.com",
        "Time",
        Some("#ff0000"),
        "America/Sao_Paulo",
    )
    .await
    .expect("google source");

    let (status, sources) = call(
        &ctx,
        Method::GET,
        &format!("/api/calendar/sources?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        sources
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["provider"] == "GOOGLE"))
    );

    calendar_db::upsert_external_event(
        &ctx.state.db,
        &ctx.workspace_id,
        &google.id,
        &calendar_db::ExternalEventInput {
            external_id: "google-event-1".into(),
            title: "Evento Google Oculto".into(),
            description: None,
            location: None,
            start_at: "2026-11-10T12:00:00Z".into(),
            end_at: "2026-11-10T13:00:00Z".into(),
            all_day: false,
            timezone: "UTC".into(),
            status: "CONFIRMED".into(),
            color_hex: Some("#ff0000".into()),
        },
    )
    .await
    .expect("google event");
    let agenda_uri = format!("/api/calendar/agenda?{query}&from=2026-11-09&to=2026-11-11");
    let (_, visible_agenda) = call(&ctx, Method::GET, &agenda_uri, None).await;
    assert!(visible_agenda["items"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["title"] == "Evento Google Oculto")
    }));

    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/calendar/sources/{}", google.id),
        Some(json!({ "is_visible": false, "sync_enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["is_visible"], false);
    assert_eq!(updated["sync_enabled"], false);
    let (_, hidden_agenda) = call(&ctx, Method::GET, &agenda_uri, None).await;
    assert!(hidden_agenda["items"].as_array().is_some_and(|items| {
        items
            .iter()
            .all(|item| item["title"] != "Evento Google Oculto")
    }));

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/calendar/sources/{}", google.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let local = calendar_db::ensure_local_source(&ctx.state.db, &ctx.workspace_id)
        .await
        .expect("local");
    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/calendar/sources/{}", local.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn calendar_google_import_without_oauth_reports_an_actionable_error() {
    let ctx = test_support::context().await;
    let query = format!("workspace_id={}", ctx.workspace_id);

    let (status, _) = call(
        &ctx,
        Method::GET,
        &format!("/api/calendar/google/calendars?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, imported) = call(
        &ctx,
        Method::POST,
        &format!("/api/calendar/google/import?{query}"),
        Some(json!({
            "calendars": [{
                "external_id": "team@example.com",
                "name": "Time",
                "color_hex": "#ff0000",
                "timezone": "America/Sao_Paulo"
            }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(imported["sources"].as_array().expect("sources").len(), 1);
    assert!(
        imported["reports"][0]["error"]
            .as_str()
            .is_some_and(|error| !error.is_empty())
    );

    let (status, synced) = call(&ctx, Method::POST, "/api/calendar/sync", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(synced["reports"].as_array().is_some());
}
