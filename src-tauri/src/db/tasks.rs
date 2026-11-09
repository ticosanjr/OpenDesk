use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub phase: Option<String>,
    pub phase_status: Option<String>,
    pub priority: i64,
    pub execution_mode: String,
    pub assigned_agent: Option<String>,
    pub preferred_model: Option<String>,
    #[sqlx(default)]
    pub agent_override: bool,
    #[sqlx(default)]
    pub model_override: bool,
    pub effort_level: String,
    pub plan_markdown: Option<String>,
    pub output_dir_path: Option<String>,
    pub notify_email: Option<String>,
    pub export_to_gdocs: bool,
    pub tags_json: String,
    pub position_index: i64,
    pub last_run_id: Option<String>,
    pub last_run_status: Option<String>,
    pub forked_from_task_id: Option<String>,
    pub last_run_at: Option<String>,
    pub blocked_reason: Option<String>,
    pub cancelled_at: Option<String>,
    pub origin: String,
    pub parent_task_id: Option<String>,
    pub dedupe_key: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// True when `create` returned an existing equivalent task instead of
    /// inserting a new one. Not persisted.
    #[sqlx(default)]
    pub deduplicated: bool,
}

impl Task {
    pub fn tags(&self) -> Vec<String> {
        serde_json::from_str(&self.tags_json).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskInput {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub execution_mode: Option<String>,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub assigned_agent: Option<String>,
    #[serde(default)]
    pub preferred_model: Option<String>,
    #[serde(default)]
    pub effort_level: Option<String>,
    #[serde(default)]
    pub notify_email: Option<String>,
    #[serde(default)]
    pub export_to_gdocs: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub parent_task_id: Option<String>,
    /// Internal callers can opt out for forks/swarms.
    #[serde(default)]
    pub dedupe: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateTaskInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub phase: Option<String>,
    pub phase_status: Option<String>,
    pub priority: Option<i64>,
    pub execution_mode: Option<String>,
    pub assigned_agent: Option<String>,
    pub preferred_model: Option<String>,
    pub effort_level: Option<String>,
    pub plan_markdown: Option<String>,
    pub output_dir_path: Option<String>,
    pub notify_email: Option<String>,
    pub export_to_gdocs: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub position_index: Option<i64>,
    #[serde(default)]
    pub agent_override: Option<bool>,
    #[serde(default)]
    pub model_override: Option<bool>,
    #[serde(default)]
    pub clear_agent: bool,
    #[serde(default)]
    pub clear_model: bool,
    #[serde(default)]
    pub clear_plan: bool,
}

const COLUMNS: &str = "id, workspace_id, title, description, status, phase, phase_status, \
    priority, execution_mode, assigned_agent, preferred_model, agent_override, model_override, \
    effort_level, plan_markdown, output_dir_path, notify_email, export_to_gdocs, tags_json, \
    position_index, last_run_id, last_run_status, last_run_at, blocked_reason, cancelled_at, \
    forked_from_task_id, origin, parent_task_id, dedupe_key, created_at, updated_at";

pub const STATUSES: [&str; 6] = [
    "TODO",
    "IN_PROGRESS",
    "IN_REVIEW",
    "DONE",
    "CANCELLED",
    "BLOCKED",
];
const MODES: [&str; 4] = ["AUTO_DSH", "PLAN_ONLY", "BUILD_DIRECT", "PLAN_THEN_BUILD"];
const EFFORTS: [&str; 3] = ["low", "medium", "high"];

fn validate_status(status: &str) -> AppResult<String> {
    let status = status.trim().to_uppercase();
    // Legacy clients and old queue jobs may still send BACKLOG. It no longer
    // exists as a workflow stage, so it is normalized to TODO.
    let status = if status == "BACKLOG" {
        "TODO".to_string()
    } else {
        status
    };
    if STATUSES.contains(&status.as_str()) {
        Ok(status)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid task status '{status}'"
        )))
    }
}

fn validate_phase(phase: &str) -> AppResult<String> {
    let phase = phase.trim().to_lowercase();
    if matches!(phase.as_str(), "plan" | "build" | "audit" | "follow_up") {
        Ok(phase)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid task phase '{phase}'"
        )))
    }
}

fn validate_mode(mode: &str) -> AppResult<String> {
    let mode = mode.trim().to_uppercase();
    if MODES.contains(&mode.as_str()) {
        Ok(mode)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid execution_mode '{mode}'"
        )))
    }
}

