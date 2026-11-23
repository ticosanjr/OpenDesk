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

pub(super) const SOURCE_COLUMNS: &str = "id, workspace_id, provider, external_id, name, color_hex, timezone, \
    is_visible, is_read_only, sync_enabled, sync_token, last_synced_at, created_at";

pub(super) const EVENT_COLUMNS: &str = "id, workspace_id, source_id, external_id, title, description, \
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
