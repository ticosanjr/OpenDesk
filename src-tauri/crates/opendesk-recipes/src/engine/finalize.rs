use crate::{
    context::RecipeContext,
    error::{AppError, AppResult},
    store,
};
use opendesk_storage::events::OpenDeskWsEvent;
use opendesk_storage::repos::tasks;

use chrono::Utc;

pub(crate) fn replace_attachment(source: &str, targets: &[String]) -> AppResult<()> {
    let target = targets
        .first()
        .ok_or_else(|| AppError::BadRequest("checkpoint sem arquivo de destino".into()))?;
    let bytes = std::fs::read(source)?;
    if let Some(parent) = std::path::Path::new(target).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(target, bytes)?;
    Ok(())
}

pub(crate) async fn finish(
    context: &RecipeContext,
    run: &store::RecipeRun,
    status: &str,
    message: &str,
    routine_id: Option<&str>,
) -> AppResult<()> {
    let _ = store::fail_running_nodes(&context.pool, &run.id, message).await;
    store::update_run(
        &context.pool,
        &run.id,
        status,
        None,
        None,
        Some(message),
        true,
    )
    .await?;
    if let Some(task_id) = &run.task_id {
        let _ =
            tasks::set_phase_status(&context.pool, task_id, Some("failed"), Some("BLOCKED")).await;
        let _ = tasks::set_blocked_reason(&context.pool, task_id, Some(message)).await;
    }
    if let Some(routine_id) = routine_id {
        mark_routine_by_id(context, routine_id, status).await;
    }
    context.events.publish(OpenDeskWsEvent::RecipeRunUpdated {
        run_id: run.id.clone(),
        recipe_id: run.recipe_id.clone(),
        workspace_id: run.workspace_id.clone(),
        status: status.to_string(),
    });
    Ok(())
}

pub(crate) async fn mark_routine(
    context: &RecipeContext,
    trigger: &serde_json::Value,
    status: &str,
) {
    if let Some(routine_id) = trigger
        .get("routine_id")
        .or_else(|| trigger.get("schedule_id"))
        .and_then(serde_json::Value::as_str)
    {
        mark_routine_by_id(context, routine_id, status).await;
    }
}

async fn mark_routine_by_id(context: &RecipeContext, routine_id: &str, status: &str) {
    if let Ok(schedule) = opendesk_storage::repos::schedules::get(&context.pool, routine_id).await {
        let _ = opendesk_storage::repos::schedules::mark_run(
            &context.pool,
            &schedule.id,
            status,
            &Utc::now().to_rfc3339(),
        )
        .await;
        context.events.publish(OpenDeskWsEvent::ScheduleUpdated {
            schedule_id: schedule.id,
            workspace_id: schedule.workspace_id,
        });
    }
}
