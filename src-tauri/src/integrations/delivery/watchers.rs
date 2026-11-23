use super::*;

/// Completion watchers are per-task subscriptions. They fire on every terminal
/// run, including failures, and are deduplicated per run+recipient through the
/// same delivery ledger used by routine notifications.
pub(crate) async fn notify_watchers(
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
pub(crate) async fn notification_route(
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

pub(crate) fn novelty_hash(
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

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
