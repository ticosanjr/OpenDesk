use super::*;
use crate::{
    adapters::AdapterService,
    notifier::NoopNotifier,
    opencode_client::{binary::OpenCodeLocator, daemon_manager::DaemonManager},
    queue_ports::{NoopQueuePorts, NoopTaskDispatcher},
    queue_scheduler::QueueConfig,
};

async fn fixture(ctx: &crate::test_support::TestContext) -> (Arc<QueueManager>, Arc<CronEngine>) {
    let events = EventBus::new(32);
    let daemon = DaemonManager::new(
        ctx.paths.clone(),
        events.clone(),
        Arc::new(OpenCodeLocator::new(None)),
    );
    let queue = QueueManager::new(QueueConfig {
        pool: ctx.db.clone(),
        scrubber: Arc::new(opendesk_storage::secrets::SecretScrubber::new()),
        paths: ctx.paths.clone(),
        events: events.clone(),
        write_buffer: ctx.write_buffer.clone(),
        daemon: daemon.clone(),
        adapters: Arc::new(AdapterService::load(&ctx.db).await.expect("adapters")),
        ports: Arc::new(NoopQueuePorts),
        notifier: Arc::new(NoopNotifier),
        concurrency: 1,
    });
    let cron = CronEngine::new(
        ctx.db.clone(),
        ctx.paths.clone(),
        events,
        queue.clone(),
        daemon,
        Arc::new(NoopTaskDispatcher),
    );
    (queue, cron)
}

fn schedule() -> Schedule {
    Schedule {
        id: "sched-1".into(),
        workspace_id: "ws-1".into(),
        name: "Relatório Diário".into(),
        description: None,
        cron_expression: "0 8 * * 1-5".into(),
        timezone: "America/Sao_Paulo".into(),
        agent_name: None,
        command_or_prompt: "/relatorio".into(),
        trigger_type: "CRON".into(),
        trigger_config_json: "{}".into(),
        target_kind: "PROMPT".into(),
        recipe_id: None,
        router_policy: "COST_BENEFIT".into(),
        output_folder_template: "output/{YYYY}-{MM}/{schedule_slug}".into(),
        notify_email_recipients: None,
        notify_signal: false,
        notify_signal_recipients: None,
        notify_telegram: false,
        notify_telegram_recipients: None,
        notify_only_on_change: false,
        recurrence_json: None,
        one_time: false,
        last_output_hash: None,
        misfire_policy: "SKIP_TO_NEXT".into(),
        safe_readonly_mode: true,
        sync_to_google_docs: false,
        sync_to_google_calendar: false,
        is_active: true,
        last_run_at: None,
        last_run_status: None,
        next_run_at: None,
        created_at: String::new(),
    }
}

