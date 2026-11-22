use super::task_dedupe;
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
    pub preferred_harness: Option<String>,
    #[sqlx(default)]
    pub agent_override: bool,
    #[sqlx(default)]
    pub model_override: bool,
    pub effort_level: String,
    pub care_level: String,
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
    pub held: bool,
    pub created_at: String,
    pub updated_at: String,
    /// True when `create` returned an existing equivalent task instead of
    /// inserting a new one. Not persisted.
    #[sqlx(default)]
    pub deduplicated: bool,
    /// Latest active queue job status, selected by `list`/`get`.
    #[sqlx(default)]
    pub active_job_status: Option<String>,
    /// Latest run status, selected by `list`/`get`.
    #[sqlx(default)]
    pub active_run_status: Option<String>,
    /// Derived workflow state used by every UI surface.
    #[sqlx(default)]
    pub workflow_state: String,
    /// Code explaining why the task is in `workflow_state`.
    #[sqlx(default)]
    pub workflow_reason: Option<String>,
}

impl Task {
    pub fn tags(&self) -> Vec<String> {
        serde_json::from_str(&self.tags_json).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskInput {
    #[serde(default)]
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
    pub preferred_harness: Option<String>,
    #[serde(default)]
    pub effort_level: Option<String>,
    #[serde(default)]
    pub care_level: Option<String>,
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
    pub preferred_harness: Option<String>,
    pub effort_level: Option<String>,
    pub care_level: Option<String>,
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
    pub clear_harness: bool,
    #[serde(default)]
    pub clear_plan: bool,
}

pub(crate) const COLUMNS: &str = "id, workspace_id, title, description, status, phase, phase_status, \
    priority, execution_mode, assigned_agent, preferred_model, preferred_harness, agent_override, model_override, \
    effort_level, care_level, plan_markdown, output_dir_path, notify_email, export_to_gdocs, tags_json, \
    position_index, last_run_id, last_run_status, last_run_at, blocked_reason, cancelled_at, \
    forked_from_task_id, origin, parent_task_id, dedupe_key, held, created_at, updated_at";

/// Extra columns that let the backend derive a single workflow state for the
/// UI instead of letting each screen guess from `status` and `phase_status`.
const ACTIVE_COLUMNS: &str = "    (SELECT j.status FROM execution_queue j      WHERE j.task_id = tasks.id AND j.status IN ('QUEUED', 'RUNNING', 'PAUSED')      ORDER BY j.enqueued_at DESC LIMIT 1) AS active_job_status,     (SELECT r.status FROM task_runs r WHERE r.task_id = tasks.id      ORDER BY r.started_at DESC LIMIT 1) AS active_run_status";

fn derive_workflow(task: &Task) -> (&'static str, Option<&'static str>) {
    let phase = task.phase_status.as_deref();
    let job = task.active_job_status.as_deref();
    match task.status.as_str() {
        "CANCELLED" => ("CANCELLED", None),
        "DONE" => ("DONE", None),
        "BLOCKED" => (
            "BLOCKED",
            Some(if phase == Some("failed") {
                "failed"
            } else {
                "manual_block"
            }),
        ),
        "IN_PROGRESS" => {
            if job == Some("RUNNING") || phase == Some("running") {
                ("RUNNING", None)
            } else {
                // The card was moved to In progress without a real job/run.
                ("BLOCKED", Some("in_progress_without_run"))
            }
        }
        "IN_REVIEW" => {
            if phase == Some("awaiting_approval") {
                ("NEEDS_APPROVAL", Some("approval"))
            } else if phase == Some("awaiting_permission") {
                ("WAITING_INPUT", Some("permission"))
            } else {
                ("WAITING_INPUT", Some("hitl"))
            }
        }
        "TODO" => {
            if phase == Some("awaiting_approval") {
                ("NEEDS_APPROVAL", Some("approval"))
            } else if task.held || phase == Some("held") {
                ("HELD", Some("held"))
            } else if matches!(job, Some("QUEUED") | Some("PAUSED"))
                || matches!(phase, Some("queued") | Some("follow_up_queued"))
            {
                ("QUEUED", Some("queued"))
            } else {
                ("READY", Some("ready"))
            }
        }
        _ => ("READY", None),
    }
}

fn decorate(mut task: Task) -> Task {
    let (state, reason) = derive_workflow(&task);
    task.workflow_state = state.to_string();
    task.workflow_reason = reason.map(str::to_string);
    task
}

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

/// Title is optional. When absent, use the first non-empty line of the
/// description, collapsed and capped so cards stay readable.
pub fn derive_title(description: &str) -> String {
    let line = description
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut title: String = collapsed.chars().take(80).collect();
    if collapsed.chars().count() > 80 {
        title.push('…');
    }
    if title.is_empty() {
        "Nova tarefa".to_string()
    } else {
        title
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
fn validate_care(value: &str) -> AppResult<String> {
    let value = value.trim().to_lowercase();
    match value.as_str() {
        "quick" | "standard" | "careful" => Ok(value),
        other => Err(AppError::BadRequest(format!(
            "invalid care_level '{other}'; expected quick, standard or careful"
        ))),
    }
}

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<Task>> {
    let query = format!(
        "SELECT {COLUMNS}, {ACTIVE_COLUMNS} FROM tasks WHERE workspace_id = ?
         ORDER BY CASE status
            WHEN 'TODO' THEN 1 WHEN 'IN_PROGRESS' THEN 2
            WHEN 'IN_REVIEW' THEN 3 WHEN 'BLOCKED' THEN 4 WHEN 'DONE' THEN 5
            WHEN 'CANCELLED' THEN 6 ELSE 7 END,
            position_index ASC, created_at ASC"
    );
    let tasks = sqlx::query_as::<_, Task>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
    Ok(tasks.into_iter().map(decorate).collect())
}

/// Tasks created directly by the user (Kanban/Fila). Routines, recipes and
/// other automations only appear in Executions.
pub async fn list_work(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<Task>> {
    let query = format!(
        "SELECT {COLUMNS}, {ACTIVE_COLUMNS} FROM tasks
         WHERE workspace_id = ? AND origin IN ('MANUAL', 'CAPTURE')
         ORDER BY CASE status
            WHEN 'TODO' THEN 1 WHEN 'IN_PROGRESS' THEN 2
            WHEN 'IN_REVIEW' THEN 3 WHEN 'BLOCKED' THEN 4 WHEN 'DONE' THEN 5
            WHEN 'CANCELLED' THEN 6 ELSE 7 END,
            position_index ASC, created_at ASC"
    );
    let tasks = sqlx::query_as::<_, Task>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
    Ok(tasks.into_iter().map(decorate).collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    let query = format!("SELECT {COLUMNS}, {ACTIVE_COLUMNS} FROM tasks WHERE tasks.id = ?");
    let task = sqlx::query_as::<_, Task>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {id}")))?;
    Ok(decorate(task))
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
    let description = input.description.trim();
    if description.is_empty() {
        return Err(AppError::BadRequest("task description is required".into()));
    }
    let title = if input.title.trim().is_empty() {
        derive_title(description)
    } else {
        input.title.trim().to_string()
    };

    let mode = validate_mode(input.execution_mode.as_deref().unwrap_or("AUTO_DSH"))?;
    let effort = validate_effort(input.effort_level.as_deref().unwrap_or("medium"))?;
    let care = validate_care(input.care_level.as_deref().unwrap_or("standard"))?;
    let priority = input.priority.unwrap_or(2).clamp(1, 4);
    let origin = task_dedupe::validate_origin(input.origin.as_deref().unwrap_or("MANUAL"))?;
    let tags_json = serde_json::to_string(&input.tags).unwrap_or_else(|_| "[]".into());
    let settings = crate::dedupe::get_settings(pool).await?;
    let dedupe_enabled = input.dedupe != Some(false) && settings.enabled;
    let dedupe_key = dedupe_enabled.then(|| {
        task_dedupe::build_key(
            workspace_id,
            &title,
            description,
            &mode,
            input.assigned_agent.as_deref(),
            input.preferred_model.as_deref(),
            &effort,
            &care,
            &input.tags,
            &origin,
        )
    });

    if let Some(key) = dedupe_key.as_deref() {
        if let Some(existing) = task_dedupe::active_by_key(pool, workspace_id, key).await? {
            return Ok(task_dedupe::as_deduplicated(existing));
        }
        if let Some(existing) = task_dedupe::recent_terminal_by_key(pool, workspace_id, key).await?
            && task_dedupe::within_cooldown(&existing, settings.cooldown_minutes)
        {
            return Ok(task_dedupe::as_deduplicated(existing));
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
            execution_mode, assigned_agent, preferred_model, preferred_harness, agent_override, model_override,
            effort_level, care_level, notify_email, export_to_gdocs, tags_json, position_index,
            origin, parent_task_id, dedupe_key
         ) VALUES (?, ?, ?, ?, 'TODO', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(title)
    .bind(input.description.trim())
    .bind(priority)
    .bind(&mode)
    .bind(input.assigned_agent.as_deref())
    .bind(input.preferred_model.as_deref())
    .bind(input.preferred_harness.as_deref())
    .bind(input.assigned_agent.is_some())
    .bind(input.preferred_model.is_some())
    .bind(&effort)
    .bind(&care)
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
            && let Some(existing) = task_dedupe::active_by_key(pool, workspace_id, key).await?
        {
            return Ok(task_dedupe::as_deduplicated(existing));
        }
        return Err(error.into());
    }

    get(pool, &id).await
}
