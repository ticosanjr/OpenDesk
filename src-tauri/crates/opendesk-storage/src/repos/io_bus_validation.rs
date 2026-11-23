//! Validation and destination resolution for I/O bus inputs.
//!
//! Kept beside `io_bus.rs` so the repository facade stays under the 500-line
//! limit. The rules are the same ones enforced by the SQLite CHECK constraints.

use super::InboundInput;
use crate::error::{AppError, AppResult};

pub(super) fn source_type(value: &str) -> AppResult<String> {
    let value = value.trim().to_uppercase();
    if value == "RSYNC_FOLDER" {
        return Ok("FOLDER_WATCH".into());
    }
    const TYPES: [&str; 10] = [
        "FOLDER_WATCH",
        "OCR_WATCH",
        "AUDIO_STT_WATCH",
        "IMAP_IDLE",
        "WEBHOOK_HTTP",
        "SQL_POLLER",
        "HTTP_POLLER",
        "TELEGRAM_BOT",
        "MATRIX_BOT",
        "MATTERMOST_BOT",
    ];
    if TYPES.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "source_type inválido '{value}'"
        )))
    }
}

pub(super) fn webhook_secret_ref(config: &serde_json::Value) -> Option<String> {
    config
        .get("vault_ref")
        .or_else(|| config.get("secret_ref"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// A webhook without a shared secret would accept any request, including one
/// that starts an agent run through `auto_execute`. Refuse to persist one.
pub(super) fn ensure_webhook_secret_ref(
    source_type: &str,
    config: &serde_json::Value,
) -> AppResult<()> {
    if source_type == "WEBHOOK_HTTP" && webhook_secret_ref(config).is_none() {
        return Err(AppError::BadRequest(
            "webhook HTTP exige segredo; informe config.secret ou config.secret_ref".into(),
        ));
    }
    Ok(())
}

pub(super) fn sink_type(value: &str) -> AppResult<String> {
    let value = value.trim().to_uppercase();
    if value == "RSYNC_FOLDER" {
        return Ok("FOLDER_WATCH".into());
    }
    const TYPES: [&str; 14] = [
        "ATOMIC_FS",
        "MINIO_S3",
        "SMTP_EMAIL",
        "SIGNAL_MSG",
        "TELEGRAM_MSG",
        "MATRIX_MSG",
        "MATTERMOST_MSG",
        "DOC_COMPILER",
        "GIT_COMMIT",
        "SQL_INSERT",
        "RSYNC_FS",
        "HTTP_WEBHOOK",
        "WHATSAPP_MSG",
        "WEBPUSH",
    ];
    if TYPES.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "sink_type inválido '{value}'"
        )))
    }
}

pub(super) fn target_column(value: Option<&str>) -> AppResult<String> {
    let value = value.unwrap_or("TODO").trim().to_uppercase();
    let value = if value == "BACKLOG" {
        "TODO".to_string()
    } else {
        value
    };
    const STATUSES: [&str; 6] = [
        "TODO",
        "IN_PROGRESS",
        "IN_REVIEW",
        "DONE",
        "CANCELLED",
        "BLOCKED",
    ];
    if STATUSES.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "target_column inválida '{value}'"
        )))
    }
}

fn target_kind(value: Option<&str>) -> AppResult<String> {
    let value = value.unwrap_or("TASK").trim().to_uppercase();
    const KINDS: [&str; 4] = ["TASK", "RECIPE", "TICKET", "SCHEDULE"];
    if KINDS.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "target_kind inválido '{value}'"
        )))
    }
}

fn config_target(config: &serde_json::Value) -> Option<(String, String)> {
    if let Some(id) = config
        .get("schedule_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(("SCHEDULE".into(), id.to_string()));
    }
    if let Some(id) = config
        .get("recipe_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(("RECIPE".into(), id.to_string()));
    }
    None
}

/// Resolves the explicit destination. `config_json` with `recipe_id` or
/// `schedule_id` is still accepted and backfilled into the typed columns.
pub(super) fn resolve_target(input: &InboundInput) -> AppResult<(String, Option<String>)> {
    let inferred = config_target(&input.config);
    let kind = match input.target_kind.as_deref() {
        Some(value) => target_kind(Some(value))?,
        None => inferred
            .as_ref()
            .map(|(kind, _)| kind.clone())
            .unwrap_or_else(|| "TASK".into()),
    };
    let explicit_id = input
        .target_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if kind == "TASK" {
        if inferred.is_some() {
            return Err(AppError::BadRequest(
                "config contém recipe_id/schedule_id; use target_kind RECIPE ou SCHEDULE".into(),
            ));
        }
        return Ok((kind, None));
    }
    let id = explicit_id.or_else(|| inferred.map(|(_, id)| id));
    match id {
        Some(id) => Ok((kind, Some(id))),
        None => Err(AppError::BadRequest(format!(
            "target_kind {kind} exige target_id"
        ))),
    }
}
