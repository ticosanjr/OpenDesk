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
    let next = if let Some(recurrence) = &parsed {
        input.cron_expression = recurrence.to_cron()?;
        input.one_time = recurrence.frequency == "once";
        if input.one_time {
            recurrence.once_datetime(&input.timezone)?
        } else {
            cron_engine::next_occurrence(&input.cron_expression, &input.timezone, Utc::now())?
        }
    } else {
        cron_engine::next_occurrence(&input.cron_expression, &input.timezone, Utc::now())?
    };
    if input.one_time && next <= Utc::now() {
        return Err(AppError::BadRequest(
            "a rotina de uma vez precisa estar no futuro".into(),
        ));
    }
    let schedule = schedules::create(&state.db, input).await?;
    schedules::set_next_run(&state.db, &schedule.id, Some(&next.to_rfc3339())).await?;
    let schedule = schedules::get(&state.db, &schedule.id).await?;
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
    let next = if schedule.one_time {
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
            })
    } else {
        cron_engine::next_occurrence(&schedule.cron_expression, &schedule.timezone, Utc::now())?
    };
    schedules::set_next_run(&state.db, &id, Some(&next.to_rfc3339())).await?;
    let schedule = schedules::get(&state.db, &id).await?;
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
    schedules::delete(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn run_schedule_now(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    crate::api::engine::require_engine(&state).await?;
    let job = state.cron.run_now(&id).await?;
    Ok(Json(serde_json::json!({ "ok": true, "job": job })))
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
    if body.active {
        let next = cron_engine::next_occurrence(
            &schedule.cron_expression,
            &schedule.timezone,
            Utc::now(),
        )?;
        schedules::set_next_run(&state.db, &id, Some(&next.to_rfc3339())).await?;
    }
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

    checks.push(DryRunCheck {
        name: "Prompt".into(),
        ok: !schedule.command_or_prompt.trim().is_empty(),
        detail: if schedule.command_or_prompt.trim().is_empty() {
            "comando/prompt vazio".into()
        } else {
            "comando/prompt presente".into()
        },
    });
    ok &= !schedule.command_or_prompt.trim().is_empty();

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
