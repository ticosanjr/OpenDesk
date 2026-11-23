use super::channel::DeliveryChannel;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn deliver_whatsapp(
    pool: &SqlitePool,
    secrets: &SecretStore,
    workspace_id: &str,
    run_id: &str,
    title: &str,
    run: &runs::TaskRun,
    artifact: Option<&artifacts::ArtifactProvenance>,
) -> AppResult<()> {
    let config = opendesk_channels::whatsapp::load_config(pool).await?;
    let to = config.phone_number.trim();
    if to.is_empty() {
        tracing::warn!(
            run = run_id,
            "WhatsApp notification skipped: phone_number is not configured"
        );
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
    let key = format!("whatsapp:{run_id}:{to}");
    let Some(delivery) = deliveries::create_pending(
        pool,
        workspace_id,
        artifact.map(|artifact| artifact.id.as_str()),
        DeliveryChannel::WhatsApp.as_str(),
        &key,
    )
    .await?
    else {
        return Ok(());
    };
    match opendesk_channels::whatsapp::send_text(pool, secrets, to, &message).await {
        Ok(sent) => {
            let _ = deliveries::mark_success(pool, &delivery.id, sent.message_id.as_deref(), None)
                .await;
        }
        Err(error) => {
            let payload = serde_json::json!({ "to": to, "message": message }).to_string();
            let _ = deliveries::mark_failed_retryable(
                pool,
                &delivery.id,
                &format!("{error:#}"),
                &payload,
                5,
            )
            .await;
            tracing::warn!(run = run_id, error = %error, "WhatsApp delivery failed");
        }
    }
    Ok(())
}
