use super::channel::DeliveryChannel;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn deliver_telegram(
    pool: &SqlitePool,
    secrets: &SecretStore,
    workspace_id: &str,
    run_id: &str,
    title: &str,
    run: &runs::TaskRun,
    artifact: Option<&artifacts::ArtifactProvenance>,
    schedule: &schedules::Schedule,
) -> AppResult<()> {
    let config = telegram::load(pool).await?;
    let Some(token) = telegram::token(secrets) else {
        tracing::warn!(schedule = %schedule.id, "Telegram notification skipped: token is not configured");
        return Ok(());
    };
    let mut recipients = schedule
        .notify_telegram_recipients
        .as_deref()
        .map(split_recipients)
        .unwrap_or_default();
    if recipients.is_empty() {
        recipients = config.allowlist.clone();
    }
    let recipients = telegram::resolve_recipients(pool, &recipients).await;
    if recipients.is_empty() {
        tracing::warn!(schedule = %schedule.id, "Telegram notification skipped: no known chat");
        return Ok(());
    }
    let message = format!(
        "[OpenDesk] {title}\nstatus: {}\n{}\n{}",
        run.status,
        run.summary_response.as_deref().unwrap_or("sem resumo"),
        artifact
            .map(|artifact| artifact.absolute_path.as_str())
            .unwrap_or("sem artefato")
    );

    for recipient in recipients {
        let key = format!("telegram:{run_id}:{recipient}");
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
                let _ = deliveries::mark_success(pool, &delivery.id, None, None).await;
            }
            Err(error) => {
                let _ = deliveries::mark_failed(pool, &delivery.id, &format!("{error:#}")).await;
                tracing::warn!(run = run_id, recipient, error = %error, "Telegram notification failed");
            }
        }
    }
    Ok(())
}
