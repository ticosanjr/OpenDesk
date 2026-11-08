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
    pub trigger_type: String,
    pub trigger_config_json: String,
    pub target_kind: String,
    pub recipe_id: Option<String>,
    pub router_policy: String,
    pub output_folder_template: String,
    pub notify_email_recipients: Option<String>,
    pub notify_signal: bool,
    pub notify_signal_recipients: Option<String>,
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
    pub trigger_type: Option<String>,
    pub trigger_config_json: Option<String>,
    pub target_kind: Option<String>,
    pub recipe_id: Option<String>,
    pub router_policy: Option<String>,
    pub output_folder_template: Option<String>,
    pub notify_email_recipients: Option<String>,
    pub notify_signal: Option<bool>,
    pub notify_signal_recipients: Option<String>,
    pub notify_only_on_change: Option<bool>,
    pub recurrence_json: Option<String>,
    pub one_time: Option<bool>,
    pub misfire_policy: Option<String>,
    pub safe_readonly_mode: Option<bool>,
    pub sync_to_google_docs: Option<bool>,
    pub sync_to_google_calendar: Option<bool>,
    pub is_active: Option<bool>,
}

fn default_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into())
}

fn default_policy() -> String {
    "COST_BENEFIT".into()
}

fn default_trigger() -> String {
    "CRON".into()
}

