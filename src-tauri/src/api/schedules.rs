use crate::{
    app_state::AppState,
    db::{
        runs::{self, TaskRun},
        schedules::{self, CreateScheduleInput, Schedule, UpdateScheduleInput},
    },
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    integrations::{signal, smtp_mailer},
    queue_scheduler::{cron_engine, recurrence},
    secrets::SecretStore,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct ScheduleQuery {
    pub workspace_id: Option<String>,
}

pub async fn list_schedules(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ScheduleQuery>,
) -> AppResult<Json<Vec<Schedule>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(schedules::list(&state.db, &workspace_id).await?))
}

#[derive(Debug, Deserialize)]
pub struct PreviewBody {
    pub recurrence: recurrence::Recurrence,
    pub timezone: String,
}

pub async fn preview_schedule(Json(body): Json<PreviewBody>) -> AppResult<Json<serde_json::Value>> {
    let cron_expression = body.recurrence.to_cron()?;
    let next_runs = recurrence::next_runs(&cron_expression, &body.timezone, Utc::now(), 5)?;
    Ok(Json(serde_json::json!({
        "cron_expression": cron_expression,
        "human": body.recurrence.human(),
        "next_runs": next_runs,
    })))
}

pub async fn create_schedule(
    State(state): State<Arc<AppState>>,
    Json(mut input): Json<CreateScheduleInput>,
) -> AppResult<Json<Schedule>> {
    let parsed = input
        .recurrence_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<recurrence::Recurrence>(value).ok());
    let trigger_type = input.trigger_type.trim().to_uppercase();
    let next = if trigger_type == "CRON" {
        if let Some(recurrence) = &parsed {
            input.cron_expression = recurrence.to_cron()?;
            input.one_time = recurrence.frequency == "once";
            if input.one_time {
                Some(recurrence.once_datetime(&input.timezone)?)
            } else {
                Some(cron_engine::next_occurrence(
                    &input.cron_expression,
                    &input.timezone,
                    Utc::now(),
                )?)
            }
        } else {
            Some(cron_engine::next_occurrence(
                &input.cron_expression,
                &input.timezone,
                Utc::now(),
            )?)
        }
    } else {
        None
    };
    if input.one_time && next.is_some_and(|next| next <= Utc::now()) {
        return Err(AppError::BadRequest(
            "a rotina de uma vez precisa estar no futuro".into(),
        ));
    }
    let schedule = schedules::create(&state.db, input).await?;
    if let Some(next) = next {
        schedules::set_next_run(&state.db, &schedule.id, Some(&next.to_rfc3339())).await?;
    }
    let schedule = schedules::get(&state.db, &schedule.id).await?;
    sync_trigger_source(&state, &schedule).await?;
    state.io_bus.reload(state.clone()).await?;
    state.events.publish(OpenDeskWsEvent::ScheduleUpdated {
        schedule_id: schedule.id.clone(),
        workspace_id: schedule.workspace_id.clone(),
    });
    Ok(Json(schedule))
}

pub async fn get_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Schedule>> {
    Ok(Json(schedules::get(&state.db, &id).await?))
}

pub async fn update_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut input): Json<UpdateScheduleInput>,
) -> AppResult<Json<Schedule>> {
    let parsed = input
        .recurrence_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<recurrence::Recurrence>(value).ok());
    if let Some(recurrence) = &parsed {
        input.cron_expression = Some(recurrence.to_cron()?);
        input.one_time = Some(recurrence.frequency == "once");
    }
    let schedule = schedules::update(&state.db, &id, input).await?;
    let next = if schedule.trigger_type != "CRON" {
        None
    } else if schedule.one_time {
        Some(
            recurrence::parse_recurrence(schedule.recurrence_json.as_deref())
                .map(|recurrence| recurrence.once_datetime(&schedule.timezone))
                .transpose()?
                .unwrap_or_else(|| {
                    cron_engine::next_occurrence(
                        &schedule.cron_expression,
                        &schedule.timezone,
                        Utc::now(),
                    )
                    .unwrap_or_else(|_| Utc::now())
                }),
        )
    } else {
        Some(cron_engine::next_occurrence(
            &schedule.cron_expression,
            &schedule.timezone,
            Utc::now(),
        )?)
    };
    schedules::set_next_run(
        &state.db,
        &id,
        next.map(|value| value.to_rfc3339()).as_deref(),
    )
    .await?;
    let schedule = schedules::get(&state.db, &id).await?;
    sync_trigger_source(&state, &schedule).await?;
    state.io_bus.reload(state.clone()).await?;
    state.events.publish(OpenDeskWsEvent::ScheduleUpdated {
        schedule_id: schedule.id.clone(),
        workspace_id: schedule.workspace_id.clone(),
    });
    Ok(Json(schedule))
}

