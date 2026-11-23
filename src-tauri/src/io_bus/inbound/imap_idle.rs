use crate::{
    app_state::AppState,
    db::io_bus::{self, InboundSource},
    error::{AppError, AppResult},
    io_bus::{NormalizedTaskEvent, source_config},
};
use async_imap::Client;
use futures_util::TryStreamExt;
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize)]
pub struct ImapConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password_ref: Option<String>,
    #[serde(default = "default_mailbox")]
    pub mailbox: String,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    /// Deprecated: the connection always uses implicit TLS. Kept so old configs
    /// keep parsing; it never disables certificate validation.
    #[serde(default = "default_true")]
    pub tls: bool,
    /// Explicit, insecure opt-in for self-signed certificates: the IMAP
    /// password can be intercepted by anyone able to impersonate the server.
    #[serde(default)]
    pub allow_invalid_certs: bool,
}

/// Certificate validation is the default and cannot be disabled by the legacy
/// `tls` flag. Only the explicit `allow_invalid_certs` opt-in turns it off.
fn validate_certificates(config: &ImapConfig) -> bool {
    !config.allow_invalid_certs
}
fn default_port() -> u16 {
    993
}
fn default_mailbox() -> String {
    "INBOX".into()
}
fn default_interval() -> u64 {
    60
}
fn default_true() -> bool {
    true
}

pub async fn run(
    state: Arc<AppState>,
    source: InboundSource,
    cancel: CancellationToken,
) -> AppResult<()> {
    let config: ImapConfig = source_config(&source)?;
    if !config.tls {
        tracing::warn!(
            source = %source.id,
            "IMAP tls=false ignorado: a conexão usa TLS; a validação de certificado segue ativa"
        );
    }
    // We reconnect on every cycle. This keeps IDLE semantics (new mail is
    // picked up promptly) while surviving NAT/proxy idle timeouts; the interval
    // is the reconnect fallback, not a polling-only limitation.
    loop {
        if cancel.is_cancelled() {
            break;
        }
        if let Err(error) = poll_once(&state, &source, &config).await {
            tracing::warn!(source = %source.id, error = %error, "IMAP cycle failed");
            let _ =
                io_bus::mark_inbound(&state.db, &source.id, "FAILED", Some(&format!("{error:#}")))
                    .await;
        }
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(config.interval_secs.clamp(15, 3600))) => {}
        }
    }
    Ok(())
}