fn default_target() -> String {
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

fn default_output_template() -> String {
    "output/{YYYY}-{MM}/{schedule_slug}".into()
}

fn default_true() -> bool {
    true
}

fn default_misfire_policy() -> String {
    "SKIP_TO_NEXT".into()
}

const COLUMNS: &str = "id, workspace_id, name, description, cron_expression, timezone, \
    agent_name, command_or_prompt, trigger_type, trigger_config_json, target_kind, recipe_id, \
    router_policy, output_folder_template, \
    notify_email_recipients, notify_signal, notify_signal_recipients, notify_only_on_change, \
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

pub async fn create(pool: &SqlitePool, input: CreateScheduleInput) -> AppResult<Schedule> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("schedule name is required".into()));
    }
    let trigger_type = input.trigger_type.trim().to_uppercase();
    if !TRIGGER_TYPES.contains(&trigger_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "invalid trigger_type '{}'",
            input.trigger_type
        )));
    }
    let target_kind = input.target_kind.trim().to_uppercase();
    if !TARGET_KINDS.contains(&target_kind.as_str()) {
        return Err(AppError::BadRequest(format!(
            "invalid target_kind '{}'",
            input.target_kind
        )));
    }
    if target_kind == "RECIPE" {
        if input
            .recipe_id
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            return Err(AppError::BadRequest(
                "recipe_id is required when target_kind is RECIPE".into(),
            ));
        }
    } else if input.command_or_prompt.trim().is_empty() {
        return Err(AppError::BadRequest("command_or_prompt is required".into()));
    }
    if trigger_type == "CRON" && input.cron_expression.trim().is_empty() {
        return Err(AppError::BadRequest(
            "cron_expression is required for CRON routines".into(),
        ));
    }
    let misfire_policy = input.misfire_policy.trim().to_uppercase();
    if !matches!(misfire_policy.as_str(), "RUN_IMMEDIATELY" | "SKIP_TO_NEXT") {
        return Err(AppError::BadRequest(format!(
            "invalid misfire_policy '{}'",
            input.misfire_policy
        )));
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
            agent_name, command_or_prompt, trigger_type, trigger_config_json, target_kind, recipe_id,
            router_policy, output_folder_template,
            notify_email_recipients, notify_signal, notify_signal_recipients, notify_only_on_change,
            recurrence_json, one_time, misfire_policy, safe_readonly_mode,
            sync_to_google_docs, sync_to_google_calendar, is_active
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(input.name.trim())
    .bind(input.description.as_deref())
    .bind(input.cron_expression.trim())
    .bind(input.timezone.trim())
    .bind(input.agent_name.as_deref())
    .bind(input.command_or_prompt.trim())
    .bind(&trigger_type)
    .bind(
        input
            .trigger_config_json
            .as_deref()
            .unwrap_or("{}"),
    )
    .bind(&target_kind)
    .bind(input.recipe_id.as_deref())
    .bind(&policy)
    .bind(input.output_folder_template.trim())
    .bind(input.notify_email_recipients.as_deref())
    .bind(input.notify_signal)
    .bind(input.notify_signal_recipients.as_deref())
    .bind(input.notify_only_on_change)
    .bind(input.recurrence_json.as_deref())
    .bind(input.one_time)
    .bind(input.misfire_policy.to_uppercase().as_str())
    .bind(input.safe_readonly_mode)
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
            command_or_prompt = ?, trigger_type = ?, trigger_config_json = ?, target_kind = ?,
            recipe_id = ?, router_policy = ?, output_folder_template = ?,
            notify_email_recipients = ?, notify_signal = ?, notify_signal_recipients = ?,
            notify_only_on_change = ?, recurrence_json = ?, one_time = ?,
            misfire_policy = ?, safe_readonly_mode = ?,
            sync_to_google_docs = ?, sync_to_google_calendar = ?, is_active = ?
         WHERE id = ?",
    )
    .bind(input.name.unwrap_or(current.name))
    .bind(input.description.or(current.description))
    .bind(input.cron_expression.unwrap_or(current.cron_expression))
    .bind(input.timezone.unwrap_or(current.timezone))
    .bind(input.agent_name.or(current.agent_name))
    .bind(input.command_or_prompt.unwrap_or(current.command_or_prompt))
    .bind(
        input
            .trigger_type
            .map(|value| value.trim().to_uppercase())
            .unwrap_or(current.trigger_type),
    )
    .bind(
        input
            .trigger_config_json
            .unwrap_or(current.trigger_config_json),
    )
    .bind(
        input
            .target_kind
            .map(|value| value.trim().to_uppercase())
            .unwrap_or(current.target_kind),
    )
    .bind(input.recipe_id.or(current.recipe_id))
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
    .bind(input.notify_signal.unwrap_or(current.notify_signal))
    .bind(
        input
            .notify_signal_recipients
            .or(current.notify_signal_recipients),
    )
    .bind(
        input
            .notify_only_on_change
            .unwrap_or(current.notify_only_on_change),
    )
    .bind(input.recurrence_json.or(current.recurrence_json))
    .bind(input.one_time.unwrap_or(current.one_time))
    .bind(
        input
            .misfire_policy
            .map(|value| value.to_uppercase())
            .unwrap_or(current.misfire_policy),
    )
    .bind(
        input
            .safe_readonly_mode
            .unwrap_or(current.safe_readonly_mode),
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

pub async fn set_last_output_hash(
    pool: &SqlitePool,
    id: &str,
    hash: Option<&str>,
) -> AppResult<()> {
    sqlx::query("UPDATE schedules SET last_output_hash = ? WHERE id = ?")
        .bind(hash)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("schedules.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    fn input() -> CreateScheduleInput {
        CreateScheduleInput {
            workspace_id: "ws-1".into(),
            name: "Diário".into(),
            description: Some("Resumo".into()),
            cron_expression: "0 8 * * *".into(),
            timezone: "America/Sao_Paulo".into(),
            agent_name: Some("build".into()),
            command_or_prompt: "Produza o resumo".into(),
            trigger_type: "CRON".into(),
            trigger_config_json: None,
            target_kind: "PROMPT".into(),
            recipe_id: None,
            router_policy: "cost_benefit".into(),
            output_folder_template: "output/{YYYY}-{MM}/{schedule_slug}".into(),
            notify_email_recipients: Some("ops@example.com".into()),
            notify_signal: false,
            notify_signal_recipients: None,
            notify_only_on_change: false,
            recurrence_json: None,
            one_time: false,
            misfire_policy: "SKIP_TO_NEXT".into(),
            safe_readonly_mode: true,
            sync_to_google_docs: true,
            sync_to_google_calendar: false,
            is_active: true,
        }
    }

    #[tokio::test]
    async fn schedule_lifecycle() {
        let (_temp, pool) = setup().await;
        let schedule = create(&pool, input()).await.expect("create");
        assert_eq!(schedule.router_policy, "COST_BENEFIT");
        assert!(schedule.is_active);
        assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 1);
        assert_eq!(active(&pool).await.expect("active").len(), 1);

        let updated = update(
            &pool,
            &schedule.id,
            UpdateScheduleInput {
                name: Some("Diário v2".into()),
                sync_to_google_docs: Some(false),
                ..Default::default()
            },
        )
        .await
        .expect("update");
        assert_eq!(updated.name, "Diário v2");
        assert!(!updated.sync_to_google_docs);

        set_next_run(&pool, &schedule.id, Some("2026-11-07T08:00:00Z"))
            .await
            .expect("next");
        mark_run(&pool, &schedule.id, "SUCCESS", "2026-11-06T08:00:00Z")
            .await
            .expect("mark");
        let after = get(&pool, &schedule.id).await.expect("get");
        assert_eq!(after.next_run_at.as_deref(), Some("2026-11-07T08:00:00Z"));
        assert_eq!(after.last_run_status.as_deref(), Some("SUCCESS"));

        let inactive = set_active(&pool, &schedule.id, false)
            .await
            .expect("active");
        assert!(!inactive.is_active);
        assert!(active(&pool).await.expect("active list").is_empty());

        delete(&pool, &schedule.id).await.expect("delete");
        assert!(get(&pool, &schedule.id).await.is_err());
    }

    #[tokio::test]
    async fn persists_recurrence_and_notification_settings() {
        let (_temp, pool) = setup().await;
        let mut value = input();
        value.recurrence_json =
            Some(r#"{"frequency":"weekly","interval":1,"time":"09:30","weekdays":[1,3]}"#.into());
        value.notify_signal = true;
        value.notify_signal_recipients = Some("+5511999999999".into());
        value.notify_only_on_change = true;
        value.one_time = false;
        let schedule = create(&pool, value).await.expect("create");

        let loaded = get(&pool, &schedule.id).await.expect("get");
        assert!(loaded.notify_signal);
        assert_eq!(
            loaded.notify_signal_recipients.as_deref(),
            Some("+5511999999999")
        );
        assert!(loaded.notify_only_on_change);
        assert_eq!(
            loaded.recurrence_json.as_deref(),
            Some(r#"{"frequency":"weekly","interval":1,"time":"09:30","weekdays":[1,3]}"#)
        );
        assert!(!loaded.one_time);
        assert!(loaded.last_output_hash.is_none());

        set_last_output_hash(&pool, &schedule.id, Some("abc"))
            .await
            .expect("hash");
        let loaded = get(&pool, &schedule.id).await.expect("get");
        assert_eq!(loaded.last_output_hash.as_deref(), Some("abc"));

        let updated = update(
            &pool,
            &schedule.id,
            UpdateScheduleInput {
                notify_signal: Some(false),
                notify_only_on_change: Some(false),
                one_time: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("update");
        assert!(!updated.notify_signal);
        assert!(!updated.notify_only_on_change);
        assert!(updated.one_time);
    }

    #[tokio::test]
    async fn rejects_invalid_schedule() {
        let (_temp, pool) = setup().await;
        let mut value = input();
        value.name = " ".into();
        assert!(create(&pool, value).await.is_err());

        let mut value = input();
        value.router_policy = "unknown".into();
        assert!(create(&pool, value).await.is_err());
    }
}
