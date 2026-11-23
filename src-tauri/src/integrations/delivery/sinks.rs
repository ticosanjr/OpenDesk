use super::*;

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

pub(crate) async fn retry_delivery(
    state: &AppState,
    delivery: &deliveries::Delivery,
) -> AppResult<()> {
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
        Some(DeliveryChannel::WhatsApp) => retry_whatsapp(state, delivery, &payload).await,
        Some(DeliveryChannel::WebPush) => retry_webpush(state, delivery, &payload).await,
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

pub(crate) fn retry_delay(attempts: i64) -> i64 {
    (5i64 * 2i64.pow(attempts.clamp(0, 6) as u32)).min(360)
}

/// Best-effort retry for transient network failures. The delivery rows are
/// idempotent, so a retry never duplicates a successful destination.
pub(crate) async fn with_retry<F, Fut, T>(attempts: usize, mut operation: F) -> anyhow::Result<T>
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

async fn retry_whatsapp(
    state: &AppState,
    delivery: &deliveries::Delivery,
    payload: &serde_json::Value,
) -> AppResult<()> {
    let to = payload
        .get("to")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let message = payload
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if to.trim().is_empty() || message.trim().is_empty() {
        let _ = deliveries::mark_skipped(&state.db, &delivery.id, "payload sem to/message").await;
        return Ok(());
    }
    let secrets = SecretStore::new(&state.paths);
    match opendesk_channels::whatsapp::send_text(&state.db, &secrets, to, message).await {
        Ok(sent) => {
            let _ =
                deliveries::mark_success(&state.db, &delivery.id, sent.message_id.as_deref(), None)
                    .await;
        }
        Err(error) => {
            let _ = deliveries::mark_failed(&state.db, &delivery.id, &format!("{error:#}")).await;
        }
    }
    Ok(())
}

async fn retry_webpush(
    state: &AppState,
    delivery: &deliveries::Delivery,
    payload: &serde_json::Value,
) -> AppResult<()> {
    let title = payload
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("OpenDesk");
    let body = payload
        .get("body")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let payload = opendesk_channels::webpush::PushPayload::new(title, body, "/");
    match opendesk_channels::webpush::send_to_all(&state.db, &state.paths, &payload).await {
        Ok(_) => {
            let _ = deliveries::mark_success(&state.db, &delivery.id, None, None).await;
        }
        Err(error) => {
            let _ = deliveries::mark_failed(&state.db, &delivery.id, &format!("{error:#}")).await;
        }
    }
    Ok(())
}