async fn poll_once(
    state: &Arc<AppState>,
    source: &InboundSource,
    config: &ImapConfig,
) -> AppResult<()> {
    let password = password(state, config)?;
    let stream = tokio::net::TcpStream::connect((config.host.as_str(), config.port))
        .await
        .map_err(|error| AppError::Daemon(format!("IMAP connect: {error}")))?;
    let mut builder = native_tls::TlsConnector::builder();
    if !validate_certificates(config) {
        tracing::warn!(
            source = %source.id,
            "IMAP com validação de certificado desativada (allow_invalid_certs)"
        );
        builder.danger_accept_invalid_certs(true);
    }
    let connector = tokio_native_tls::TlsConnector::from(
        builder
            .build()
            .map_err(|error| AppError::Daemon(format!("IMAP TLS config: {error}")))?,
    );
    let stream = connector
        .connect(&config.host, stream)
        .await
        .map_err(|error| AppError::Daemon(format!("IMAP TLS: {error}")))?;
    let client = Client::new(stream);
    let mut session = client
        .login(&config.username, &password)
        .await
        .map_err(|(error, _)| AppError::Daemon(format!("IMAP login: {error}")))?;
    session
        .select(&config.mailbox)
        .await
        .map_err(|error| AppError::Daemon(format!("IMAP select: {error}")))?;
    let unseen = session
        .search("UNSEEN")
        .await
        .map_err(|error| AppError::Daemon(format!("IMAP search: {error}")))?;
    for sequence in unseen {
        let messages = session
            .fetch(sequence.to_string(), "(RFC822)")
            .await
            .map_err(|error| AppError::Daemon(format!("IMAP fetch: {error}")))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|error| AppError::Daemon(format!("IMAP fetch stream: {error}")))?;
        for message in messages {
            let Some(body) = message.body() else {
                continue;
            };
            let parsed = mail_parser::MessageParser::default().parse(body);
            let Some(parsed) = parsed else {
                continue;
            };
            let subject = parsed
                .subject()
                .map(str::to_string)
                .unwrap_or_else(|| "E-mail recebido".into());
            let from = parsed
                .from()
                .and_then(|address| address.first())
                .and_then(|address| address.address())
                .unwrap_or("desconhecido")
                .to_string();
            let text = parsed
                .body_text(0)
                .map(|body| body.to_string())
                .or_else(|| {
                    parsed
                        .body_html(0)
                        .and_then(|html| html2text::from_read(html.as_bytes(), 100).ok())
                })
                .unwrap_or_default();
            let message_id = parsed
                .message_id()
                .map(str::to_string)
                .unwrap_or_else(|| format!("{subject}:{from}:{}", message.message));
            let in_reply_to = parsed
                .in_reply_to()
                .map(|values| values.iter().collect::<Vec<_>>().join(" "));
            let references = parsed
                .references()
                .map(|values| values.iter().collect::<Vec<_>>().join(" "));
            let workspace = crate::db::workspaces::get(&state.db, &source.workspace_id).await?;
            let root = PathBuf::from(&workspace.root_path);
            let attachment_dir = root.join("input").join("attachments").join(
                message_id
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect::<String>(),
            );
            std::fs::create_dir_all(&attachment_dir)?;
            let mut attachments = Vec::new();
            for attachment in parsed.attachments() {
                let Some(name) = attachment.attachment_name() else {
                    continue;
                };
                let bytes = attachment.raw_body();
                let path = attachment_dir.join(name);
                std::fs::write(&path, bytes)?;
                if let Ok(relative) = path.strip_prefix(&root) {
                    attachments.push(relative.to_string_lossy().to_string());
                }
            }
            if crate::recipes::hitl::extract_token(&subject).is_some()
                && crate::recipes::hitl::email::handle_reply(
                    &crate::app_state::recipe_context(state),
                    &subject,
                    &from,
                    &text,
                    &attachments,
                )
                .await
                .unwrap_or(false)
            {
                let _ = session.store(sequence.to_string(), "+FLAGS (\\Seen)").await;
                continue;
            }
            save_email_chat(
                state,
                source,
                &message_id,
                in_reply_to.as_deref().unwrap_or(""),
                &subject,
                &from,
                &text,
                &attachments,
            )
            .await?;
            let event = NormalizedTaskEvent {
                source_id: source.id.clone(),
                external_id: message_id.clone(),
                title: subject.clone(),
                body_markdown: format!("# {from}\n\n{text}"),
                priority: None,
                tags: vec!["email".into()],
                attachments,
                metadata: serde_json::json!({
                    "from": from,
                    "subject": subject,
                    "message_id": message_id,
                    "in_reply_to": in_reply_to,
                    "references": references,
                }),
            };
            crate::io_bus::ingest(state, source, event).await?;
        }
        let _ = session.store(sequence.to_string(), "+FLAGS (\\Seen)").await;
    }
    let _ = io_bus::mark_inbound(&state.db, &source.id, "TRIGGERED", None).await;
    let _ = session.logout().await;
    Ok(())
}

