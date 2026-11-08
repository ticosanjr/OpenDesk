use super::*;

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
