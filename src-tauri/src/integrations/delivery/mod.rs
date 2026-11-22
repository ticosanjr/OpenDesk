mod channel;
mod email;
mod google;
mod render;
mod signal_channel;
mod telegram_channel;
#[cfg(test)]
mod tests;

use crate::{
    app_state::AppState,
    db::{artifacts, deliveries, runs, schedules, settings, task_watchers, tasks, workspaces},
    error::{AppError, AppResult},
    integrations::{
        google_api::GoogleApi,
        google_oauth::GoogleOAuth,
        signal,
        smtp_mailer::{self, EmailAttachment},
        telegram,
    },
    media::mime_for_path,
    paths::AppPaths,
    secrets::SecretStore,
    text::split_recipients,
};
use channel::DeliveryChannel;
use chrono::{Timelike, Utc};
use email::{deliver_email, retry_email};
use google::{export_google, retry_drive, retry_gdocs};
use html_escape::encode_text;
use render::{attachments_from_paths, build_attachments, build_html};
use sha2::{Digest, Sha256};
use signal_channel::deliver_signal;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use telegram_channel::deliver_telegram;

pub async fn deliver_run(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    task_id: Option<&str>,
    schedule_id: Option<&str>,
    run_id: &str,
    google: Arc<GoogleOAuth>,
) -> AppResult<()> {
    let run = match runs::get(pool, run_id).await {
        Ok(run) => run,
        Err(_) => return Ok(()),
    };

    if run.status != "SUCCESS" {
        if let Some(task_id) = task_id {
            let title = tasks::get(pool, task_id)
                .await
                .map(|task| task.title)
                .unwrap_or_else(|_| {
                    run.task_title
                        .clone()
                        .unwrap_or_else(|| "OpenDesk run".to_string())
                });
            let secrets = SecretStore::new(paths);
            if let Err(error) =
                notify_watchers(pool, &secrets, workspace_id, task_id, &title, &run, &[]).await
            {
                tracing::warn!(run = run_id, error = %error, "task watcher failed");
            }
        }
        return Ok(());
    }

    let schedule = match schedule_id {
        Some(id) => schedules::get(pool, id).await.ok(),
        None => None,
    };

    let mut recipients = Vec::new();
    let mut export_to_gdocs = false;
    let mut sync_to_drive = false;
    let mut title = run
        .task_title
        .clone()
        .unwrap_or_else(|| "OpenDesk run".to_string());

    if let Some(task_id) = task_id
        && let Ok(task) = tasks::get(pool, task_id).await
    {
        title = task.title.clone();
        export_to_gdocs = task.export_to_gdocs;
        sync_to_drive = task.export_to_gdocs;
        if let Some(email) = task.notify_email {
            recipients.extend(split_recipients(&email));
        }
    }

    if let Some(schedule) = &schedule {
        title = schedule.name.clone();
        export_to_gdocs = schedule.sync_to_google_docs;
        sync_to_drive = schedule.sync_to_google_docs;
        if let Some(email) = &schedule.notify_email_recipients {
            recipients.extend(split_recipients(email));
        }
    }

    recipients.sort();
    recipients.dedup();

    let workspace_row = workspaces::get(pool, workspace_id).await.ok();
    let workspace_root = workspace_row
        .as_ref()
        .map(|workspace| PathBuf::from(&workspace.root_path));
    let notification_route = notification_route(
        pool,
        workspace_row
            .as_ref()
            .map(|workspace| workspace.timezone.as_str()),
    )
    .await;
    let allows_channel = |channel: &str| match &notification_route {
        Some(route) => route.deliver && route.channels.iter().any(|item| item == channel),
        None => true,
    };

    let artifact_list = artifacts::list(pool, workspace_id, None, task_id, None, 50)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|artifact| artifact.run_id == run_id)
        .collect::<Vec<_>>();

    let novelty_hash = novelty_hash(&artifact_list, run.summary_response.as_deref());
    let unchanged_output = schedule.as_ref().is_some_and(|schedule| {
        schedule.notify_only_on_change
            && novelty_hash.is_some()
            && schedule.last_output_hash.as_deref() == novelty_hash.as_deref()
    });
    if unchanged_output {
        tracing::info!(
            run = run_id,
            "no output change; keeping exports and skipping notifications"
        );
    }

    if export_to_gdocs || sync_to_drive {
        export_google(
            pool,
            google.clone(),
            workspace_id,
            task_id,
            run_id,
            &title,
            &artifact_list,
            export_to_gdocs,
            sync_to_drive,
        )
        .await;
    }

    let subject = format!("[OpenDesk] {title} · concluído");
    let html = build_html(&title, &run, &artifact_list);
    let attachments = build_attachments(&artifact_list, workspace_root.as_deref());
    let retry_artifacts = artifact_list
        .iter()
        .filter(|artifact| artifact.action_type != "DELETED")
        .map(|artifact| artifact.absolute_path.clone())
        .collect::<Vec<_>>();
    let secrets = SecretStore::new(paths);
    let smtp_config = smtp_mailer::load_config(pool, &secrets, workspace_id)
        .await
        .ok()
        .flatten();

    if !unchanged_output && !recipients.is_empty() && smtp_config.is_none() {
        tracing::info!(
            workspace = workspace_id,
            "SMTP is not configured; skipping email delivery"
        );
    }

    if let Some(config) = &smtp_config
        && !unchanged_output
        && allows_channel("email")
    {
        deliver_email(
            pool,
            workspace_id,
            run_id,
            artifact_list.first().map(|artifact| artifact.id.as_str()),
            &subject,
            &html,
            attachments,
            &retry_artifacts,
            &recipients,
            config,
        )
        .await?;
    }

    if let Some(schedule) = &schedule
        && schedule.notify_signal
        && !unchanged_output
        && allows_channel("signal")
    {
        deliver_signal(
            pool,
            workspace_id,
            run_id,
            &title,
            &run,
            artifact_list.first(),
            schedule,
        )
        .await?;
    }

    if let Some(schedule) = &schedule
        && schedule.notify_telegram
        && !unchanged_output
        && allows_channel("telegram")
    {
        deliver_telegram(
            pool,
            &secrets,
            workspace_id,
            run_id,
            &title,
            &run,
            artifact_list.first(),
            schedule,
        )
        .await?;
    }

    if let Some(task_id) = task_id
        && let Err(error) = notify_watchers(
            pool,
            &secrets,
            workspace_id,
            task_id,
            &title,
            &run,
            &artifact_list,
        )
        .await
    {
        tracing::warn!(run = run_id, error = %error, "task watcher failed");
    }

    if let (Some(schedule), Some(hash)) = (&schedule, novelty_hash.as_deref()) {
        let _ = schedules::set_last_output_hash(pool, &schedule.id, Some(hash)).await;
    }

    Ok(())
}

