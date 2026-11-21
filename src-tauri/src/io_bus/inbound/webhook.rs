use crate::{
    app_state::AppState,
    db::io_bus::{self, InboundSource},
    error::{AppError, AppResult},
    io_bus::{NormalizedTaskEvent, WEBHOOK_SECRET_NAMESPACE, source_config},
};
use axum::http::HeaderMap;
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use serde_json_path::JsonPath;
use sha2::Sha256;
use std::sync::Arc;

#[derive(Debug, Default, Deserialize)]
pub struct WebhookConfig {
    #[serde(default)]
    pub secret_ref: Option<String>,
    #[serde(default = "default_title_path")]
    pub title_path: String,
    #[serde(default = "default_body_path")]
    pub body_path: String,
    #[serde(default)]
    pub priority_path: Option<String>,
    #[serde(default)]
    pub external_id_path: Option<String>,
    #[serde(default)]
    pub signature_header: Option<String>,
}

fn default_title_path() -> String {
    "$.title".into()
}
fn default_body_path() -> String {
    "$.body".into()
}

pub async fn handle(
    state: &Arc<AppState>,
    source: InboundSource,
    headers: &HeaderMap,
    body: &str,
) -> AppResult<serde_json::Value> {
    if !source.is_enabled {
        return Err(AppError::BadRequest(format!(
            "fonte webhook {} está desativada",
            source.id
        )));
    }
    let config: WebhookConfig = source_config(&source)?;
    // No secret means no authentication. Refuse instead of accepting anonymous
    // requests that could create tasks and start agent runs.
    let secret_ref = config
        .secret_ref
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::BadRequest(
                "webhook sem segredo configurado; defina secret_ref na fonte".into(),
            )
        })?;
    let secret = crate::secrets::SecretStore::new(&state.paths)
        .get(WEBHOOK_SECRET_NAMESPACE, secret_ref)?
        .ok_or_else(|| AppError::BadRequest("segredo do webhook ausente".into()))?;
    let header = config
        .signature_header
        .as_deref()
        .unwrap_or("x-opendesk-signature");
    let signature = headers
        .get(header)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("assinatura do webhook ausente".into()))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::Internal("HMAC inválido".into()))?;
    mac.update(body.as_bytes());
    let expected = hex(&mac.finalize().into_bytes());
    if !constant_time_eq(signature.trim(), &expected) {
        return Err(AppError::BadRequest(
            "assinatura do webhook inválida".into(),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| AppError::BadRequest(format!("JSON inválido: {error}")))?;
    let title = path_string(&value, &config.title_path)
        .or_else(|| {
            value
                .get("title")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Webhook recebido".into());
    let body_markdown = path_string(&value, &config.body_path)
        .or_else(|| {
            value
                .get("body")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "```json\n".to_string() + &value.to_string() + "\n```");
    let priority = config
        .priority_path
        .as_deref()
        .and_then(|path| path_value(&value, path))
        .and_then(|value| value.as_i64());
    let external_id = config
        .external_id_path
        .as_deref()
        .and_then(|path| path_string(&value, path))
        .unwrap_or_else(|| hash(body));
    let event = NormalizedTaskEvent {
        source_id: source.id.clone(),
        external_id,
        title,
        body_markdown,
        priority,
        tags: vec!["webhook".into()],
        attachments: vec![],
        metadata: value,
    };
    let task = crate::io_bus::ingest(state, &source, event).await?;
    let _ = io_bus::mark_inbound(&state.db, &source.id, "TRIGGERED", None).await;
    Ok(serde_json::json!({
        "ok": true,
        "task_id": task.as_ref().map(|task| task.id.clone()),
    }))
}

fn path_value(value: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let path = JsonPath::parse(path).ok()?;
    path.query(value)
        .all()
        .first()
        .map(|value| (*value).clone())
}

fn path_string(value: &serde_json::Value, path: &str) -> Option<String> {
    let value = path_value(value, path)?;
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| Some(value.to_string()))
}

fn hash(value: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0u8, |accumulator, (a, b)| accumulator | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_json_paths_and_hashes() {
        let value = json!({"ticket": {"subject": "Oi", "priority": 3}});
        assert_eq!(
            path_string(&value, "$.ticket.subject").as_deref(),
            Some("Oi")
        );
        assert_eq!(
            path_value(&value, "$.ticket.priority").and_then(|value| value.as_i64()),
            Some(3)
        );
        assert_eq!(hash("x"), hash("x"));
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
    }
}