fn validate_effort(effort: &str) -> AppResult<String> {
    let effort = effort.trim().to_lowercase();
    if EFFORTS.contains(&effort.as_str()) {
        Ok(effort)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid effort_level '{effort}'"
        )))
    }
}

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<Task>> {
    let query = format!(
        "SELECT {COLUMNS} FROM tasks WHERE workspace_id = ?
         ORDER BY CASE status
            WHEN 'TODO' THEN 1 WHEN 'IN_PROGRESS' THEN 2
            WHEN 'IN_REVIEW' THEN 3 WHEN 'BLOCKED' THEN 4 WHEN 'DONE' THEN 5
            WHEN 'CANCELLED' THEN 6 ELSE 7 END,
            position_index ASC, created_at ASC"
    );
    Ok(sqlx::query_as::<_, Task>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    let query = format!("SELECT {COLUMNS} FROM tasks WHERE id = ?");
    sqlx::query_as::<_, Task>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {id}")))
}

pub const ORIGINS: [&str; 5] = ["MANUAL", "RECIPE", "IO_BUS", "CAPTURE", "ROUTINE"];

fn validate_origin(value: &str) -> AppResult<String> {
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
fn build_dedupe_key(
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

async fn active_by_key(
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

async fn recent_terminal_by_key(
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

fn within_cooldown(task: &Task, minutes: i64) -> bool {
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

fn as_deduplicated(mut task: Task) -> Task {
    task.deduplicated = true;
    task
}

pub async fn create(
    pool: &SqlitePool,
    workspace_id: &str,
    input: CreateTaskInput,
) -> AppResult<Task> {
    create_with_id(pool, workspace_id, None, input).await
}

pub async fn create_with_id(
    pool: &SqlitePool,
    workspace_id: &str,
    id: Option<&str>,
    input: CreateTaskInput,
) -> AppResult<Task> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest("task title is required".into()));
    }
    if input.description.trim().is_empty() {
        return Err(AppError::BadRequest("task description is required".into()));
    }

    let mode = validate_mode(input.execution_mode.as_deref().unwrap_or("AUTO_DSH"))?;
    let effort = validate_effort(input.effort_level.as_deref().unwrap_or("medium"))?;
    let priority = input.priority.unwrap_or(2).clamp(1, 4);
    let origin = validate_origin(input.origin.as_deref().unwrap_or("MANUAL"))?;
    let tags_json = serde_json::to_string(&input.tags).unwrap_or_else(|_| "[]".into());
    let settings = crate::dedupe::get_settings(pool).await?;
    let dedupe_enabled = input.dedupe != Some(false) && settings.enabled;
    let dedupe_key = dedupe_enabled.then(|| {
        build_dedupe_key(
            workspace_id,
            title,
            input.description.trim(),
            &mode,
            input.assigned_agent.as_deref(),
            input.preferred_model.as_deref(),
            &effort,
            &input.tags,
            &origin,
        )
    });

    if let Some(key) = dedupe_key.as_deref() {
        if let Some(existing) = active_by_key(pool, workspace_id, key).await? {
            return Ok(as_deduplicated(existing));
        }
        if let Some(existing) = recent_terminal_by_key(pool, workspace_id, key).await?
            && within_cooldown(&existing, settings.cooldown_minutes)
        {
            return Ok(as_deduplicated(existing));
        }
    }

    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position_index), -1) + 1 FROM tasks
         WHERE workspace_id = ? AND status = 'TODO'",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?;

    let id = id
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    let insert = sqlx::query(
        "INSERT INTO tasks (
            id, workspace_id, title, description, status, priority,
            execution_mode, assigned_agent, preferred_model, agent_override, model_override,
            effort_level, notify_email, export_to_gdocs, tags_json, position_index,
            origin, parent_task_id, dedupe_key
         ) VALUES (?, ?, ?, ?, 'TODO', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(title)
    .bind(input.description.trim())
    .bind(priority)
    .bind(&mode)
    .bind(input.assigned_agent.as_deref())
    .bind(input.preferred_model.as_deref())
    .bind(input.assigned_agent.is_some())
    .bind(input.preferred_model.is_some())
    .bind(&effort)
    .bind(input.notify_email.as_deref())
    .bind(input.export_to_gdocs)
    .bind(&tags_json)
    .bind(position)
    .bind(&origin)
    .bind(input.parent_task_id.as_deref())
    .bind(dedupe_key.as_deref())
    .execute(pool)
    .await;

    if let Err(error) = insert {
        if let sqlx::Error::Database(database) = &error
            && database.message().contains("UNIQUE constraint failed")
            && let Some(key) = dedupe_key.as_deref()
            && let Some(existing) = active_by_key(pool, workspace_id, key).await?
        {
            return Ok(as_deduplicated(existing));
        }
        return Err(error.into());
    }

    get(pool, &id).await
}

pub async fn update(pool: &SqlitePool, id: &str, input: UpdateTaskInput) -> AppResult<Task> {
    let current = get(pool, id).await?;

    let title = match input.title {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        Some(_) => return Err(AppError::BadRequest("task title cannot be empty".into())),
        None => current.title,
    };
    let status = match input.status {
        Some(value) => validate_status(&value)?,
        None => current.status,
    };
    let phase = match input.phase {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => Some(validate_phase(&value)?),
        None => current.phase,
    };
    let mode = match input.execution_mode {
        Some(value) => validate_mode(&value)?,
        None => current.execution_mode,
    };
    let effort = match input.effort_level {
        Some(value) => validate_effort(&value)?,
        None => current.effort_level,
    };
    let tags_json = match input.tags {
        Some(tags) => serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into()),
        None => current.tags_json,
    };

    sqlx::query(
        "UPDATE tasks SET
            title = ?, description = ?, status = ?, phase = ?, phase_status = ?,
            priority = ?, execution_mode = ?, assigned_agent = ?, preferred_model = ?,
            agent_override = ?, model_override = ?, effort_level = ?, plan_markdown = ?, output_dir_path = ?, notify_email = ?,
            export_to_gdocs = ?, tags_json = ?, position_index = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(&title)
    .bind(input.description.unwrap_or(current.description))
    .bind(&status)
    .bind(&phase)
    .bind(input.phase_status.or(current.phase_status))
    .bind(input.priority.unwrap_or(current.priority).clamp(1, 4))
    .bind(&mode)
    .bind(if input.clear_agent {
        None
    } else {
        input.assigned_agent.or(current.assigned_agent)
    })
    .bind(if input.clear_model {
        None
    } else {
        input.preferred_model.or(current.preferred_model)
    })
    .bind(if input.clear_agent {
        false
    } else {
        input.agent_override.unwrap_or(current.agent_override)
    })
    .bind(if input.clear_model {
        false
    } else {
        input.model_override.unwrap_or(current.model_override)
    })
    .bind(&effort)
    .bind(if input.clear_plan {
        None
    } else {
        input.plan_markdown.or(current.plan_markdown)
    })
    .bind(input.output_dir_path.or(current.output_dir_path))
    .bind(input.notify_email.or(current.notify_email))
    .bind(input.export_to_gdocs.unwrap_or(current.export_to_gdocs))
    .bind(&tags_json)
    .bind(input.position_index.unwrap_or(current.position_index))
    .bind(id)
    .execute(pool)
    .await?;

    // Keep the fingerprint aligned with the edited content. If another active
    // task already owns it, surface a conflict instead of silently duplicating.
    let updated = get(pool, id).await?;
    let dedupe_key = build_dedupe_key(
        &updated.workspace_id,
        &updated.title,
        &updated.description,
        &updated.execution_mode,
        updated.assigned_agent.as_deref(),
        updated.preferred_model.as_deref(),
        &updated.effort_level,
        &updated.tags(),
        &updated.origin,
    );
    if let Err(error) = sqlx::query("UPDATE tasks SET dedupe_key = ? WHERE id = ?")
        .bind(&dedupe_key)
        .bind(id)
        .execute(pool)
        .await
    {
        if let sqlx::Error::Database(database) = &error
            && database.message().contains("UNIQUE constraint failed")
        {
            return Err(AppError::Conflict(
                "já existe uma tarefa ativa com os mesmos parâmetros".into(),
            ));
        }
        return Err(error.into());
    }

    get(pool, id).await
}

pub async fn move_to(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    position_index: Option<i64>,
) -> AppResult<Task> {
    let status = validate_status(status)?;
    let position = match position_index {
        Some(position) => position,
        None => {
            sqlx::query_scalar(
                "SELECT COALESCE(MAX(position_index), -1) + 1 FROM tasks
                 WHERE workspace_id = (SELECT workspace_id FROM tasks WHERE id = ?)
                   AND status = ?",
            )
            .bind(id)
            .bind(&status)
            .fetch_one(pool)
            .await?
        }
    };

    let result = sqlx::query(
        "UPDATE tasks SET status = ?, position_index = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(&status)
    .bind(position)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("task {id}")));
    }

    get(pool, id).await
}