/// Completion watchers are per-task subscriptions. They fire on every terminal
/// run, including failures, and are deduplicated per run+recipient through the
/// same delivery ledger used by routine notifications.
async fn notify_watchers(
    pool: &SqlitePool,
    secrets: &SecretStore,
    workspace_id: &str,
    task_id: &str,
    title: &str,
    run: &runs::TaskRun,
    artifact_list: &[artifacts::ArtifactProvenance],
) -> AppResult<()> {
    let watchers = task_watchers::list_for_task(pool, task_id).await?;
    if watchers.is_empty() {
        return Ok(());
    }
    let artifact = artifact_list
        .iter()
        .find(|artifact| artifact.action_type != "DELETED");
    let summary = run
        .summary_response
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            run.error_message
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("sem resumo");
    let message = format!(
        "[OpenDesk] {title}\nstatus: {}\n{}\n{}",
        run.status,
        summary,
        artifact
            .map(|artifact| artifact.file_path.as_str())
            .unwrap_or("sem artefato")
    );
    let attachments = artifact_list
        .iter()
        .filter(|artifact| artifact.action_type != "DELETED")
        .map(|artifact| artifact.absolute_path.clone())
        .collect::<Vec<_>>();

    for watcher in watchers {
        match watcher.channel.as_str() {
            "telegram" => {
                let config = telegram::load(pool).await?;
                let Some(token) = telegram::token(secrets) else {
                    tracing::warn!(task = task_id, "Telegram watcher skipped: token missing");
                    continue;
                };
                let requested = watcher
                    .recipient
                    .as_deref()
                    .map(|recipient| vec![recipient.to_string()])
                    .unwrap_or_else(|| config.allowlist.clone());
                let recipients = telegram::resolve_recipients(pool, &requested).await;
                if recipients.is_empty() {
                    tracing::warn!(task = task_id, "Telegram watcher skipped: no known chat");
                    continue;
                }
                for recipient in recipients {
                    let key = format!("telegram:watch:{}:{}:{}", run.id, watcher.id, recipient);
                    let Some(delivery) = deliveries::create_pending(
                        pool,
                        workspace_id,
                        artifact.map(|artifact| artifact.id.as_str()),
                        DeliveryChannel::Telegram.as_str(),
                        &key,
                    )
                    .await?
                    else {
                        continue;
                    };
                    match telegram::send_text(&token, &recipient, &message).await {
                        Ok(()) => {
                            if let Some(artifact) = artifact {
                                let path = Path::new(&artifact.absolute_path);
                                if path.is_file()
                                    && let Err(error) =
                                        telegram::send_document(&token, &recipient, path, None)
                                            .await
                                {
                                    tracing::warn!(
                                        run = %run.id,
                                        recipient,
                                        error = %error,
                                        "Telegram watcher attachment failed"
                                    );
                                }
                            }
                            let _ = deliveries::mark_success(pool, &delivery.id, None, None).await;
                        }
                        Err(error) => {
                            let _ =
                                deliveries::mark_failed(pool, &delivery.id, &format!("{error:#}"))
                                    .await;
                            tracing::warn!(
                                run = %run.id,
                                recipient,
                                error = %error,
                                "Telegram watcher failed"
                            );
                        }
                    }
                }
            }
            "signal" => {
                let config = signal::load(pool).await?;
                if config.account.trim().is_empty() {
                    tracing::warn!(task = task_id, "Signal watcher skipped: account missing");
                    continue;
                }
                let recipient = watcher
                    .recipient
                    .clone()
                    .or_else(|| config.allowlist.first().cloned());
                let Some(recipient) = recipient else {
                    tracing::warn!(task = task_id, "Signal watcher skipped: no recipient");
                    continue;
                };
                let key = format!("signal:watch:{}:{}:{}", run.id, watcher.id, recipient);
                let Some(delivery) = deliveries::create_pending(
                    pool,
                    workspace_id,
                    artifact.map(|artifact| artifact.id.as_str()),
                    DeliveryChannel::Signal.as_str(),
                    &key,
                )
                .await?
                else {
                    continue;
                };
                match signal::send_with_attachments(
                    &config.cli_path,
                    &config.account,
                    &recipient,
                    &message,
                    &attachments,
                )
                .await
                {
                    Ok(()) => {
                        let _ = deliveries::mark_success(pool, &delivery.id, None, None).await;
                    }
                    Err(error) => {
                        let _ = deliveries::mark_failed(pool, &delivery.id, &format!("{error:#}"))
                            .await;
                        tracing::warn!(
                            run = %run.id,
                            recipient,
                            error = %error,
                            "Signal watcher failed"
                        );
                    }
                }
            }
            other => {
                tracing::warn!(task = task_id, channel = other, "unknown watcher channel");
            }
        }
    }
    Ok(())
}

