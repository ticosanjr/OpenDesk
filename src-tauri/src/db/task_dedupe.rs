//! Task deduplication: stable fingerprint, active lookup and cooldown.

use super::tasks::{COLUMNS, Task};
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

pub const ORIGINS: [&str; 6] = [
    "MANUAL",
    "RECIPE",
    "IO_BUS",
    "CAPTURE",
    "ROUTINE",
    "DELEGATION",
];

pub fn validate_origin(value: &str) -> AppResult<String> {
    let normalized = value.trim().to_uppercase();
    if ORIGINS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid task origin '{value}'"
        )))
    }
}

fn normalize_title(value: &str) -> String {
    crate::dedupe::normalize_text(value)
}

#[allow(clippy::too_many_arguments)]
pub fn build_key(
    workspace_id: &str,
    title: &str,
    description: &str,
    mode: &str,
    agent: Option<&str>,
    model: Option<&str>,
    effort: &str,
    tags: &[String],
    origin: &str,
) -> String {
    let mut normalized_tags = tags
        .iter()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>();
    normalized_tags.sort();
    normalized_tags.dedup();
    let tags = normalized_tags.join("\n");
    crate::dedupe::fingerprint(&[
        workspace_id,
        origin,
        &normalize_title(title),
        &normalize_title(description),
        mode,
        agent.unwrap_or_default(),
        model.unwrap_or_default(),
        effort,
        &tags,
    ])
}

pub async fn active_by_key(
    pool: &SqlitePool,
    workspace_id: &str,
    dedupe_key: &str,
) -> AppResult<Option<Task>> {
    let query = format!(
        "SELECT {COLUMNS} FROM tasks
         WHERE workspace_id = ? AND dedupe_key = ?
           AND status IN ('TODO', 'IN_PROGRESS', 'IN_REVIEW')
         ORDER BY created_at DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, Task>(&query)
        .bind(workspace_id)
        .bind(dedupe_key)
        .fetch_optional(pool)
        .await?)
}

pub async fn recent_terminal_by_key(
    pool: &SqlitePool,
    workspace_id: &str,
    dedupe_key: &str,
) -> AppResult<Option<Task>> {
    let query = format!(
        "SELECT {COLUMNS} FROM tasks
         WHERE workspace_id = ? AND dedupe_key = ?
           AND status IN ('DONE', 'CANCELLED', 'BLOCKED')
         ORDER BY COALESCE(last_run_at, updated_at, created_at) DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, Task>(&query)
        .bind(workspace_id)
        .bind(dedupe_key)
        .fetch_optional(pool)
        .await?)
}

pub fn within_cooldown(task: &Task, minutes: i64) -> bool {
    if minutes <= 0 {
        return false;
    }
    let stamp = task
        .last_run_at
        .as_deref()
        .or(Some(task.updated_at.as_str()))
        .unwrap_or(task.created_at.as_str());
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(stamp) else {
        return false;
    };
    chrono::Utc::now()
        .signed_duration_since(parsed)
        .num_minutes()
        < minutes
}

pub fn as_deduplicated(mut task: Task) -> Task {
    task.deduplicated = true;
    task
}