/// Makes inbound mail visible in Chat. The task ingestion continues below; the
/// thread keeps the Message-ID so a reply from Chat preserves the mail thread.
#[allow(clippy::too_many_arguments)]
async fn save_email_chat(
    state: &Arc<AppState>,
    source: &InboundSource,
    message_id: &str,
    in_reply_to: &str,
    subject: &str,
    from: &str,
    text: &str,
    attachments: &[String],
) -> AppResult<()> {
    let candidate = in_reply_to
        .split_whitespace()
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(message_id);
    let mut thread =
        crate::db::chat::find_external(&state.db, &source.workspace_id, "email", candidate).await?;
    if thread.is_none() && candidate != message_id {
        thread =
            crate::db::chat::find_external(&state.db, &source.workspace_id, "email", message_id)
                .await?;
    }
    let thread = match thread {
        Some(thread) => thread,
        None => {
            crate::db::chat::create_thread(
                &state.db,
                crate::db::chat::CreateThreadInput {
                    workspace_id: source.workspace_id.clone(),
                    title: Some(format!("E-mail · {subject}")),
                    agent: "build".into(),
                    model: None,
                    source: Some("email".into()),
                    external_id: Some(message_id.to_string()),
                },
            )
            .await?
        }
    };
    let metadata = serde_json::json!({
        "from": from,
        "subject": subject,
        "message_id": message_id,
        "in_reply_to": in_reply_to,
        "attachments": attachments,
    });
    crate::db::chat::add_message(
        &state.db,
        &thread.id,
        "user",
        &format!("De: {from}\nAssunto: {subject}\n\n{text}"),
        Some(&metadata.to_string()),
    )
    .await?;
    state
        .events
        .publish(crate::events::OpenDeskWsEvent::ChatUpdated {
            thread_id: thread.id,
            workspace_id: source.workspace_id.clone(),
        });
    Ok(())
}

fn password(state: &Arc<AppState>, config: &ImapConfig) -> AppResult<String> {
    crate::io_bus::secret(&state.paths, config.password_ref.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    fn config(json: &str) -> ImapConfig {
        serde_json::from_str(json).expect("config")
    }

    #[test]
    fn legacy_tls_flag_never_disables_certificate_validation() {
        let legacy = config(r#"{"host":"imap.example","username":"u","tls":false}"#);
        assert!(!legacy.tls);
        assert!(validate_certificates(&legacy));

        let insecure =
            config(r#"{"host":"imap.example","username":"u","allow_invalid_certs":true}"#);
        assert!(!validate_certificates(&insecure));

        let default = config(r#"{"host":"imap.example","username":"u"}"#);
        assert!(default.tls);
        assert!(!default.allow_invalid_certs);
        assert!(validate_certificates(&default));
    }

    #[tokio::test]
    async fn inbound_email_creates_one_chat_thread_and_keeps_replies_together() {
        let ctx = test_support::context().await;
        let source = InboundSource {
            id: "imap-1".into(),
            workspace_id: ctx.workspace_id.clone(),
            name: "IMAP".into(),
            source_type: "IMAP".into(),
            config_json: "{}".into(),
            target_agent: None,
            target_column: "TODO".into(),
            target_kind: "TASK".into(),
            target_id: None,
            auto_execute: false,
            is_enabled: true,
            last_triggered_at: None,
            last_status: None,
            last_error: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        save_email_chat(
            &ctx.state,
            &source,
            "<root@example.com>",
            "",
            "Primeiro",
            "user@example.com",
            "olá",
            &[],
        )
        .await
        .expect("first");
        save_email_chat(
            &ctx.state,
            &source,
            "<reply@example.com>",
            "<root@example.com>",
            "Re: Primeiro",
            "user@example.com",
            "resposta",
            &[],
        )
        .await
        .expect("reply");
        let threads =
            crate::db::chat::list_threads(&ctx.state.db, &ctx.workspace_id, Some("email"))
                .await
                .expect("threads");
        assert_eq!(threads.len(), 1);
        let messages = crate::db::chat::list_messages(&ctx.state.db, &threads[0].id)
            .await
            .expect("messages");
        assert_eq!(messages.len(), 2);
        assert!(messages[0].content.contains("olá"));
        assert!(messages[1].content.contains("resposta"));
    }
}
