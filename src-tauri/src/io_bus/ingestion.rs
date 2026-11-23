use super::NormalizedTaskEvent;
use crate::{
    app_state::AppState,
    db::{
        self,
        io_bus::InboundSource,
        tasks::{self, CreateTaskInput},
    },
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use std::sync::Arc;
use uuid::Uuid;

/// Resolves the typed destination, falling back to `config_json` for rows
/// created before `target_kind`/`target_id` existed.
fn typed_target(source: &InboundSource) -> Option<(String, String)> {
    if source.target_kind != "TASK" {
        return source
            .target_id
            .clone()
            .map(|id| (source.target_kind.clone(), id));
    }
    let config = serde_json::from_str::<serde_json::Value>(&source.config_json).ok()?;
    for (key, kind) in [("schedule_id", "SCHEDULE"), ("recipe_id", "RECIPE")] {
        if let Some(id) = config
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some((kind.into(), id.to_string()));
        }
    }
    None
}

/// Convert any inbound signal into exactly one deterministic task.
pub async fn ingest(
    state: &Arc<AppState>,
    source: &InboundSource,
    event: NormalizedTaskEvent,
) -> AppResult<Option<tasks::Task>> {
    if let Some((kind, id)) = typed_target(source) {
        match kind.as_str() {
            // A routine can subscribe to this source. The routine decides
            // whether it runs a prompt or a recipe, so ingest does not create
            // a task in that case.
            "SCHEDULE" => {
                if let Ok(schedule) = crate::db::schedules::get(&state.db, &id).await {
                    return crate::api::schedules::execute_schedule_from_source(
                        state, &schedule, &event,
                    )
                    .await;
                }
            }
            // Sources can route their signal to a declarative recipe instead of
            // creating a standalone task. This is how IMAP/Signal/SQL/webhook
            // triggers reuse the recipe engine without duplicating adapters.
            "RECIPE" => {
                let yaml =
                    crate::recipes::store::yaml_for(&state.db, &source.workspace_id, &id).await?;
                let recipe = crate::recipes::parser::parse(&yaml)?;
                let trigger = serde_json::json!({
                    "source_id": source.id,
                    "external_id": event.external_id,
                    "title": event.title,
                    "body": event.body_markdown,
                    "attachments": event.attachments,
                    "metadata": event.metadata,
                });
                let run = crate::recipes::engine::start(
                    crate::app_state::recipe_context(state),
                    &source.workspace_id,
                    recipe,
                    trigger,
                    false,
                )
                .await?;
                if let Some(task_id) = run.task_id {
                    return Ok(Some(tasks::get(&state.db, &task_id).await?));
                }
                return Err(AppError::Internal(
                    "recipe acionada por I/O não criou task".into(),
                ));
            }
            "TICKET" => {
                crate::db::tickets::create(
                    &state.db,
                    crate::db::tickets::CreateTicketInput {
                        workspace_id: source.workspace_id.clone(),
                        source: format!("io_bus:{}", source.id),
                        source_ref: Some(event.external_id.clone()),
                        title: event.title.clone(),
                        body: event.body_markdown.clone(),
                        priority: event.priority,
                        assigned_agent: source.target_agent.clone(),
                        metadata: event.metadata.clone(),
                    },
                )
                .await?;
                return Ok(None);
            }
            _ => {}
        }
    }
    let seed = format!(
        "opendesk:io:{}:{}",
        source.id,
        if event.external_id.trim().is_empty() {
            format!("{}:{}", event.title, event.body_markdown)
        } else {
            event.external_id.clone()
        }
    );
    let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, seed.as_bytes()).to_string();
    if let Ok(existing) = tasks::get(&state.db, &id).await {
        return Ok(Some(existing));
    }

    let mut body = event.body_markdown.trim().to_string();
    if body.is_empty() {
        body = event.title.clone();
    }
    if !event.attachments.is_empty() {
        body.push_str("\n\n## Arquivos de entrada\n\n");
        for path in &event.attachments {
            body.push_str(&format!("- `{path}`\n"));
        }
    }
    let mut tags = event.tags.clone();
    tags.push("io-bus".into());
    tags.push(format!("io-source:{}", source.id));
    let task = tasks::create_with_id(
        &state.db,
        &source.workspace_id,
        Some(&id),
        CreateTaskInput {
            title: event.title.chars().take(180).collect(),
            description: body,
            execution_mode: Some("AUTO_DSH".into()),
            priority: event.priority.or(Some(2)),
            assigned_agent: source.target_agent.clone(),
            preferred_model: None,
            preferred_harness: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags,
            origin: Some("IO_BUS".into()),
            parent_task_id: None,
            dedupe: None,
        },
    )
    .await?;
    if task.deduplicated {
        let _ = db::io_bus::mark_inbound(&state.db, &source.id, "DEDUPLICATED", None).await;
        return Ok(Some(task));
    }

    if source.target_agent.is_some() {
        let _ = tasks::update(
            &state.db,
            &task.id,
            tasks::UpdateTaskInput {
                agent_override: Some(true),
                ..Default::default()
            },
        )
        .await;
    }
    if !source.auto_execute {
        let column = if source.target_column.is_empty() {
            "TODO"
        } else {
            &source.target_column
        };
        let _ = tasks::move_to(&state.db, &task.id, column, None).await;
    }
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });

    if source.auto_execute {
        let body = crate::api::tasks::DispatchBody {
            policy: None,
            manual_agent: source.target_agent.clone(),
            manual_model: None,
            manual_effort: None,
            execution_mode: None,
            prompt_append: None,
        };
        if let Err(error) =
            crate::api::tasks::dispatch_task_internal(state, &task.id, body, None).await
        {
            tracing::warn!(task = %task.id, error = %error, "io bus task queued without engine dispatch");
            let _ = db::io_bus::mark_inbound(
                &state.db,
                &source.id,
                "QUEUED_TASK",
                Some(&format!("{error:#}")),
            )
            .await;
            let column = if source.target_column.is_empty() {
                "TODO"
            } else {
                &source.target_column
            };
            let _ = tasks::move_to(&state.db, &task.id, column, None).await;
            return Ok(Some(tasks::get(&state.db, &task.id).await?));
        }
    } else {
        let _ = db::io_bus::mark_inbound(&state.db, &source.id, "TASK_CREATED", None).await;
    }
    Ok(Some(tasks::get(&state.db, &task.id).await?))
}