/// Applies the channel router to run notifications. Missing settings keep the
/// historical behavior (every channel explicitly enabled by the task/schedule).
async fn notification_route(
    pool: &SqlitePool,
    timezone: Option<&str>,
) -> Option<opendesk_channels::NotificationDecision> {
    let value = settings::get(pool, "channels.notification_router")
        .await
        .ok()
        .flatten()?;
    let config: opendesk_channels::RouterConfig = serde_json::from_value(value).ok()?;
    let local_hour = timezone
        .and_then(|value| value.parse::<chrono_tz::Tz>().ok())
        .map(|tz| Utc::now().with_timezone(&tz).hour() as u8)
        .unwrap_or_else(|| Utc::now().hour() as u8);
    Some(opendesk_channels::NotificationRouter::new(config).route(
        opendesk_domain::NotificationLevel::Info,
        opendesk_domain::CareLevel::Normal,
        local_hour,
    ))
}

fn novelty_hash(
    artifacts: &[artifacts::ArtifactProvenance],
    summary: Option<&str>,
) -> Option<String> {
    let artifact = artifacts
        .iter()
        .filter(|artifact| artifact.action_type != "DELETED")
        .find(|artifact| artifact.file_path.ends_with(".md"))
        .or_else(|| {
            artifacts
                .iter()
                .find(|artifact| artifact.action_type != "DELETED")
        });
    if let Some(artifact) = artifact
        && let Ok(bytes) = std::fs::read(&artifact.absolute_path)
    {
        return Some(hex_digest(&bytes));
    }
    summary.map(|value| hex_digest(value.as_bytes()))
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Durable retry worker. It survives restarts because the payload and next
/// attempt live in SQLite.
#[allow(clippy::too_many_arguments)]
pub async fn retry_loop(state: std::sync::Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        let Ok(due) = deliveries::due_retries(&state.db, 10).await else {
            continue;
        };
        for delivery in due {
            if let Err(error) = retry_delivery(&state, &delivery).await {
                tracing::warn!(delivery = %delivery.id, error = %error, "delivery retry failed");
            }
        }
    }
}

