use super::model::*;
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;
use uuid::Uuid;

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
    // Accepted for API compatibility; new routines always start paused.
    let _requested_active = input.is_active;
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
            notify_email_recipients, notify_signal, notify_signal_recipients, notify_telegram,
            notify_telegram_recipients, notify_only_on_change,
            recurrence_json, one_time, misfire_policy, safe_readonly_mode,
            sync_to_google_docs, sync_to_google_calendar, is_active
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(input.notify_telegram)
    .bind(input.notify_telegram_recipients.as_deref())
    .bind(input.notify_only_on_change)
    .bind(input.recurrence_json.as_deref())
    .bind(input.one_time)
    .bind(input.misfire_policy.to_uppercase().as_str())
    .bind(input.safe_readonly_mode)
    .bind(input.sync_to_google_docs)
    .bind(input.sync_to_google_calendar)
    // Every routine starts paused. Enabling goes through an explicit
    // set_active call so agents never schedule new work silently.
    .bind(false)
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
            notify_telegram = ?, notify_telegram_recipients = ?,
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
    .bind(input.notify_telegram.unwrap_or(current.notify_telegram))
    .bind(
        input
            .notify_telegram_recipients
            .or(current.notify_telegram_recipients),
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

/// Rewrites the trigger config without touching the rest of the routine. Used
/// to replace a plaintext webhook secret with its `secret_ref`.
pub async fn set_trigger_config(pool: &SqlitePool, id: &str, config_json: &str) -> AppResult<()> {
    sqlx::query("UPDATE schedules SET trigger_config_json = ? WHERE id = ?")
        .bind(config_json)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
