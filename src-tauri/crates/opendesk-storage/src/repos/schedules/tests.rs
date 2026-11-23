use super::*;
use sqlx::SqlitePool;
use tempfile::TempDir;

async fn setup() -> (TempDir, SqlitePool) {
    let temp = TempDir::new().expect("temp");
    let pool = crate::db::init(&temp.path().join("schedules.db"))
        .await
        .expect("db");
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
        .bind(temp.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .expect("workspace");
    (temp, pool)
}

fn input() -> CreateScheduleInput {
    CreateScheduleInput {
        workspace_id: "ws-1".into(),
        name: "Diário".into(),
        description: Some("Resumo".into()),
        cron_expression: "0 8 * * *".into(),
        timezone: "America/Sao_Paulo".into(),
        agent_name: Some("build".into()),
        command_or_prompt: "Produza o resumo".into(),
        trigger_type: "CRON".into(),
        trigger_config_json: None,
        target_kind: "PROMPT".into(),
        recipe_id: None,
        router_policy: "cost_benefit".into(),
        output_folder_template: "output/{YYYY}-{MM}/{schedule_slug}".into(),
        notify_email_recipients: Some("ops@example.com".into()),
        notify_signal: false,
        notify_signal_recipients: None,
        notify_telegram: false,
        notify_telegram_recipients: None,
        notify_only_on_change: false,
        recurrence_json: None,
        one_time: false,
        misfire_policy: "SKIP_TO_NEXT".into(),
        safe_readonly_mode: true,
        sync_to_google_docs: true,
        sync_to_google_calendar: false,
        is_active: true,
    }
}

#[tokio::test]
async fn schedule_lifecycle() {
    let (_temp, pool) = setup().await;
    let schedule = create(&pool, input()).await.expect("create");
    assert_eq!(schedule.router_policy, "COST_BENEFIT");
    assert!(!schedule.is_active, "new routines always start paused");
    assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 1);
    assert!(active(&pool).await.expect("active").is_empty());

    let enabled = set_active(&pool, &schedule.id, true)
        .await
        .expect("activate");
    assert!(enabled.is_active);
    assert_eq!(active(&pool).await.expect("active").len(), 1);

    let updated = update(
        &pool,
        &schedule.id,
        UpdateScheduleInput {
            name: Some("Diário v2".into()),
            sync_to_google_docs: Some(false),
            ..Default::default()
        },
    )
    .await
    .expect("update");
    assert_eq!(updated.name, "Diário v2");
    assert!(!updated.sync_to_google_docs);

    set_next_run(&pool, &schedule.id, Some("2026-11-07T08:00:00Z"))
        .await
        .expect("next");
    mark_run(&pool, &schedule.id, "SUCCESS", "2026-11-06T08:00:00Z")
        .await
        .expect("mark");
    let after = get(&pool, &schedule.id).await.expect("get");
    assert_eq!(after.next_run_at.as_deref(), Some("2026-11-07T08:00:00Z"));
    assert_eq!(after.last_run_status.as_deref(), Some("SUCCESS"));

    let inactive = set_active(&pool, &schedule.id, false)
        .await
        .expect("active");
    assert!(!inactive.is_active);
    assert!(active(&pool).await.expect("active list").is_empty());

    delete(&pool, &schedule.id).await.expect("delete");
    assert!(get(&pool, &schedule.id).await.is_err());
}

#[tokio::test]
async fn persists_recurrence_and_notification_settings() {
    let (_temp, pool) = setup().await;
    let mut value = input();
    value.recurrence_json =
        Some(r#"{"frequency":"weekly","interval":1,"time":"09:30","weekdays":[1,3]}"#.into());
    value.notify_signal = true;
    value.notify_signal_recipients = Some("+5511999999999".into());
    value.notify_telegram = true;
    value.notify_telegram_recipients = Some("123456".into());
    value.notify_only_on_change = true;
    value.one_time = false;
    let schedule = create(&pool, value).await.expect("create");

    let loaded = get(&pool, &schedule.id).await.expect("get");
    assert!(loaded.notify_signal);
    assert_eq!(
        loaded.notify_signal_recipients.as_deref(),
        Some("+5511999999999")
    );
    assert!(loaded.notify_telegram);
    assert_eq!(loaded.notify_telegram_recipients.as_deref(), Some("123456"));
    assert!(loaded.notify_only_on_change);
    assert_eq!(
        loaded.recurrence_json.as_deref(),
        Some(r#"{"frequency":"weekly","interval":1,"time":"09:30","weekdays":[1,3]}"#)
    );
    assert!(!loaded.one_time);
    assert!(loaded.last_output_hash.is_none());

    set_last_output_hash(&pool, &schedule.id, Some("abc"))
        .await
        .expect("hash");
    let loaded = get(&pool, &schedule.id).await.expect("get");
    assert_eq!(loaded.last_output_hash.as_deref(), Some("abc"));

    let updated = update(
        &pool,
        &schedule.id,
        UpdateScheduleInput {
            notify_signal: Some(false),
            notify_telegram: Some(false),
            notify_only_on_change: Some(false),
            one_time: Some(true),
            ..Default::default()
        },
    )
    .await
    .expect("update");
    assert!(!updated.notify_signal);
    assert!(!updated.notify_telegram);
    assert!(!updated.notify_only_on_change);
    assert!(updated.one_time);
}

#[tokio::test]
async fn rejects_invalid_schedule() {
    let (_temp, pool) = setup().await;
    let mut value = input();
    value.name = " ".into();
    assert!(create(&pool, value).await.is_err());

    let mut value = input();
    value.router_policy = "unknown".into();
    assert!(create(&pool, value).await.is_err());
}
