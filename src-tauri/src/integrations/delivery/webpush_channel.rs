use super::channel::DeliveryChannel;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn deliver_webpush(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    run_id: &str,
    title: &str,
    run: &runs::TaskRun,
    artifact: Option<&artifacts::ArtifactProvenance>,
) -> AppResult<()> {
    let key = format!("webpush:{run_id}");
    let Some(delivery) = deliveries::create_pending(
        pool,
        workspace_id,
        artifact.map(|artifact| artifact.id.as_str()),
        DeliveryChannel::WebPush.as_str(),
        &key,
    )
    .await?
    else {
        return Ok(());
    };
    let body = format!(
        "{} · {}",
        run.status,
        run.summary_response.as_deref().unwrap_or("sem resumo")
    );
    let payload = opendesk_channels::webpush::PushPayload::new(title, body.clone(), "/");
    match opendesk_channels::webpush::send_to_all(pool, paths, &payload).await {
        Ok(sent) => {
            let _ = deliveries::mark_success(pool, &delivery.id, None, None).await;
            tracing::info!(run = run_id, devices = sent, "WebPush delivery sent");
        }
        Err(error) => {
            let payload = serde_json::json!({ "title": title, "body": body }).to_string();
            let _ = deliveries::mark_failed_retryable(
                pool,
                &delivery.id,
                &format!("{error:#}"),
                &payload,
                5,
            )
            .await;
            tracing::warn!(run = run_id, error = %error, "WebPush delivery failed");
        }
    }
    Ok(())
}