pub async fn fork(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    fork_with_model(pool, id, None).await
}

/// Fork a task, optionally pinning the copy to a specific model. Used by the
/// comparative A/B swarm so both cards stay independent.
pub async fn fork_with_model(pool: &SqlitePool, id: &str, model: Option<&str>) -> AppResult<Task> {
    let source = get(pool, id).await?;
    let new_id = Uuid::now_v7().to_string();
    let suffix = model
        .map(|model| {
            let short = model
                .split('#')
                .next()
                .unwrap_or(model)
                .rsplit('/')
                .next()
                .unwrap_or(model);
            format!(" · {short}")
        })
        .unwrap_or_default();
    sqlx::query(
        "INSERT INTO tasks (
            id, workspace_id, title, description, status, phase, phase_status, priority,
            execution_mode, assigned_agent, preferred_model, agent_override, model_override,
            effort_level, plan_markdown, notify_email, export_to_gdocs, tags_json,
            position_index, forked_from_task_id, origin
         ) VALUES (?, ?, ?, ?, 'TODO', NULL, 'forked', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'MANUAL')",
    )
    .bind(&new_id)
    .bind(&source.workspace_id)
    .bind(format!("{} (fork{suffix})", source.title))
    .bind(&source.description)
    .bind(source.priority)
    .bind(&source.execution_mode)
    .bind(&source.assigned_agent)
    .bind(model.map(str::to_string).or(source.preferred_model.clone()))
    .bind(source.agent_override)
    .bind(model.is_some() || source.model_override)
    .bind(&source.effort_level)
    .bind(&source.plan_markdown)
    .bind(&source.notify_email)
    .bind(source.export_to_gdocs)
    .bind(&source.tags_json)
    .bind(source.position_index)
    .bind(&source.id)
    .execute(pool)
    .await?;
    get(pool, &new_id).await
}

