use crate::{
    db::integrations,
    error::{AppError, AppResult},
    secrets::SecretStore,
};
use anyhow::{Context, Result};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Attachment, Message, MultiPart, SinglePart, header::ContentType},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: i64,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_email: String,
    pub use_tls: bool,
}

#[derive(Clone)]
pub struct EmailAttachment {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub mime: String,
}

pub async fn load_config(
    pool: &SqlitePool,
    secrets: &SecretStore,
    workspace_id: &str,
) -> AppResult<Option<SmtpConfig>> {
    let Some(settings) = integrations::get(pool, workspace_id).await? else {
        return Ok(None);
    };
    let Some(host) = settings.smtp_host.filter(|host| !host.trim().is_empty()) else {
        return Ok(None);
    };
    let Some(from_email) = settings
        .smtp_from_email
        .filter(|email| !email.trim().is_empty())
    else {
        return Ok(None);
    };
    let password = secrets.get("smtp", workspace_id)?;

    Ok(Some(SmtpConfig {
        host,
        port: settings.smtp_port,
        username: settings.smtp_username,
        password,
        from_email,
        use_tls: settings.smtp_use_tls,
    }))
}

pub fn build_transport(config: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let mut builder = if config.port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .context("invalid SMTP relay")?
            .port(config.port as u16)
            .tls(Tls::Wrapper(
                TlsParameters::new(config.host.clone()).context("invalid TLS parameters")?,
            ))
    } else if config.use_tls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .context("invalid STARTTLS relay")?
            .port(config.port as u16)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            .port(config.port as u16)
    };

    if let (Some(username), Some(password)) = (&config.username, &config.password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }

    Ok(builder.build())
}

pub async fn test_connection(config: &SmtpConfig) -> AppResult<()> {
    let transport = build_transport(config)
        .map_err(|error| AppError::BadRequest(format!("SMTP configuration: {error:#}")))?;
    transport
        .test_connection()
        .await
        .map_err(|error| AppError::Daemon(format!("SMTP connection failed: {error}")))?;
    Ok(())
}

pub async fn send_email(
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    html: String,
    attachments: Vec<EmailAttachment>,
) -> Result<()> {
    let from = config
        .from_email
        .parse()
        .context("invalid SMTP from address")?;
    let to = to.parse().context("invalid recipient address")?;

    let mut body =
        MultiPart::mixed().multipart(MultiPart::alternative().singlepart(SinglePart::html(html)));
    for attachment in attachments {
        let content_type = ContentType::parse(&attachment.mime)
            .unwrap_or(ContentType::parse("application/octet-stream").expect("content type"));
        body = body
            .singlepart(Attachment::new(attachment.filename).body(attachment.bytes, content_type));
    }

    let email = Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .multipart(body)
        .context("could not build email")?;

    let transport = build_transport(config)?;
    transport.send(email).await.context("SMTP send failed")?;
    Ok(())
}

pub fn attachment_limit(bytes: usize) -> bool {
    bytes <= 10 * 1024 * 1024
}
