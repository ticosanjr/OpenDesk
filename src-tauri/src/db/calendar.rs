use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CalendarSource {
    pub id: String,
    pub workspace_id: String,
    pub provider: String,
    pub external_id: Option<String>,
    pub name: String,
    pub color_hex: String,
    pub timezone: String,
    pub is_visible: bool,
    pub is_read_only: bool,
    pub sync_enabled: bool,
    pub sync_token: Option<String>,
    pub last_synced_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CalendarEvent {
    pub id: String,
    pub workspace_id: String,
    pub source_id: Option<String>,
    pub external_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_at: String,
    pub end_at: String,
    pub all_day: bool,
    pub timezone: String,
    pub status: String,
    pub color_hex: Option<String>,
    pub recurrence_rule: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEventInput {
    #[serde(default)]
    pub source_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    pub start_at: String,
    pub end_at: String,
    #[serde(default)]
    pub all_day: bool,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub color_hex: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateEventInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub all_day: Option<bool>,
    pub timezone: Option<String>,
    pub status: Option<String>,
    pub color_hex: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExternalEventInput {
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_at: String,
    pub end_at: String,
    pub all_day: bool,
    pub timezone: String,
    pub status: String,
    pub color_hex: Option<String>,
}

const SOURCE_COLUMNS: &str = "id, workspace_id, provider, external_id, name, color_hex, timezone, \
    is_visible, is_read_only, sync_enabled, sync_token, last_synced_at, created_at";

const EVENT_COLUMNS: &str = "id, workspace_id, source_id, external_id, title, description, \
    location, start_at, end_at, all_day, timezone, status, color_hex, recurrence_rule, \
    created_at, updated_at";

/// Every workspace has one local calendar. Imported providers get additional
/// sources in Phase 4.
pub async fn ensure_local_source(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<CalendarSource> {
    let query = format!(
        "SELECT {SOURCE_COLUMNS} FROM calendar_sources
         WHERE workspace_id = ? AND provider = 'LOCAL' ORDER BY created_at ASC LIMIT 1"
    );
    if let Some(source) = sqlx::query_as::<_, CalendarSource>(&query)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?
    {
        return Ok(source);
    }
    let id = Uuid::now_v7().to_string();
    // Concurrent requests (agenda, events and sources load together in the UI)
    // must not fail because two of them created the local calendar at once.
    sqlx::query(
        "INSERT OR IGNORE INTO calendar_sources (
            id, workspace_id, provider, external_id, name, color_hex, timezone
         ) VALUES (?, ?, 'LOCAL', 'local', 'Meu calendário', '#3b82f6', 'UTC')",
    )
    .bind(&id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    let query = format!(
        "SELECT {SOURCE_COLUMNS} FROM calendar_sources
         WHERE workspace_id = ? AND provider = 'LOCAL' ORDER BY created_at ASC LIMIT 1"
    );
    sqlx::query_as::<_, CalendarSource>(&query)
        .bind(workspace_id)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

pub async fn get_source(pool: &SqlitePool, id: &str) -> AppResult<CalendarSource> {
    let query = format!("SELECT {SOURCE_COLUMNS} FROM calendar_sources WHERE id = ?");
    sqlx::query_as::<_, CalendarSource>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("calendar source {id}")))
}

pub async fn list_sources(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<CalendarSource>> {
    let query = format!(
        "SELECT {SOURCE_COLUMNS} FROM calendar_sources
         WHERE workspace_id = ? ORDER BY provider = 'LOCAL' DESC, name COLLATE NOCASE"
    );
    Ok(sqlx::query_as::<_, CalendarSource>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get_source_by_external(
    pool: &SqlitePool,
    workspace_id: &str,
    external_id: &str,
) -> AppResult<Option<CalendarSource>> {
    let query = format!(
        "SELECT {SOURCE_COLUMNS} FROM calendar_sources
         WHERE workspace_id = ? AND provider = 'GOOGLE' AND external_id = ?"
    );
    Ok(sqlx::query_as::<_, CalendarSource>(&query)
        .bind(workspace_id)
        .bind(external_id)
        .fetch_optional(pool)
        .await?)
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_google_source(
    pool: &SqlitePool,
    workspace_id: &str,
    external_id: &str,
    name: &str,
    color_hex: Option<&str>,
    timezone: &str,
) -> AppResult<CalendarSource> {
    if let Some(existing) = get_source_by_external(pool, workspace_id, external_id).await? {
        sqlx::query(
            "UPDATE calendar_sources SET name = ?, color_hex = ?, timezone = ?, sync_enabled = 1
             WHERE id = ?",
        )
        .bind(name)
        .bind(color_hex.unwrap_or(&existing.color_hex))
        .bind(timezone)
        .bind(&existing.id)
        .execute(pool)
        .await?;
        return get_source(pool, &existing.id).await;
    }
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO calendar_sources (
            id, workspace_id, provider, external_id, name, color_hex, timezone,
            is_read_only, sync_enabled
         ) VALUES (?, ?, 'GOOGLE', ?, ?, ?, ?, 0, 1)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(external_id)
    .bind(name)
    .bind(color_hex.unwrap_or("#3b82f6"))
    .bind(timezone)
    .execute(pool)
    .await?;
    get_source(pool, &id).await
}

pub async fn update_source_options(
    pool: &SqlitePool,
    id: &str,
    is_visible: Option<bool>,
    sync_enabled: Option<bool>,
) -> AppResult<CalendarSource> {
    let current = get_source(pool, id).await?;
    sqlx::query("UPDATE calendar_sources SET is_visible = ?, sync_enabled = ? WHERE id = ?")
        .bind(is_visible.unwrap_or(current.is_visible))
        .bind(sync_enabled.unwrap_or(current.sync_enabled))
        .bind(id)
        .execute(pool)
        .await?;
    get_source(pool, id).await
}

pub async fn set_sync_state(
    pool: &SqlitePool,
    id: &str,
    sync_token: Option<&str>,
    last_synced_at: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE calendar_sources SET sync_token = ?, last_synced_at = COALESCE(?, last_synced_at)
         WHERE id = ?",
    )
    .bind(sync_token)
    .bind(last_synced_at)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_source(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM calendar_sources WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("calendar source {id}")));
    }
    Ok(())
}

pub async fn get_event(pool: &SqlitePool, id: &str) -> AppResult<CalendarEvent> {
    let query = format!("SELECT {EVENT_COLUMNS} FROM calendar_events WHERE id = ?");
    sqlx::query_as::<_, CalendarEvent>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("calendar event {id}")))
}

pub async fn list_events_between(
    pool: &SqlitePool,
    workspace_id: &str,
    from: &str,
    to: &str,
    limit: i64,
) -> AppResult<Vec<CalendarEvent>> {
    let query = format!(
        "SELECT {EVENT_COLUMNS} FROM calendar_events
         WHERE workspace_id = ? AND status != 'CANCELLED'
           AND start_at < ? AND end_at > ?
         ORDER BY start_at ASC LIMIT ?"
    );
    Ok(sqlx::query_as::<_, CalendarEvent>(&query)
        .bind(workspace_id)
        .bind(to)
        .bind(from)
        .bind(limit.clamp(1, 1000))
        .fetch_all(pool)
        .await?)
}

#[allow(clippy::too_many_arguments)]
pub async fn create_event(
    pool: &SqlitePool,
    workspace_id: &str,
    source_id: &str,
    input: CreateEventInput,
    start_at: &str,
    end_at: &str,
    timezone: &str,
    status: &str,
) -> AppResult<CalendarEvent> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO calendar_events (
            id, workspace_id, source_id, title, description, location, start_at, end_at,
            all_day, timezone, status, color_hex
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(source_id)
    .bind(input.title.trim())
    .bind(input.description.as_deref())
    .bind(input.location.as_deref())
    .bind(start_at)
    .bind(end_at)
    .bind(input.all_day)
    .bind(timezone)
    .bind(status)
    .bind(input.color_hex.as_deref())
    .execute(pool)
    .await?;
    get_event(pool, &id).await
}

pub async fn update_event(
    pool: &SqlitePool,
    id: &str,
    input: UpdateEventInput,
    start_at: &str,
    end_at: &str,
    timezone: &str,
    status: &str,
) -> AppResult<CalendarEvent> {
    let current = get_event(pool, id).await?;
    sqlx::query(
        "UPDATE calendar_events SET title = ?, description = ?, location = ?, start_at = ?,
            end_at = ?, all_day = ?, timezone = ?, status = ?, color_hex = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(input.title.unwrap_or(current.title))
    .bind(input.description.or(current.description))
    .bind(input.location.or(current.location))
    .bind(start_at)
    .bind(end_at)
    .bind(input.all_day.unwrap_or(current.all_day))
    .bind(timezone)
    .bind(status)
    .bind(input.color_hex.or(current.color_hex))
    .bind(id)
    .execute(pool)
    .await?;
    get_event(pool, id).await
}

pub async fn upsert_external_event(
    pool: &SqlitePool,
    workspace_id: &str,
    source_id: &str,
    input: &ExternalEventInput,
) -> AppResult<CalendarEvent> {
    if let Some(existing) = sqlx::query_as::<_, CalendarEvent>(&format!(
        "SELECT {EVENT_COLUMNS} FROM calendar_events WHERE source_id = ? AND external_id = ?"
    ))
    .bind(source_id)
    .bind(&input.external_id)
    .fetch_optional(pool)
    .await?
    {
        sqlx::query(
            "UPDATE calendar_events SET title = ?, description = ?, location = ?, start_at = ?,
                end_at = ?, all_day = ?, timezone = ?, status = ?, color_hex = ?,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?",
        )
        .bind(&input.title)
        .bind(input.description.as_deref())
        .bind(input.location.as_deref())
        .bind(&input.start_at)
        .bind(&input.end_at)
        .bind(input.all_day)
        .bind(&input.timezone)
        .bind(&input.status)
        .bind(input.color_hex.as_deref())
        .bind(&existing.id)
        .execute(pool)
        .await?;
        return get_event(pool, &existing.id).await;
    }

    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO calendar_events (
            id, workspace_id, source_id, external_id, title, description, location,
            start_at, end_at, all_day, timezone, status, color_hex
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(source_id)
    .bind(&input.external_id)
    .bind(&input.title)
    .bind(input.description.as_deref())
    .bind(input.location.as_deref())
    .bind(&input.start_at)
    .bind(&input.end_at)
    .bind(input.all_day)
    .bind(&input.timezone)
    .bind(&input.status)
    .bind(input.color_hex.as_deref())
    .execute(pool)
    .await?;
    get_event(pool, &id).await
}

pub async fn mark_external_event_cancelled(
    pool: &SqlitePool,
    source_id: &str,
    external_id: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE calendar_events SET status = 'CANCELLED',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE source_id = ? AND external_id = ?",
    )
    .bind(source_id)
    .bind(external_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_all_sources(pool: &SqlitePool) -> AppResult<Vec<CalendarSource>> {
    let query = format!(
        "SELECT {SOURCE_COLUMNS} FROM calendar_sources
         WHERE provider = 'GOOGLE' AND sync_enabled = 1 ORDER BY workspace_id, name COLLATE NOCASE"
    );
    Ok(sqlx::query_as::<_, CalendarSource>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn list_events_starting_between(
    pool: &SqlitePool,
    from: &str,
    to: &str,
) -> AppResult<Vec<CalendarEvent>> {
    let query = format!(
        "SELECT {EVENT_COLUMNS} FROM calendar_events
         WHERE status != 'CANCELLED' AND start_at >= ? AND start_at <= ?
         ORDER BY start_at ASC LIMIT 1000"
    );
    Ok(sqlx::query_as::<_, CalendarEvent>(&query)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await?)
}

/// Returns true only for the first claim of a recipe/event occurrence.
pub async fn claim_trigger_fire(
    pool: &SqlitePool,
    recipe_id: &str,
    event_id: &str,
    occurrence_start: &str,
) -> AppResult<bool> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO calendar_trigger_fires (recipe_id, event_id, occurrence_start)
         VALUES (?, ?, ?)",
    )
    .bind(recipe_id)
    .bind(event_id)
    .bind(occurrence_start)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn delete_event(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM calendar_events WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("calendar event {id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
