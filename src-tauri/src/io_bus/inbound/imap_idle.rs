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
    pub password: Option<String>,
    #[serde(default)]
    pub password_ref: Option<String>,
    #[serde(default = "default_mailbox")]
    pub mailbox: String,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    #[serde(default = "default_true")]
    pub tls: bool,
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
    let connector = tokio_native_tls::TlsConnector::from(
        native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(!config.tls)
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
                    state,
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

fn password(state: &Arc<AppState>, config: &ImapConfig) -> AppResult<String> {
    crate::io_bus::secret(
        &state.paths,
        config.password_ref.as_deref(),
        config.password.as_deref(),
    )
}