pub async fn delete_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    remove_trigger_sources(&state, &id).await?;
    schedules::delete(&state.db, &id).await?;
    state.io_bus.reload(state.clone()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn run_schedule_now(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    crate::api::engine::require_engine(&state).await?;
    let result = state.cron.run_now(&id).await?;
    Ok(Json(serde_json::json!({ "ok": true, "result": result })))
}

/// Create, update or remove the inbound source that feeds a non-cron routine.
/// Cron routines need no source; SIGNAL_MSG is delivered by the Signal bridge.
pub(crate) async fn sync_trigger_source(
    state: &Arc<AppState>,
    schedule: &Schedule,
) -> AppResult<()> {
    let source_type = match schedule.trigger_type.as_str() {
        "FOLDER_WATCH" => Some("FOLDER_WATCH"),
        "RSYNC_FOLDER" => Some("FOLDER_WATCH"),
        "IMAP_IDLE" => Some("IMAP_IDLE"),
        "WEBHOOK" => Some("WEBHOOK_HTTP"),
        "SQL_POLL" => Some("SQL_POLLER"),
        _ => None,
    };
    let existing = crate::db::io_bus::list_inbound(&state.db, &schedule.workspace_id)
        .await?
        .into_iter()
        .find(|source| {
            serde_json::from_str::<serde_json::Value>(&source.config_json)
                .ok()
                .and_then(|config| {
                    config
                        .get("schedule_id")
                        .and_then(|value| value.as_str())
                        .map(|value| value == schedule.id)
                })
                .unwrap_or(false)
        });

    let Some(source_type) = source_type else {
        if let Some(source) = existing {
            crate::db::io_bus::delete_inbound(&state.db, &source.id).await?;
        }
        return Ok(());
    };

    let mut config = serde_json::from_str::<serde_json::Value>(&schedule.trigger_config_json)
        .unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = config.as_object_mut() {
        object.insert(
            "schedule_id".into(),
            serde_json::Value::String(schedule.id.clone()),
        );
    }
    let input = crate::db::io_bus::InboundInput {
        name: format!("Rotina: {}", schedule.name),
        source_type: source_type.into(),
        config,
        target_agent: schedule.agent_name.clone(),
        target_column: Some("TODO".into()),
        auto_execute: true,
        is_enabled: schedule.is_active,
    };
    if let Some(source) = existing {
        crate::db::io_bus::update_inbound(&state.db, &source.id, input).await?;
    } else {
        crate::db::io_bus::create_inbound(&state.db, &schedule.workspace_id, input).await?;
    }
    Ok(())
}

pub(crate) async fn remove_trigger_sources(
    state: &Arc<AppState>,
    schedule_id: &str,
) -> AppResult<()> {
    let schedule = schedules::get(&state.db, schedule_id).await?;
    for source in crate::db::io_bus::list_inbound(&state.db, &schedule.workspace_id).await? {
        let linked = serde_json::from_str::<serde_json::Value>(&source.config_json)
            .ok()
            .and_then(|config| {
                config
                    .get("schedule_id")
                    .and_then(|value| value.as_str())
                    .map(|value| value == schedule_id)
            })
            .unwrap_or(false);
        if linked {
            crate::db::io_bus::delete_inbound(&state.db, &source.id).await?;
        }
    }
    Ok(())
}

/// Rebuild every routine source. Called once at boot so routines created
/// before the trigger plumbing existed keep working.
pub async fn sync_all_trigger_sources(state: &Arc<AppState>) -> AppResult<()> {
    for workspace in crate::db::workspaces::list(&state.db).await? {
        for schedule in schedules::list(&state.db, &workspace.id).await? {
            if let Err(error) = sync_trigger_source(state, &schedule).await {
                tracing::warn!(schedule = %schedule.id, error = %error, "routine source sync failed");
            }
        }
    }
    Ok(())
}

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
        let yaml = crate::recipes::store::yaml_for(&state.db, recipe_id).await?;
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
        let run = match crate::recipes::engine::start(state.clone(), recipe, trigger, false).await {
            Ok(run) => run,
            Err(error) => {
                let _ = schedules::mark_run(
                    &state.db,
                    &schedule.id,
                    "FAILED",
                    &Utc::now().to_rfc3339(),
                )
                .await;
                return Err(error);
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
            let yaml = crate::recipes::store::yaml_for(&state.db, recipe_id).await?;
            let recipe = crate::recipes::parser::parse(&yaml)?;
            let trigger = serde_json::json!({
                "routine_id": schedule.id,
                "routine_name": schedule.name,
                "signal_source": source_number,
                "text": text,
            });
            if let Err(error) =
                crate::recipes::engine::start(state.clone(), recipe, trigger, false).await
            {
                let _ = schedules::mark_run(
                    &state.db,
                    &schedule.id,
                    "FAILED",
                    &Utc::now().to_rfc3339(),
                )
                .await;
                return Err(error);
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
        let Ok(yaml) = crate::recipes::store::yaml_for(&state.db, &index.id).await else {
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
            crate::recipes::engine::start(state.clone(), recipe.clone(), context, false).await?;
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Debug, Deserialize)]
pub struct ActiveBody {
    pub active: bool,
}

pub async fn set_schedule_active(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ActiveBody>,
) -> AppResult<Json<Schedule>> {
    let schedule = schedules::set_active(&state.db, &id, body.active).await?;
    if body.active && schedule.trigger_type == "CRON" {
        let next = cron_engine::next_occurrence(
            &schedule.cron_expression,
            &schedule.timezone,
            Utc::now(),
        )?;
        schedules::set_next_run(&state.db, &id, Some(&next.to_rfc3339())).await?;
    }
    sync_trigger_source(&state, &schedule).await?;
    state.io_bus.reload(state.clone()).await?;
    let schedule = schedules::get(&state.db, &id).await?;
    state.events.publish(OpenDeskWsEvent::ScheduleUpdated {
        schedule_id: schedule.id.clone(),
        workspace_id: schedule.workspace_id.clone(),
    });
    Ok(Json(schedule))
}

#[derive(Debug, Deserialize)]
pub struct TickQuery {
    #[allow(dead_code)]
    pub force: Option<bool>,
}

pub async fn tick_scheduler(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<TickQuery>,
) -> AppResult<Json<cron_engine::CronTickResult>> {
    let result = state.cron.tick_once().await?;
    Ok(Json(result))
}

#[allow(dead_code)]
fn _error(_: AppError) {}

#[derive(Debug, serde::Serialize)]
pub struct DryRunCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

pub async fn dry_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let schedule = schedules::get(&state.db, &id).await?;
    let workspace = crate::db::workspaces::get(&state.db, &schedule.workspace_id).await?;
    let mut checks = Vec::new();
    let mut ok = true;

    if schedule.target_kind == "RECIPE" {
        let recipe = match schedule.recipe_id.as_deref() {
            Some(recipe_id) => crate::recipes::store::yaml_for(&state.db, recipe_id)
                .await
                .ok()
                .and_then(|yaml| crate::recipes::parser::parse(&yaml).ok())
                .is_some(),
            None => false,
        };
        checks.push(DryRunCheck {
            name: "Receita".into(),
            ok: recipe,
            detail: if recipe {
                format!(
                    "receita {} válida",
                    schedule.recipe_id.clone().unwrap_or_default()
                )
            } else {
                "receita ausente ou inválida".into()
            },
        });
        ok &= recipe;
    } else {
        let prompt_ok = !schedule.command_or_prompt.trim().is_empty();
        checks.push(DryRunCheck {
            name: "Prompt".into(),
            ok: prompt_ok,
            detail: if prompt_ok {
                "comando/prompt presente".into()
            } else {
                "comando/prompt vazio".into()
            },
        });
        ok &= prompt_ok;
    }

    if schedule.trigger_type == "CRON" {
        let next =
            cron_engine::next_occurrence(&schedule.cron_expression, &schedule.timezone, Utc::now());
        match &next {
            Ok(value) => checks.push(DryRunCheck {
                name: "Agenda".into(),
                ok: true,
                detail: format!("próximo disparo em {}", value.to_rfc3339()),
            }),
            Err(error) => {
                ok = false;
                checks.push(DryRunCheck {
                    name: "Agenda".into(),
                    ok: false,
                    detail: format!("{error}"),
                });
            }
        }
    } else {
        checks.push(DryRunCheck {
            name: "Gatilho".into(),
            ok: true,
            detail: format!("acionado por {}", schedule.trigger_type),
        });
    }

    let folder = cron_engine::render_output_folder(
        &schedule,
        std::path::Path::new(&workspace.root_path),
        Utc::now(),
    );
    match &folder {
        Ok(path) => {
            let result = std::fs::create_dir_all(path);
            let folder_ok = result.is_ok();
            let detail = result
                .err()
                .map(|error| format!("{error}"))
                .unwrap_or_else(|| path.display().to_string());
            checks.push(DryRunCheck {
                name: "Pasta de saída".into(),
                ok: folder_ok,
                detail,
            });
            ok &= folder_ok;
        }
        Err(error) => {
            ok = false;
            checks.push(DryRunCheck {
                name: "Pasta de saída".into(),
                ok: false,
                detail: format!("{error}"),
            });
        }
    }

    let engine_online = state.daemon.status().await.online;
    checks.push(DryRunCheck {
        name: "Engine".into(),
        ok: engine_online,
        detail: if engine_online {
            "OpenCode online".into()
        } else {
            "OpenCode offline".into()
        },
    });
    ok &= engine_online;

    let secrets = SecretStore::new(&state.paths);
    let smtp = smtp_mailer::load_config(&state.db, &secrets, &schedule.workspace_id).await?;
    checks.push(DryRunCheck {
        name: "SMTP".into(),
        ok: smtp.is_some(),
        detail: if smtp.is_some() {
            "configurado".into()
        } else {
            "não configurado".into()
        },
    });

    let google = state.google.status(&schedule.workspace_id).await?;
    checks.push(DryRunCheck {
        name: "Google".into(),
        ok: google.connected,
        detail: google.email.unwrap_or_else(|| "não conectado".into()),
    });

    Ok(Json(serde_json::json!({
        "ok": ok,
        "checks": checks,
    })))
}

pub async fn schedule_runs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<TaskRun>>> {
    schedules::get(&state.db, &id).await?;
    Ok(Json(runs::list_by_schedule(&state.db, &id).await?))
}

#[derive(Debug, Deserialize)]
pub struct NotifyTestBody {
    pub channel: String,
}

pub async fn test_notification(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<NotifyTestBody>,
) -> AppResult<Json<serde_json::Value>> {
    let schedule = schedules::get(&state.db, &id).await?;
    let subject = format!("[OpenDesk] Teste de notificação · {}", schedule.name);
    let html = format!(
        "<p>Este é um teste de notificação da rotina <strong>{}</strong>.</p>\
         <p>Próximo disparo: {}</p>",
        schedule.name,
        schedule.next_run_at.as_deref().unwrap_or("não agendado")
    );
    match body.channel.as_str() {
        "email" => {
            let recipients =
                split_recipients(schedule.notify_email_recipients.as_deref().unwrap_or(""));
            if recipients.is_empty() {
                return Err(AppError::BadRequest(
                    "a rotina não tem destinatários de e-mail".into(),
                ));
            }
            let secrets = SecretStore::new(&state.paths);
            let config = smtp_mailer::load_config(&state.db, &secrets, &schedule.workspace_id)
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest("SMTP não está configurado para este workspace".into())
                })?;
            for recipient in &recipients {
                smtp_mailer::send_email(&config, recipient, &subject, html.clone(), Vec::new())
                    .await
                    .map_err(|error| AppError::Daemon(format!("SMTP test failed: {error:#}")))?;
            }
            Ok(Json(serde_json::json!({
                "sent": recipients.len(),
                "channel": "email"
            })))
        }
        "signal" => {
            let config = signal::load(&state.db).await?;
            if config.account.trim().is_empty() {
                return Err(AppError::BadRequest(
                    "Signal não está vinculado neste OpenDesk".into(),
                ));
            }
            let mut recipients =
                split_recipients(schedule.notify_signal_recipients.as_deref().unwrap_or(""));
            if recipients.is_empty() {
                recipients = config.allowlist.clone();
            }
            if recipients.is_empty() {
                return Err(AppError::BadRequest(
                    "a rotina não tem destinatários Signal e a allowlist está vazia".into(),
                ));
            }
            let message = format!("[OpenDesk] Teste da rotina {}", schedule.name);
            for recipient in &recipients {
                signal::send_text(&config.cli_path, &config.account, recipient, &message).await?;
            }
            Ok(Json(serde_json::json!({
                "sent": recipients.len(),
                "channel": "signal"
            })))
        }
        other => Err(AppError::BadRequest(format!(
            "invalid notification channel '{other}'; expected email or signal"
        ))),
    }
}

fn split_recipients(value: &str) -> Vec<String> {
    value
        .split([',', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::split_recipients;

    #[test]
    fn splits_notification_recipients() {
        assert_eq!(
            split_recipients(" a@x.com, b@x.com; c@x.com "),
            vec!["a@x.com", "b@x.com", "c@x.com"]
        );
        assert!(split_recipients("  ").is_empty());
    }
}
