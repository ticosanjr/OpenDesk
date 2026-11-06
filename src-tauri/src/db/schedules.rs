use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Schedule {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
    pub cron_expression: String,
    pub timezone: String,
    pub agent_name: Option<String>,
    pub command_or_prompt: String,
    pub router_policy: String,
    pub output_folder_template: String,
    pub notify_email_recipients: Option<String>,
    pub sync_to_google_docs: bool,
    pub sync_to_google_calendar: bool,
    pub is_active: bool,
    pub last_run_at: Option<String>,
    pub last_run_status: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateScheduleInput {
    pub workspace_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub cron_expression: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default)]
    pub agent_name: Option<String>,
    pub command_or_prompt: String,
    #[serde(default = "default_policy")]
    pub router_policy: String,
    #[serde(default = "default_output_template")]
    pub output_folder_template: String,
    #[serde(default)]
    pub notify_email_recipients: Option<String>,
    #[serde(default)]
    pub sync_to_google_docs: bool,
    #[serde(default)]
    pub sync_to_google_calendar: bool,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateScheduleInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub cron_expression: Option<String>,
    pub timezone: Option<String>,
    pub agent_name: Option<String>,
    pub command_or_prompt: Option<String>,
    pub router_policy: Option<String>,
    pub output_folder_template: Option<String>,
    pub notify_email_recipients: Option<String>,
    pub sync_to_google_docs: Option<bool>,
    pub sync_to_google_calendar: Option<bool>,
    pub is_active: Option<bool>,
}

fn default_timezone() -> String {
    "UTC".into()
}

fn default_policy() -> String {
    "COST_BENEFIT".into()
}

fn default_output_template() -> String {
    "output/{YYYY}-{MM}/{schedule_slug}".into()
}

fn default_true() -> bool {
    true
}

const COLUMNS: &str = "id, workspace_id, name, description, cron_expression, timezone, \
    agent_name, command_or_prompt, router_policy, output_folder_template, \
    notify_email_recipients, sync_to_google_docs, sync_to_google_calendar, is_active, \
    last_run_at, last_run_status, next_run_at, created_at";

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<Schedule>> {
    let query =
        format!("SELECT {COLUMNS} FROM schedules WHERE workspace_id = ? ORDER BY created_at DESC");
    Ok(sqlx::query_as::<_, Schedule>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn active(pool: &SqlitePool) -> AppResult<Vec<Schedule>> {
    let query =
        format!("SELECT {COLUMNS} FROM schedules WHERE is_active = 1 ORDER BY next_run_at ASC");
    Ok(sqlx::query_as::<_, Schedule>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Schedule> {
    let query = format!("SELECT {COLUMNS} FROM schedules WHERE id = ?");
    sqlx::query_as::<_, Schedule>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("schedule {id}")))
}

pub async fn create(pool: &SqlitePool, input: CreateScheduleInput) -> AppResult<Schedule> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("schedule name is required".into()));
    }
    if input.command_or_prompt.trim().is_empty() {
        return Err(AppError::BadRequest("command_or_prompt is required".into()));
    }
    let policy = input.router_policy.to_uppercase();
    if !matches!(
        policy.as_str(),
        "COST_BENEFIT" | "CHEAPEST" | "BEST_QUALITY" | "MANUAL" | "CUSTOM_RULES"
    ) {
        return Err(AppError::BadRequest(format!(
            "invalid router_policy '{}'",
            input.router_policy
        )));
    }

    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO schedules (
            id, workspace_id, name, description, cron_expression, timezone,
            agent_name, command_or_prompt, router_policy, output_folder_template,
            notify_email_recipients, sync_to_google_docs, sync_to_google_calendar, is_active
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(input.name.trim())
    .bind(input.description.as_deref())
    .bind(input.cron_expression.trim())
    .bind(input.timezone.trim())
    .bind(input.agent_name.as_deref())
    .bind(input.command_or_prompt.trim())
    .bind(&policy)
    .bind(input.output_folder_template.trim())
    .bind(input.notify_email_recipients.as_deref())
    .bind(input.sync_to_google_docs)
    .bind(input.sync_to_google_calendar)
    .bind(input.is_active)
    .execute(pool)
    .await?;

    get(pool, &id).await
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    input: UpdateScheduleInput,
) -> AppResult<Schedule> {
    let current = get(pool, id).await?;
    let policy = input
        .router_policy
        .unwrap_or(current.router_policy)
        .to_uppercase();
    if !matches!(
        policy.as_str(),
        "COST_BENEFIT" | "CHEAPEST" | "BEST_QUALITY" | "MANUAL" | "CUSTOM_RULES"
    ) {
        return Err(AppError::BadRequest(format!(
            "invalid router_policy '{policy}'"
        )));
    }

    sqlx::query(
        "UPDATE schedules SET
            name = ?, description = ?, cron_expression = ?, timezone = ?, agent_name = ?,
            command_or_prompt = ?, router_policy = ?, output_folder_template = ?,
            notify_email_recipients = ?, sync_to_google_docs = ?,
            sync_to_google_calendar = ?, is_active = ?
         WHERE id = ?",
    )
    .bind(input.name.unwrap_or(current.name))
    .bind(input.description.or(current.description))
    .bind(input.cron_expression.unwrap_or(current.cron_expression))
    .bind(input.timezone.unwrap_or(current.timezone))
    .bind(input.agent_name.or(current.agent_name))
    .bind(input.command_or_prompt.unwrap_or(current.command_or_prompt))
    .bind(&policy)
    .bind(
        input
            .output_folder_template
            .unwrap_or(current.output_folder_template),
    )
    .bind(
        input
            .notify_email_recipients
            .or(current.notify_email_recipients),
    )
    .bind(
        input
            .sync_to_google_docs
            .unwrap_or(current.sync_to_google_docs),
    )
    .bind(
        input
            .sync_to_google_calendar
            .unwrap_or(current.sync_to_google_calendar),
    )
    .bind(input.is_active.unwrap_or(current.is_active))
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM schedules WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("schedule {id}")));
    }
    Ok(())
}

pub async fn set_next_run(pool: &SqlitePool, id: &str, next_run_at: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE schedules SET next_run_at = ? WHERE id = ?")
        .bind(next_run_at)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_run(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    last_run_at: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE schedules SET last_run_at = ?, last_run_status = ? WHERE id = ?")
        .bind(last_run_at)
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_active(pool: &SqlitePool, id: &str, active: bool) -> AppResult<Schedule> {
    sqlx::query("UPDATE schedules SET is_active = ? WHERE id = ?")
        .bind(active)
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}
