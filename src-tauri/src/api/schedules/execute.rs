use crate::{
    app_state::AppState,
    db::schedules::{self, Schedule},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use chrono::Utc;
use std::sync::Arc;

/// Execute a routine because an inbound source fired.
pub(crate) async fn execute_schedule_from_source(
    state: &Arc<AppState>,
    schedule: &Schedule,
    event: &crate::io_bus::NormalizedTaskEvent,
) -> AppResult<Option<crate::db::tasks::Task>> {
    if schedule.target_kind == "RECIPE" {
        let recipe_id = schedule
            .recipe_id
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("rotina sem receita vinculada".into()))?;
        let yaml =
            crate::recipes::store::yaml_for(&state.db, &schedule.workspace_id, recipe_id).await?;
        let recipe = crate::recipes::parser::parse(&yaml)?;
        let trigger = serde_json::json!({
            "routine_id": schedule.id,
            "routine_name": schedule.name,
            "source_id": event.source_id,
            "external_id": event.external_id,
            "title": event.title,
            "body": event.body_markdown,
            "attachments": event.attachments,
            "metadata": event.metadata,
        });
        let run = match crate::recipes::engine::start(
            crate::app_state::recipe_context(state),
            &schedule.workspace_id,
            recipe,
            trigger,
            false,
        )
        .await
        {
            Ok(run) => run,
            Err(error) => {
                let _ = schedules::mark_run(
                    &state.db,
                    &schedule.id,
                    "FAILED",
                    &Utc::now().to_rfc3339(),
                )
                .await;
                return Err(error.into());
            }
        };
        if let Some(task_id) = run.task_id {
            return Ok(Some(crate::db::tasks::get(&state.db, &task_id).await?));
        }
        return Ok(None);
    }
    state.cron.enqueue_now(schedule).await?;
    let _ = schedules::mark_run(&state.db, &schedule.id, "QUEUED", &Utc::now().to_rfc3339()).await;
    state.events.publish(OpenDeskWsEvent::ScheduleUpdated {
        schedule_id: schedule.id.clone(),
        workspace_id: schedule.workspace_id.clone(),
    });
    state.cron.wake();
    Ok(None)
}

/// Signal messages route to SIGNAL_MSG routines before falling back to chat.
pub(crate) async fn execute_signal_routines(
    state: &Arc<AppState>,
    workspace_id: &str,
    source_number: &str,
    text: &str,
) -> AppResult<bool> {
    let routines = schedules::list_by_trigger(&state.db, workspace_id, "SIGNAL_MSG").await?;
    for schedule in routines {
        let config = serde_json::from_str::<serde_json::Value>(&schedule.trigger_config_json)
            .unwrap_or_else(|_| serde_json::json!({}));
        let filter = config
            .get("from_filter")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim();
        if !filter.is_empty() && !source_number.contains(filter) {
            continue;
        }
        if schedule.target_kind == "RECIPE" {
            let Some(recipe_id) = schedule.recipe_id.as_deref() else {
                continue;
            };
            let yaml =
                crate::recipes::store::yaml_for(&state.db, &schedule.workspace_id, recipe_id)
                    .await?;
            let recipe = crate::recipes::parser::parse(&yaml)?;
            let trigger = serde_json::json!({
                "routine_id": schedule.id,
                "routine_name": schedule.name,
                "signal_source": source_number,
                "text": text,
            });
            if let Err(error) = crate::recipes::engine::start(
                crate::app_state::recipe_context(state),
                &schedule.workspace_id,
                recipe,
                trigger,
                false,
            )
            .await
            {
                let _ = schedules::mark_run(
                    &state.db,
                    &schedule.id,
                    "FAILED",
                    &Utc::now().to_rfc3339(),
                )
                .await;
                return Err(error.into());
            }
        } else {
            state.cron.enqueue_now(&schedule).await?;
        }
        let _ =
            schedules::mark_run(&state.db, &schedule.id, "QUEUED", &Utc::now().to_rfc3339()).await;
        return Ok(true);
    }

    // A recipe can also subscribe to Signal directly via a SIGNAL_MSG trigger.
    for index in crate::recipes::store::list_all(&state.db).await? {
        if !index.is_enabled || index.workspace_id != workspace_id {
            continue;
        }
        let Ok(yaml) =
            crate::recipes::store::yaml_for(&state.db, &index.workspace_id, &index.id).await
        else {
            continue;
        };
        let Ok(recipe) = crate::recipes::parser::parse(&yaml) else {
            continue;
        };
        for trigger in recipe.triggers() {
            if trigger.trigger_type != "SIGNAL_MSG" {
                continue;
            }
            let filter = trigger
                .config
                .get("from_filter")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim();
            if !filter.is_empty() && !source_number.contains(filter) {
                continue;
            }
            let context = serde_json::json!({
                "signal_source": source_number,
                "text": text,
                "recipe_id": recipe.id,
            });
            crate::recipes::engine::start(
                crate::app_state::recipe_context(state),
                &index.workspace_id,
                recipe.clone(),
                context,
                false,
            )
            .await?;
            return Ok(true);
        }
    }
    Ok(false)
}