async fn retry_delivery(state: &AppState, delivery: &deliveries::Delivery) -> AppResult<()> {
    let Some(payload) = delivery.payload_json.as_deref() else {
        return Ok(());
    };
    let payload: serde_json::Value = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(_) => {
            let _ =
                deliveries::mark_skipped(&state.db, &delivery.id, "invalid retry payload").await;
            return Ok(());
        }
    };
    match DeliveryChannel::parse(&delivery.channel) {
        Some(DeliveryChannel::GoogleDocs) => retry_gdocs(state, delivery, &payload).await,
        Some(DeliveryChannel::GoogleDrive) => retry_drive(state, delivery, &payload).await,
        Some(DeliveryChannel::Smtp) => retry_email(state, delivery, &payload).await,
        Some(DeliveryChannel::Signal) | Some(DeliveryChannel::Telegram) => Ok(()),
        Some(DeliveryChannel::Gmail)
        | Some(DeliveryChannel::GoogleCalendar)
        | Some(DeliveryChannel::GoogleTasks) => {
            let _ = deliveries::mark_skipped(
                &state.db,
                &delivery.id,
                "delivery channel has no retry adapter",
            )
            .await;
            Ok(())
        }
        None => {
            let _ =
                deliveries::mark_skipped(&state.db, &delivery.id, "unknown delivery channel").await;
            Ok(())
        }
    }
}

/// Retry one delivery immediately, on user request. Only failed deliveries
/// with a stored payload are eligible; successful deliveries are never replayed
/// because that could duplicate a document or upload.
pub async fn retry_one(state: &AppState, id: &str) -> AppResult<deliveries::Delivery> {
    let delivery = deliveries::get(&state.db, id).await?;
    if delivery.status != "FAILED" {
        return Err(AppError::BadRequest(format!(
            "somente entregas FAILED podem ser reenviadas (status atual: {})",
            delivery.status
        )));
    }
    if delivery.payload_json.is_none() {
        return Err(AppError::BadRequest(
            "entrega não possui payload para reenvio".into(),
        ));
    }
    retry_delivery(state, &delivery).await?;
    deliveries::get(&state.db, id).await.map_err(Into::into)
}

fn retry_delay(attempts: i64) -> i64 {
    (5i64 * 2i64.pow(attempts.clamp(0, 6) as u32)).min(360)
}

/// Best-effort retry for transient network failures. The delivery rows are
/// idempotent, so a retry never duplicates a successful destination.
async fn with_retry<F, Fut, T>(attempts: usize, mut operation: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let attempts = attempts.max(1);
    let mut last_error = None;
    for attempt in 1..=attempts {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                last_error = Some(error);
                if attempt < attempts {
                    tokio::time::sleep(std::time::Duration::from_millis(400 * attempt as u64))
                        .await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("retry operation produced no result")))
}
