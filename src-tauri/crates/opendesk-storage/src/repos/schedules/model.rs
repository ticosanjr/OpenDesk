use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

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
    pub trigger_type: String,
    pub trigger_config_json: String,
    pub target_kind: String,
    pub recipe_id: Option<String>,
    pub router_policy: String,
    pub output_folder_template: String,
    pub notify_email_recipients: Option<String>,
    pub notify_signal: bool,
    pub notify_signal_recipients: Option<String>,
    pub notify_telegram: bool,
    pub notify_telegram_recipients: Option<String>,
    pub notify_only_on_change: bool,
    pub recurrence_json: Option<String>,
    pub one_time: bool,
    pub last_output_hash: Option<String>,
    pub misfire_policy: String,
    pub safe_readonly_mode: bool,
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
    #[serde(default)]
    pub cron_expression: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default)]
    pub agent_name: Option<String>,
    pub command_or_prompt: String,
    #[serde(default = "default_trigger")]
    pub trigger_type: String,
    #[serde(default)]
    pub trigger_config_json: Option<String>,
    #[serde(default = "default_target")]
    pub target_kind: String,
    #[serde(default)]
    pub recipe_id: Option<String>,
    #[serde(default = "default_policy")]
    pub router_policy: String,
    #[serde(default = "default_output_template")]
    pub output_folder_template: String,
    #[serde(default)]
    pub notify_email_recipients: Option<String>,
    #[serde(default)]
    pub notify_signal: bool,
    #[serde(default)]
    pub notify_signal_recipients: Option<String>,
    #[serde(default)]
    pub notify_telegram: bool,
    #[serde(default)]
    pub notify_telegram_recipients: Option<String>,
    #[serde(default)]
    pub notify_only_on_change: bool,
    #[serde(default)]
    pub recurrence_json: Option<String>,
    #[serde(default)]
    pub one_time: bool,
    #[serde(default = "default_misfire_policy")]
    pub misfire_policy: String,
    #[serde(default = "default_true")]
    pub safe_readonly_mode: bool,
    #[serde(default)]
    pub sync_to_google_docs: bool,
    #[serde(default)]
    pub sync_to_google_calendar: bool,
    /// Kept for API compatibility, but `create` always stores `false`.
    /// Activation is an explicit action (`set_active`).
    #[serde(default)]
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
    pub trigger_type: Option<String>,
    pub trigger_config_json: Option<String>,
    pub target_kind: Option<String>,
    pub recipe_id: Option<String>,
    pub router_policy: Option<String>,
    pub output_folder_template: Option<String>,
    pub notify_email_recipients: Option<String>,
    pub notify_signal: Option<bool>,
    pub notify_signal_recipients: Option<String>,
    pub notify_telegram: Option<bool>,
    pub notify_telegram_recipients: Option<String>,
    pub notify_only_on_change: Option<bool>,
    pub recurrence_json: Option<String>,
    pub one_time: Option<bool>,
    pub misfire_policy: Option<String>,
    pub safe_readonly_mode: Option<bool>,
    pub sync_to_google_docs: Option<bool>,
    pub sync_to_google_calendar: Option<bool>,
    pub is_active: Option<bool>,
}

pub fn default_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into())
}

pub(super) fn default_policy() -> String {
    "COST_BENEFIT".into()
}

pub(super) fn default_trigger() -> String {
    "CRON".into()
}

pub(super) fn default_target() -> String {
    "PROMPT".into()
}

pub const TRIGGER_TYPES: [&str; 7] = [
    "CRON",
    "FOLDER_WATCH",
    "RSYNC_FOLDER",
    "IMAP_IDLE",
    "SIGNAL_MSG",
    "WEBHOOK",
    "SQL_POLL",
];

pub const TARGET_KINDS: [&str; 2] = ["PROMPT", "RECIPE"];

pub(super) fn default_output_template() -> String {
    "output/{YYYY}-{MM}/{schedule_slug}".into()
}

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_misfire_policy() -> String {
    "SKIP_TO_NEXT".into()
}

pub(super) const COLUMNS: &str = "id, workspace_id, name, description, cron_expression, timezone, \
    agent_name, command_or_prompt, trigger_type, trigger_config_json, target_kind, recipe_id, \
    router_policy, output_folder_template, \
    notify_email_recipients, notify_signal, notify_signal_recipients, notify_telegram, \
    notify_telegram_recipients, notify_only_on_change, \
    recurrence_json, one_time, last_output_hash, misfire_policy, safe_readonly_mode, \
    sync_to_google_docs, sync_to_google_calendar, is_active, last_run_at, last_run_status, \
    next_run_at, created_at";

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<Schedule>> {
    let query =
        format!("SELECT {COLUMNS} FROM schedules WHERE workspace_id = ? ORDER BY created_at DESC");
    Ok(sqlx::query_as::<_, Schedule>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn list_by_trigger(
    pool: &SqlitePool,
    workspace_id: &str,
    trigger_type: &str,
) -> AppResult<Vec<Schedule>> {
    let query = format!(
        "SELECT {COLUMNS} FROM schedules
         WHERE workspace_id = ? AND trigger_type = ? AND is_active = 1
         ORDER BY created_at"
    );
    Ok(sqlx::query_as::<_, Schedule>(&query)
        .bind(workspace_id)
        .bind(trigger_type.trim().to_uppercase())
        .fetch_all(pool)
        .await?)
}

pub async fn find_by_name_recipe(
    pool: &SqlitePool,
    workspace_id: &str,
    name: &str,
    recipe_id: &str,
) -> AppResult<Option<Schedule>> {
    let query = format!(
        "SELECT {COLUMNS} FROM schedules
         WHERE workspace_id = ? AND name = ? AND recipe_id = ?
         ORDER BY created_at LIMIT 1"
    );
    Ok(sqlx::query_as::<_, Schedule>(&query)
        .bind(workspace_id)
        .bind(name)
        .bind(recipe_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn find_by_recipe(
    pool: &SqlitePool,
    workspace_id: &str,
    recipe_id: &str,
) -> AppResult<Option<Schedule>> {
    let query = format!(
        "SELECT {COLUMNS} FROM schedules
         WHERE workspace_id = ? AND recipe_id = ?
         ORDER BY created_at LIMIT 1"
    );
    Ok(sqlx::query_as::<_, Schedule>(&query)
        .bind(workspace_id)
        .bind(recipe_id)
        .fetch_optional(pool)
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
