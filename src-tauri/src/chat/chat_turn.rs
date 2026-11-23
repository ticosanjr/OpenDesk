use serde_json::Value;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use super::CANCEL_MESSAGE;
use super::chat_parse::{turn_has_idle, turn_has_text, turn_signature};

/// Waits for the current turn to finish before reading the answer. OpenCode's
/// prompt endpoint is asynchronous; reading the message list immediately used
/// to return the previous assistant reply.
///
/// A tool-using turn emits intermediate `assistant` messages that contain only
/// tool calls. Those must never be mistaken for the final answer, so the idle
/// marker is authoritative: the turn ends only when OpenCode reports idle and a
/// text message from this turn exists. The quiet-signature fallback is only a
/// safety net for a missing idle marker and requires text (30s of silence).
pub(super) async fn wait_for_turn(
    client: &crate::opencode_client::rest_client::RestClient,
    session_id: &str,
    since_ms: i64,
    cancel: &CancellationToken,
    mut events: Option<tokio::sync::mpsc::Receiver<Value>>,
    timeout: Duration,
) -> Result<Value, String> {
    const IDLE_WITHOUT_TEXT_GRACE: Duration = Duration::from_secs(2);
    const TEXT_STABLE_GRACE: Duration = Duration::from_secs(30);
    let mut stable_signature = String::new();
    let mut stable_since: Option<std::time::Instant> = None;
    let mut idle_without_text_since: Option<std::time::Instant> = None;
    let deadline = std::time::Instant::now() + timeout;

    loop {
        if cancel.is_cancelled() {
            return Err(CANCEL_MESSAGE.into());
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "o turno excedeu {} minuto(s) sem resposta final",
                timeout.as_secs() / 60
            ));
        }

        // With an SSE subscription, wait for a terminal event for this session.
        // Without one, keep the historical 500ms polling fallback (used by
        // tests and when the event endpoint is unavailable).
        let fallback = if events.is_some() {
            Duration::from_secs(10)
        } else {
            Duration::from_millis(500)
        };
        let mut terminal_event = false;
        let mut closed = false;
        if let Some(receiver) = events.as_mut() {
            tokio::select! {
                _ = cancel.cancelled() => return Err(CANCEL_MESSAGE.into()),
                event = receiver.recv() => match event {
                    Some(event) => {
                        let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
                        let terminal = matches!(
                            kind,
                            "session.idle"
                                | "session.execution.succeeded"
                                | "session.execution.failed"
                        );
                        if !terminal
                            || !crate::opencode_client::event_stream::session_matches(
                                &event,
                                session_id,
                            )
                        {
                            continue;
                        }
                        terminal_event = true;
                    }
                    None => closed = true,
                },
                _ = tokio::time::sleep(fallback) => {}
            }
        } else {
            tokio::select! {
                _ = cancel.cancelled() => return Err(CANCEL_MESSAGE.into()),
                _ = tokio::time::sleep(fallback) => {}
            }
        }
        if closed {
            events = None;
        }

        let value = client
            .messages(session_id)
            .await
            .map_err(|error| format!("could not read session messages: {error:#}"))?;
        let signature = turn_signature(&value, since_ms);
        let has_text = turn_has_text(&value, since_ms);
        let has_idle = turn_has_idle(&value, since_ms);

        // A terminal SSE event is authoritative even if the message list has
        // not materialized its idle marker yet, as long as the text arrived.
        if has_text && (has_idle || terminal_event) {
            return Ok(value);
        }
        if (has_idle || terminal_event) && !has_text {
            let since = *idle_without_text_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= IDLE_WITHOUT_TEXT_GRACE {
                return Ok(value);
            }
        } else {
            idle_without_text_since = None;
        }

        if !has_text {
            stable_signature.clear();
            stable_since = None;
        } else if signature == stable_signature {
            if stable_since.is_some_and(|since| since.elapsed() >= TEXT_STABLE_GRACE) {
                return Ok(value);
            }
        } else {
            stable_signature = signature;
            stable_since = Some(std::time::Instant::now());
        }
    }
}