pub async fn set_plan(pool: &SqlitePool, id: &str, plan_markdown: &str) -> AppResult<Task> {
    sqlx::query(
        "UPDATE tasks SET plan_markdown = ?, status = 'TODO', phase = 'plan',
            phase_status = 'awaiting_approval',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(plan_markdown)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_phase_status(
    pool: &SqlitePool,
    id: &str,
    phase_status: Option<&str>,
    status: Option<&str>,
) -> AppResult<()> {
    let status = match status {
        Some(value) => Some(validate_status(value)?),
        None => None,
    };
    sqlx::query(
        "UPDATE tasks SET
            phase_status = COALESCE(?, phase_status),
            status = COALESCE(?, status),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(phase_status)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Replace (not merge) the phase status. Used by explicit workflow
/// transitions such as moving a card back to Backlog or to Done.
pub async fn replace_phase_status(
    pool: &SqlitePool,
    id: &str,
    phase_status: Option<&str>,
    status: Option<&str>,
) -> AppResult<Task> {
    let status = match status {
        Some(value) => Some(validate_status(value)?),
        None => None,
    };
    sqlx::query(
        "UPDATE tasks SET
            phase_status = ?,
            status = COALESCE(?, status),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(phase_status)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_phase(pool: &SqlitePool, id: &str, phase: Option<&str>) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET phase = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(phase)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_last_run(
    pool: &SqlitePool,
    id: &str,
    run_id: Option<&str>,
    status: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET last_run_id = ?, last_run_status = ?,
            last_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(run_id)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_blocked_reason(
    pool: &SqlitePool,
    id: &str,
    reason: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET blocked_reason = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(reason)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_cancelled(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    sqlx::query(
        "UPDATE tasks SET status = 'CANCELLED', phase_status = 'cancelled',
            cancelled_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_output_dir(pool: &SqlitePool, id: &str, output_dir: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET output_dir_path = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(output_dir)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("task {id}")));
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn count_artifacts(pool: &SqlitePool, task_id: &str) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM artifacts_provenance WHERE task_id = ?")
            .bind(task_id)
            .fetch_one(pool)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("tasks.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    fn input() -> CreateTaskInput {
        CreateTaskInput {
            title: "Relatório".into(),
            description: "Produza o relatório".into(),
            execution_mode: Some("PLAN_THEN_BUILD".into()),
            priority: Some(9),
            assigned_agent: Some("build".into()),
            preferred_model: None,
            effort_level: Some("high".into()),
            notify_email: Some("ops@example.com".into()),
            export_to_gdocs: true,
            tags: vec!["financeiro".into(), "mensal".into()],
            origin: None,
            parent_task_id: None,
            dedupe: None,
        }
    }

    #[tokio::test]
    async fn fork_can_pin_a_model() {
        let (_temp, pool) = setup().await;
        let task = create(&pool, "ws-1", input()).await.expect("create");
        let fork = fork_with_model(&pool, &task.id, Some("opencode-go/glm-5.3"))
            .await
            .expect("fork");
        assert_eq!(fork.forked_from_task_id.as_deref(), Some(task.id.as_str()));
        assert_eq!(fork.preferred_model.as_deref(), Some("opencode-go/glm-5.3"));
        assert!(fork.model_override);
        assert!(fork.title.contains("glm-5.3"));
    }

    #[tokio::test]
    async fn deduplicates_active_and_recent_tasks() {
        let (_temp, pool) = setup().await;
        let first = create(&pool, "ws-1", input()).await.expect("first");
        let duplicate = create(&pool, "ws-1", input()).await.expect("duplicate");
        assert!(duplicate.deduplicated);
        assert_eq!(duplicate.id, first.id);

        move_to(&pool, &first.id, "DONE", None).await.expect("done");
        let cooldown = create(&pool, "ws-1", input()).await.expect("cooldown");
        assert!(cooldown.deduplicated);
        assert_eq!(cooldown.id, first.id);

        crate::dedupe::set_settings(
            &pool,
            crate::dedupe::DedupeSettings {
                enabled: true,
                cooldown_minutes: 0,
            },
        )
        .await
        .expect("settings");
        let fresh = create(&pool, "ws-1", input()).await.expect("fresh");
        assert!(!fresh.deduplicated);
        assert_ne!(fresh.id, first.id);

        let mut different = input();
        different.description = "Outro escopo".into();
        let other = create(&pool, "ws-1", different).await.expect("other");
        assert!(!other.deduplicated);
        assert_ne!(other.id, fresh.id);
    }

    #[tokio::test]
    async fn forced_creation_bypasses_dedupe() {
        let (_temp, pool) = setup().await;
        let first = create(&pool, "ws-1", input()).await.expect("first");
        let mut forced = input();
        forced.dedupe = Some(false);
        let second = create(&pool, "ws-1", forced).await.expect("forced");
        assert!(!second.deduplicated);
        assert_ne!(second.id, first.id);
    }

    #[tokio::test]
    async fn task_lifecycle() {
        let (_temp, pool) = setup().await;
        let task = create(&pool, "ws-1", input()).await.expect("create");
        assert_eq!(task.status, "TODO");
        assert_eq!(task.priority, 4);
        assert_eq!(task.execution_mode, "PLAN_THEN_BUILD");
        assert_eq!(task.tags(), vec!["financeiro", "mensal"]);
        assert!(task.export_to_gdocs);

        let updated = update(
            &pool,
            &task.id,
            UpdateTaskInput {
                title: Some("Relatório v2".into()),
                priority: Some(1),
                assigned_agent: Some("explore".into()),
                preferred_model: Some("opencode-go/glm-5.3#high".into()),
                ..Default::default()
            },
        )
        .await
        .expect("update");
        assert_eq!(updated.title, "Relatório v2");
        assert_eq!(updated.priority, 1);
        assert_eq!(updated.assigned_agent.as_deref(), Some("explore"));
        assert_eq!(
            updated.preferred_model.as_deref(),
            Some("opencode-go/glm-5.3#high")
        );

        let moved = move_to(&pool, &task.id, "IN_PROGRESS", Some(2))
            .await
            .expect("move");
        assert_eq!(moved.status, "IN_PROGRESS");
        assert_eq!(moved.position_index, 2);

        assert!(
            set_plan(&pool, &task.id, "# Plano")
                .await
                .expect("plan")
                .plan_markdown
                .is_some()
        );
        set_phase_status(&pool, &task.id, Some("running"), Some("IN_PROGRESS"))
            .await
            .expect("phase");
        assert_eq!(
            get(&pool, &task.id)
                .await
                .expect("get")
                .phase_status
                .as_deref(),
            Some("running")
        );

        set_output_dir(&pool, &task.id, "/tmp/output")
            .await
            .expect("output");
        assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 1);
        delete(&pool, &task.id).await.expect("delete");
        assert!(get(&pool, &task.id).await.is_err());
    }

    #[tokio::test]
    async fn rejects_invalid_task_input() {
        let (_temp, pool) = setup().await;
        let mut value = input();
        value.title = "  ".into();
        assert!(create(&pool, "ws-1", value).await.is_err());

        let mut value = input();
        value.execution_mode = Some("nonsense".into());
        assert!(create(&pool, "ws-1", value).await.is_err());

        let mut value = input();
        value.effort_level = Some("extreme".into());
        assert!(create(&pool, "ws-1", value).await.is_err());
    }
}
