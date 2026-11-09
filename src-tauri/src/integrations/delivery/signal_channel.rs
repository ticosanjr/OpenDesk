use super::channel::DeliveryChannel;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn deliver_signal(
    pool: &SqlitePool,
    workspace_id: &str,
    run_id: &str,
    title: &str,
    run: &runs::TaskRun,
    artifact: Option<&artifacts::ArtifactProvenance>,
    schedule: &schedules::Schedule,
) -> AppResult<()> {
    let config = signal::load(pool).await?;
    let mut recipients = schedule
        .notify_signal_recipients
        .as_deref()
        .map(split_recipients)
        .unwrap_or_default();
    if recipients.is_empty() {
        recipients = config.allowlist.clone();
    }
    let message = format!(
        "[OpenDesk] {title}\nstatus: {}\n{}\n{}",
        run.status,
        run.summary_response.as_deref().unwrap_or("sem resumo"),
        artifact
            .map(|artifact| artifact.absolute_path.as_str())
            .unwrap_or("sem artefato")
    );
    if config.account.trim().is_empty() {
        tracing::warn!(schedule = %schedule.id, "Signal notification skipped: account is not configured");
        return Ok(());
    }

    for recipient in recipients {
        let key = format!("signal:{run_id}:{recipient}");
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
        match signal::send_text(&config.cli_path, &config.account, &recipient, &message).await {
            Ok(()) => {
                let _ = deliveries::mark_success(pool, &delivery.id, None, None).await;
            }
            Err(error) => {
                let _ = deliveries::mark_failed(pool, &delivery.id, &format!("{error:#}")).await;
                tracing::warn!(run = run_id, recipient, error = %error, "Signal notification failed");
            }
        }
    }
    Ok(())
}
