
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

pub async fn release_trigger_fire(
    pool: &SqlitePool,
    recipe_id: &str,
    event_id: &str,
    occurrence_start: &str,
) -> AppResult<()> {
    sqlx::query(
        "DELETE FROM calendar_trigger_fires
         WHERE recipe_id = ? AND event_id = ? AND occurrence_start = ?",
    )
    .bind(recipe_id)
    .bind(event_id)
    .bind(occurrence_start)
    .execute(pool)
    .await?;
    Ok(())
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
