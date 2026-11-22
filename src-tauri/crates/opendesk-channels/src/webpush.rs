//! Web Push (VAPID) delivery for the PWA.
//!
//! The desktop app keeps using native notifications. This module lets the
//! headless daemon notify a phone/browser even when no WebSocket is connected:
//! the frontend registers a `PushSubscription`, OpenDesk stores it in SQLite,
//! and the event bus fans relevant events out to every registered device.
//!
//! VAPID signing keys are generated once and stored in `push_vapid.json` with
//! mode 0600; raw subscriptions keys never leave the database.

use crate::error::{AppError, AppResult};
use opendesk_storage::events::OpenDeskWsEvent;
use opendesk_storage::paths::AppPaths;
use opendesk_storage::repos::push;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::{SecretKey, elliptic_curve::sec1::ToEncodedPoint};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use web_push::{
    ContentEncoding, HyperWebPushClient, SubscriptionInfo, Urgency, VapidSignatureBuilder,
    WebPushClient, WebPushMessageBuilder,
};

const VAPID_FILE: &str = "push_vapid.json";
/// VAPID `sub` claim. Push services only use it to contact the operator.
const VAPID_SUBJECT: &str = "mailto:opendesk@localhost";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VapidKeys {
    /// Raw 32-byte P-256 scalar, base64url without padding.
    pub private_key: String,
    /// Uncompressed public point (65 bytes), base64url without padding.
    /// This is the `applicationServerKey` a browser subscription needs.
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushPayload {
    pub title: String,
    pub body: String,
    #[serde(default = "default_url")]
    pub url: String,
    #[serde(default)]
    pub tag: Option<String>,
}

fn default_url() -> String {
    "/".into()
}

impl PushPayload {
    pub fn new(title: impl Into<String>, body: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            url: url.into(),
            tag: None,
        }
    }

    fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

fn generate_keys() -> VapidKeys {
    let secret = SecretKey::random(&mut OsRng);
    let public = secret.public_key().to_encoded_point(false);
    VapidKeys {
        private_key: URL_SAFE_NO_PAD.encode(secret.to_bytes()),
        public_key: URL_SAFE_NO_PAD.encode(public.as_bytes()),
    }
}

/// Loads the daemon's VAPID key pair, creating it on first run.
pub fn load_or_create_keys(paths: &AppPaths) -> AppResult<VapidKeys> {
    let path = paths.data_dir.join(VAPID_FILE);
    if path.is_file() {
        let raw = std::fs::read_to_string(&path)?;
        return serde_json::from_str(&raw)
            .map_err(|error| AppError::Internal(format!("push_vapid.json inválido: {error}")));
    }

    let keys = generate_keys();
    let bytes = serde_json::to_vec_pretty(&keys).map_err(|error| {
        AppError::Internal(format!("não foi possível serializar VAPID: {error}"))
    })?;
    std::fs::write(&path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(keys)
}

/// Sends one payload to a single subscription.
async fn deliver(
    client: &HyperWebPushClient,
    keys: &VapidKeys,
    subscription: &push::PushSubscription,
    payload: &PushPayload,
) -> Result<(), web_push::WebPushError> {
    let info = SubscriptionInfo::new(
        subscription.endpoint.clone(),
        subscription.p256dh.clone(),
        subscription.auth.clone(),
    );
    let mut signature_builder = VapidSignatureBuilder::from_base64(&keys.private_key, &info)?;
    signature_builder.add_claim("sub", VAPID_SUBJECT);
    let signature = signature_builder.build()?;
    let body = serde_json::to_vec(payload).map_err(|_| web_push::WebPushError::Unspecified)?;
    let mut builder = WebPushMessageBuilder::new(&info);
    builder.set_ttl(60 * 60);
    builder.set_urgency(Urgency::Normal);
    if let Some(tag) = payload.tag.as_deref() {
        builder.set_topic(tag.to_string());
    }
    builder.set_vapid_signature(signature);
    builder.set_payload(ContentEncoding::Aes128Gcm, &body);
    client.send(builder.build()?).await
}

/// Fan-out used by the API test button. Returns how many devices accepted it.
pub async fn send_to_all(
    pool: &SqlitePool,
    paths: &AppPaths,
    payload: &PushPayload,
) -> AppResult<usize> {
    let keys = load_or_create_keys(paths)?;
    send_with_keys(pool, &keys, payload).await
}

async fn send_with_keys(
    pool: &sqlx::SqlitePool,
    keys: &VapidKeys,
    payload: &PushPayload,
) -> AppResult<usize> {
    let subscriptions = push::list(pool).await?;
    if subscriptions.is_empty() {
        return Ok(0);
    }
    let client = HyperWebPushClient::new();
    let mut delivered = 0;
    for subscription in subscriptions {
        match deliver(&client, keys, &subscription, payload).await {
            Ok(()) => {
                delivered += 1;
                push::mark_success(pool, &subscription.id).await?;
            }
            Err(error) => {
                if matches!(
                    error,
                    web_push::WebPushError::EndpointNotFound(_)
                        | web_push::WebPushError::EndpointNotValid(_)
                ) {
                    let _ = push::remove_by_id(pool, &subscription.id).await;
                } else {
                    let failures = push::mark_failure(pool, &subscription.id).await?;
                    tracing::warn!(
                        subscription = %subscription.id,
                        failures,
                        error = %error,
                        "web push delivery failed"
                    );
                }
            }
        }
    }
    Ok(delivered)
}

/// Maps bus events worth interrupting the user for into push payloads.
fn payload_for(event: &OpenDeskWsEvent) -> Option<PushPayload> {
    match event {
        OpenDeskWsEvent::HitlRequested {
            token,
            run_id,
            interaction_type,
            ..
        } => Some(
            PushPayload::new(
                "Aprovação pendente",
                format!("A execução {run_id} aguarda uma decisão ({interaction_type})."),
                format!("/#/executions?interaction={token}"),
            )
            .with_tag(format!("hitl-{run_id}")),
        ),
        OpenDeskWsEvent::PermissionRequested {
            request_id,
            action,
            run_id,
            ..
        } => Some(
            PushPayload::new(
                "Permissão solicitada",
                format!("{action} — execução {run_id}."),
                format!("/#/executions?permission={request_id}"),
            )
            .with_tag(format!("permission-{run_id}")),
        ),
        OpenDeskWsEvent::RunFinished {
            run_id,
            status,
            estimated_cost_usd,
            ..
        } => {
            let failed = !matches!(status.as_str(), "success" | "completed" | "concluido");
            Some(
                PushPayload::new(
                    if failed {
                        "Execução falhou"
                    } else {
                        "Execução concluída"
                    },
                    format!("Run {run_id} terminou como {status} (US$ {estimated_cost_usd:.2})."),
                    format!("/#/executions?run={run_id}"),
                )
                .with_tag(format!("run-{run_id}")),
            )
        }
        OpenDeskWsEvent::HandoffUpdated {
            handoff_id,
            status,
            target_agent,
            ..
        } if status == "PENDING" => Some(
            PushPayload::new(
                "Handoff aguardando aceite",
                format!("{target_agent} recebeu uma delegação."),
                format!("/#/governance?handoff={handoff_id}"),
            )
            .with_tag(format!("handoff-{handoff_id}")),
        ),
        OpenDeskWsEvent::TicketUpdated {
            ticket_id,
            source,
            status,
            ..
        } if status == "OPEN" => Some(
            PushPayload::new(
                "Novo ticket",
                format!("Mensagem recebida via {source}."),
                format!("/#/integrations?section=channels&ticket={ticket_id}"),
            )
            .with_tag(format!("ticket-{ticket_id}")),
        ),
        OpenDeskWsEvent::ProposalCreated {
            proposal_id,
            title,
            kind,
            ..
        } => Some(
            PushPayload::new(
                "Nova proposta",
                format!("{kind}: {title}"),
                format!("/#/proposals?proposal={proposal_id}"),
            )
            .with_tag(format!("proposal-{proposal_id}")),
        ),
        OpenDeskWsEvent::BudgetAlert { level, message, .. } => Some(
            PushPayload::new(
                "Alerta de orçamento",
                format!("{level}: {message}"),
                "/#/settings".to_string(),
            )
            .with_tag("budget-alert"),
        ),
        _ => None,
    }
}

/// Background dispatcher. Lagged receivers only mean bursts faster than push;
/// the UI still receives every event over WebSocket.
pub fn start(pool: SqlitePool, paths: AppPaths, events: opendesk_storage::events::EventBus) {
    tokio::spawn(async move {
        let mut receiver = events.subscribe();
        loop {
            match receiver.recv().await {
                Ok(envelope) => {
                    let Some(payload) = payload_for(&envelope.event) else {
                        continue;
                    };
                    let Ok(keys) = load_or_create_keys(&paths) else {
                        continue;
                    };
                    if let Err(error) = send_with_keys(&pool, &keys, &payload).await {
                        tracing::warn!(error = %error, "web push fan-out failed");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_keys_are_url_safe_and_complete() {
        let keys = generate_keys();
        let private = URL_SAFE_NO_PAD
            .decode(&keys.private_key)
            .expect("private key");
        assert_eq!(private.len(), 32);
        let public = URL_SAFE_NO_PAD
            .decode(&keys.public_key)
            .expect("public key");
        assert_eq!(public.len(), 65);
        assert_eq!(public[0], 0x04);
    }

    #[test]
    fn maps_hitl_and_run_events_to_payloads() {
        let hitl = OpenDeskWsEvent::HitlRequested {
            token: "tok".into(),
            run_id: "run-1".into(),
            workspace_id: "ws".into(),
            checkpoint_id: "cp".into(),
            interaction_type: "APPROVAL".into(),
        };
        let payload = payload_for(&hitl).expect("hitl payload");
        assert_eq!(payload.title, "Aprovação pendente");
        assert!(payload.url.contains("interaction=tok"));

        let finished = OpenDeskWsEvent::RunFinished {
            run_id: "run-2".into(),
            task_id: None,
            status: "failed".into(),
            estimated_cost_usd: 0.5,
            tokens_input: 10,
            tokens_output: 5,
        };
        let payload = payload_for(&finished).expect("run payload");
        assert_eq!(payload.title, "Execução falhou");
        assert_eq!(payload.tag.as_deref(), Some("run-run-2"));

        assert!(payload_for(&OpenDeskWsEvent::ResyncRequired { reason: "x".into() }).is_none());
    }

    #[test]
    fn keys_are_persisted_with_restrictive_permissions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = opendesk_storage::paths::test_paths(dir.path());
        let first = load_or_create_keys(&paths).expect("create");
        let second = load_or_create_keys(&paths).expect("load");
        assert_eq!(first.private_key, second.private_key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(paths.data_dir.join(VAPID_FILE))
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
