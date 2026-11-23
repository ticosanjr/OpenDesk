use super::*;
use sqlx::SqlitePool;
use tempfile::TempDir;

async fn setup() -> (TempDir, SqlitePool) {
    let temp = TempDir::new().expect("temp");
    let pool = crate::db::init(&temp.path().join("calendar.db"))
        .await
        .expect("db");
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
        .bind(temp.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .expect("workspace");
    (temp, pool)
}

#[tokio::test]
async fn creates_one_local_source_per_workspace() {
    let (_temp, pool) = setup().await;
    let first = ensure_local_source(&pool, "ws-1").await.expect("first");
    let second = ensure_local_source(&pool, "ws-1").await.expect("second");
    assert_eq!(first.id, second.id);
    assert_eq!(first.provider, "LOCAL");
    assert_eq!(list_sources(&pool, "ws-1").await.expect("sources").len(), 1);
}

#[tokio::test]
async fn local_source_creation_is_idempotent_under_concurrency() {
    let (_temp, pool) = setup().await;
    let (first, second) = tokio::join!(
        ensure_local_source(&pool, "ws-1"),
        ensure_local_source(&pool, "ws-1")
    );
    let first = first.expect("first");
    let second = second.expect("second");
    assert_eq!(first.id, second.id);
    assert_eq!(list_sources(&pool, "ws-1").await.expect("sources").len(), 1);
}

#[tokio::test]
async fn creates_lists_updates_and_deletes_events() {
    let (_temp, pool) = setup().await;
    let source = ensure_local_source(&pool, "ws-1").await.expect("source");
    let event = create_event(
        &pool,
        "ws-1",
        &source.id,
        CreateEventInput {
            source_id: None,
            title: "Planejamento".into(),
            description: Some("semana".into()),
            location: None,
            start_at: "2026-11-10T12:00:00Z".into(),
            end_at: "2026-11-10T13:00:00Z".into(),
            all_day: false,
            timezone: Some("UTC".into()),
            status: None,
            color_hex: None,
        },
        "2026-11-10T12:00:00Z",
        "2026-11-10T13:00:00Z",
        "UTC",
        "CONFIRMED",
    )
    .await
    .expect("event");
    let listed = list_events_between(
        &pool,
        "ws-1",
        "2026-11-10T00:00:00Z",
        "2026-11-11T00:00:00Z",
        10,
    )
    .await
    .expect("list");
    assert_eq!(listed.len(), 1);
    let updated = update_event(
        &pool,
        &event.id,
        UpdateEventInput {
            title: Some("Planejamento v2".into()),
            ..Default::default()
        },
        "2026-11-10T12:00:00Z",
        "2026-11-10T14:00:00Z",
        "UTC",
        "CONFIRMED",
    )
    .await
    .expect("update");
    assert_eq!(updated.title, "Planejamento v2");
    assert_eq!(updated.end_at, "2026-11-10T14:00:00Z");
    delete_event(&pool, &event.id).await.expect("delete");
    assert!(get_event(&pool, &event.id).await.is_err());
}

#[tokio::test]
async fn trigger_claims_are_idempotent() {
    let (_temp, pool) = setup().await;
    assert!(
        claim_trigger_fire(&pool, "recipe-1", "event-1", "2026-11-10T12:00:00Z")
            .await
            .expect("first")
    );
    assert!(
        !claim_trigger_fire(&pool, "recipe-1", "event-1", "2026-11-10T12:00:00Z")
            .await
            .expect("second")
    );
    release_trigger_fire(&pool, "recipe-1", "event-1", "2026-11-10T12:00:00Z")
        .await
        .expect("release");
    assert!(
        claim_trigger_fire(&pool, "recipe-1", "event-1", "2026-11-10T12:00:00Z")
            .await
            .expect("retry")
    );
}

#[tokio::test]
async fn upserts_google_sources_and_events_idempotently() {
    let (_temp, pool) = setup().await;
    let source = upsert_google_source(
        &pool,
        "ws-1",
        "team@example.com",
        "Time",
        Some("#ff0000"),
        "America/Sao_Paulo",
    )
    .await
    .expect("source");
    let again = upsert_google_source(
        &pool,
        "ws-1",
        "team@example.com",
        "Time v2",
        Some("#00ff00"),
        "America/Sao_Paulo",
    )
    .await
    .expect("again");
    assert_eq!(source.id, again.id);
    assert_eq!(again.name, "Time v2");

    let input = ExternalEventInput {
        external_id: "evt-1".into(),
        title: "Reunião".into(),
        description: None,
        location: None,
        start_at: "2026-11-10T12:00:00Z".into(),
        end_at: "2026-11-10T13:00:00Z".into(),
        all_day: false,
        timezone: "America/Sao_Paulo".into(),
        status: "CONFIRMED".into(),
        color_hex: Some("#ff0000".into()),
    };
    let first = upsert_external_event(&pool, "ws-1", &source.id, &input)
        .await
        .expect("event");
    let second = upsert_external_event(
        &pool,
        "ws-1",
        &source.id,
        &ExternalEventInput {
            title: "Reunião v2".into(),
            ..input.clone()
        },
    )
    .await
    .expect("event 2");
    assert_eq!(first.id, second.id);
    assert_eq!(second.title, "Reunião v2");
    assert!(delete_source(&pool, &source.id).await.is_ok());
}
