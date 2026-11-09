use super::channel::DeliveryChannel;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn deliver_email(
    pool: &SqlitePool,
    workspace_id: &str,
    run_id: &str,
    artifact_id: Option<&str>,
    subject: &str,
    html: &str,
    attachments: Vec<EmailAttachment>,
    retry_artifacts: &[String],
    recipients: &[String],
    config: &smtp_mailer::SmtpConfig,
) -> AppResult<()> {
    for recipient in recipients {
        let key = format!("smtp:{run_id}:{recipient}");
        let Some(delivery) = deliveries::create_pending(
            pool,
            workspace_id,
            artifact_id,
            DeliveryChannel::Smtp.as_str(),
            &key,
        )
        .await?
        else {
            continue;
        };

        match with_retry(3, || {
            smtp_mailer::send_email(
                config,
                recipient,
                subject,
                html.to_string(),
                attachments.clone(),
            )
        })
        .await
        {
            Ok(()) => {
                let _ = deliveries::mark_success(pool, &delivery.id, None, None).await;
                tracing::info!(run = run_id, recipient, "run summary emailed");
            }
            Err(error) => {
                let payload = serde_json::json!({
                    "recipient": recipient,
                    "subject": subject,
                    "html": html,
                    "artifact_paths": retry_artifacts,
                })
                .to_string();
                let _ = deliveries::mark_failed_retryable(
                    pool,
                    &delivery.id,
                    &format!("{error:#}"),
                    &payload,
                    5,
                )
                .await;
                tracing::warn!(run = run_id, recipient, error = %error, "email delivery failed; scheduled for retry");
            }
        }
    }
    Ok(())
}

pub(super) async fn retry_email(
    state: &AppState,
    delivery: &deliveries::Delivery,
    payload: &serde_json::Value,
) -> AppResult<()> {
    let recipient = payload
        .get("recipient")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let subject = payload
        .get("subject")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("[OpenDesk] entrega pendente");
    let html = payload
        .get("html")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let paths = payload
        .get("artifact_paths")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let attachments = attachments_from_paths(&paths);

    let Some(config) =
        smtp_mailer::load_config(&state.db, &state.secrets, &delivery.workspace_id).await?
    else {
        let _ = deliveries::reschedule(&state.db, &delivery.id, "SMTP not configured", 60).await;
        return Ok(());
    };

    match smtp_mailer::send_email(&config, recipient, subject, html.to_string(), attachments).await
    {
        Ok(()) => {
            let _ = deliveries::mark_success(&state.db, &delivery.id, None, None).await;
        }
        Err(error) => {
            let _ = deliveries::reschedule(
                &state.db,
                &delivery.id,
                &format!("{error:#}"),
                retry_delay(delivery.attempts),
            )
            .await;
        }
    }
    Ok(())
}