#[test]
fn next_run_uses_timezone_and_is_future() {
    let now = DateTime::parse_from_rfc3339("2026-11-06T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let next = next_occurrence("0 8 * * 1-5", "America/Sao_Paulo", now).expect("next");
    assert!(next > now);
    let tz: Tz = "America/Sao_Paulo".parse().expect("tz");
    assert_eq!(next.with_timezone(&tz).format("%H:%M").to_string(), "08:00");
}

#[test]
fn output_template_renders_and_rejects_traversal() {
    let now = DateTime::parse_from_rfc3339("2026-11-06T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let path = render_output_folder(&schedule(), Path::new("/tmp/ws"), now).expect("path");
    assert!(
        path.to_string_lossy()
            .ends_with("output/2026-11/relatorio-diario")
    );

    let mut bad = schedule();
    bad.output_folder_template = "../escape".into();
    assert!(render_output_folder(&bad, Path::new("/tmp/ws"), now).is_err());
}

#[test]
fn output_folder_renders_tokens_and_rejects_escapes() {
    use chrono::TimeZone;
    let mut value = schedule();
    value.output_folder_template = "output/{YYYY}-{MM}/{DD}/{schedule_slug}".into();
    value.name = "Relatório Diário".into();
    let now = chrono::Utc
        .with_ymd_and_hms(2026, 11, 8, 15, 30, 0)
        .unwrap();
    let root = Path::new("/tmp/ws");
    let rendered = render_output_folder(&value, root, now).expect("folder");
    assert_eq!(
        rendered,
        Path::new("/tmp/ws/output/2026-11/08/relatorio-diario")
    );

    value.output_folder_template = "../escape".into();
    assert!(render_output_folder(&value, root, now).is_err());
    value.output_folder_template = "/tmp/outside".into();
    assert!(render_output_folder(&value, root, now).is_err());
    value.output_folder_template = "/tmp/ws/inside".into();
    assert_eq!(
        render_output_folder(&value, root, now).expect("inside"),
        Path::new("/tmp/ws/inside")
    );
}

#[tokio::test]
async fn tick_skips_offline_and_misfired_schedules() {
    let ctx = crate::test_support::context().await;
    let (_queue, cron) = fixture(&ctx).await;
    let input: opendesk_storage::repos::schedules::CreateScheduleInput =
        serde_json::from_value(serde_json::json!({
            "workspace_id": ctx.workspace_id,
            "name": "Rotina offline",
            "command_or_prompt": "Resuma",
            "cron_expression": "0 8 * * *",
            "timezone": "UTC",
            "trigger_type": "CRON",
            "target_kind": "PROMPT"
        }))
        .expect("input");
    let schedule = opendesk_storage::repos::schedules::create(&ctx.db, input)
        .await
        .expect("schedule");
    opendesk_storage::repos::schedules::set_active(&ctx.db, &schedule.id, true)
        .await
        .expect("active");

    let recent_past = (Utc::now() - chrono::Duration::seconds(10)).to_rfc3339();
    opendesk_storage::repos::schedules::set_next_run(&ctx.db, &schedule.id, Some(&recent_past))
        .await
        .expect("next");
    let tick = cron.tick_once().await.expect("tick offline");
    assert_eq!(tick.enqueued, 0);
    let after = opendesk_storage::repos::schedules::get(&ctx.db, &schedule.id)
        .await
        .expect("after");
    assert_eq!(after.last_run_status.as_deref(), Some("SKIPPED_OFFLINE"));

    let old_past = (Utc::now() - chrono::Duration::seconds(120)).to_rfc3339();
    opendesk_storage::repos::schedules::set_next_run(&ctx.db, &schedule.id, Some(&old_past))
        .await
        .expect("next old");
    let tick = cron.tick_once().await.expect("tick misfire");
    assert_eq!(tick.enqueued, 0);
    let after = opendesk_storage::repos::schedules::get(&ctx.db, &schedule.id)
        .await
        .expect("after misfire");
    assert_eq!(after.last_run_status.as_deref(), Some("SKIPPED_MISFIRE"));
}

#[tokio::test]
async fn run_now_enqueues_and_queue_controls_work() {
    let ctx = crate::test_support::context().await;
    let (queue, cron) = fixture(&ctx).await;
    let input: opendesk_storage::repos::schedules::CreateScheduleInput =
        serde_json::from_value(serde_json::json!({
            "workspace_id": ctx.workspace_id,
            "name": "Rotina de fila",
            "command_or_prompt": "Resuma o dia",
            "cron_expression": "0 8 * * *",
            "timezone": "UTC",
            "trigger_type": "CRON",
            "target_kind": "PROMPT"
        }))
        .expect("input");
    let schedule = opendesk_storage::repos::schedules::create(&ctx.db, input)
        .await
        .expect("schedule");

    let result = cron.run_now(&schedule.id).await.expect("run now");
    assert_eq!(result["kind"], "queue");
    let job_id = result["job"]["id"].as_str().expect("job id").to_string();

    let status = queue.status(Some(&ctx.workspace_id)).await.expect("status");
    assert_eq!(status.waiting, 1);
    assert!(!status.paused);

    let listed = queue.list(Some(&ctx.workspace_id)).await.expect("list");
    assert!(listed.iter().any(|job| job.id == job_id));

    let prioritized = queue.update_priority(&job_id, 4).await.expect("priority");
    assert_eq!(prioritized.priority, 4);

    queue.pause("manutenção").await;
    let paused = queue.status(None).await.expect("paused");
    assert!(paused.paused);
    assert_eq!(paused.pause_reason.as_deref(), Some("manutenção"));
    queue.resume().await.expect("resume");
    assert!(!queue.is_paused());

    let cancelled = queue.cancel(&job_id).await.expect("cancel");
    assert_eq!(cancelled.status, "CANCELLED");
    let retried = queue.retry(&job_id).await.expect("retry");
    assert_eq!(retried.status, "QUEUED");
    assert_eq!(retried.retry_count, 1);
    let _ = queue.cancel(&job_id).await.expect("cancel again");
}

#[test]
fn invalid_cron_and_timezone_are_rejected() {
    let now = chrono::Utc::now();
    assert!(next_occurrence("not-a-cron", "UTC", now).is_err());
    assert!(next_occurrence("0 8 * * *", "Marte/Olympus", now).is_err());
}
