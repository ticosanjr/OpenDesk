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
        host: host.trim().to_string(),
        port: settings.smtp_port,
        username: settings
            .smtp_username
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        password,
        from_email: from_email.trim().to_string(),
        use_tls: settings.smtp_use_tls,
    }))
}

pub fn build_transport(config: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let host = config.host.trim().to_string();
    // Gmail (and most providers) only advertise AUTH after STARTTLS. Port 587
    // is treated as STARTTLS whenever credentials are present, even if an old
    // workspace saved `use_tls = false`.
    let starttls =
        config.port != 465 && (config.use_tls || (config.port == 587 && config.username.is_some()));
    let mut builder = if config.port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
            .context("invalid SMTP relay")?
            .port(config.port as u16)
            .tls(Tls::Wrapper(
                TlsParameters::new(host.clone()).context("invalid TLS parameters")?,
            ))
    } else if starttls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
            .context("invalid STARTTLS relay")?
            .port(config.port as u16)
            .tls(Tls::Required(
                TlsParameters::new(host.clone()).context("invalid TLS parameters")?,
            ))
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host).port(config.port as u16)
    };

    if let Some((username, password)) =
        normalized_credentials(config.username.as_deref(), config.password.as_deref())
    {
        builder = builder.credentials(Credentials::new(username, password));
    }

    Ok(builder.build())
}

fn normalized_credentials(
    username: Option<&str>,
    password: Option<&str>,
) -> Option<(String, String)> {
    // Gmail displays App Passwords with spaces; SMTP AUTH expects none.
    let username = username?.trim();
    let password = password?.trim();
    if username.is_empty() || password.is_empty() {
        return None;
    }
    // Gmail shows App Passwords as four groups of four characters. Only strip
    // those spaces; passwords for other SMTP servers may legitimately contain
    // spaces.
    let groups = password.split_whitespace().collect::<Vec<_>>();
    let password = if groups.len() == 4 && groups.iter().all(|group| group.len() == 4) {
        groups.concat()
    } else {
        password.to_string()
    };
    Some((username.to_string(), password))
}

pub async fn test_connection(config: &SmtpConfig) -> AppResult<()> {
    let transport = build_transport(config)
        .map_err(|error| AppError::BadRequest(format!("SMTP configuration: {error:#}")))?;
    transport
        .test_connection()
        .await
        .map_err(|error| {
            let detail = error.to_string();
            let hint = if detail.contains("authentication mechanism") {
                " O servidor não ofereceu AUTH antes do TLS. Use porta 587 com STARTTLS ou 465 com SSL e uma App Password."
            } else if detail.contains("authentication") || detail.contains("credentials") {
                " Confira usuário, App Password e o modo de segurança (587 STARTTLS / 465 SSL)."
            } else {
                ""
            };
            AppError::Daemon(format!("SMTP connection failed: {error}.{hint}"))
        })?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct EmailThread {
    pub in_reply_to: String,
    pub references: Vec<String>,
}

pub fn build_message(
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    html: String,
    attachments: Vec<EmailAttachment>,
    thread: Option<&EmailThread>,
) -> Result<Message> {
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

    let mut builder = Message::builder().from(from).to(to).subject(subject);
    if let Some(thread) = thread {
        let in_reply_to = thread.in_reply_to.trim();
        if !in_reply_to.is_empty() {
            builder = builder.in_reply_to(in_reply_to.to_string());
        }
        let references = if thread.references.is_empty() {
            in_reply_to.to_string()
        } else {
            thread.references.join(" ")
        };
        if !references.trim().is_empty() {
            builder = builder.references(references);
        }
    }
    builder.multipart(body).context("could not build email")
}

pub async fn send_email(
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    html: String,
    attachments: Vec<EmailAttachment>,
) -> Result<()> {
    send_email_threaded(config, to, subject, html, attachments, None).await
}

pub async fn send_email_threaded(
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    html: String,
    attachments: Vec<EmailAttachment>,
    thread: Option<&EmailThread>,
) -> Result<()> {
    let email = build_message(config, to, subject, html, attachments, thread)?;
    let transport = build_transport(config)?;
    transport.send(email).await.context("SMTP send failed")?;
    Ok(())
}

pub fn attachment_limit(bytes: usize) -> bool {
    bytes <= 10 * 1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(host: &str, port: i64, use_tls: bool, user: Option<&str>) -> SmtpConfig {
        SmtpConfig {
            host: host.into(),
            port,
            username: user.map(str::to_string),
            password: user.map(|_| "abcd efgh".into()),
            from_email: "ops@example.com".into(),
            use_tls,
        }
    }

    #[test]
    fn builds_both_tls_modes_and_plaintext() {
        assert!(build_transport(&config("smtp.gmail.com", 465, true, Some("u"))).is_ok());
        assert!(build_transport(&config("smtp.gmail.com", 587, false, Some("u"))).is_ok());
        assert!(build_transport(&config("localhost", 25, false, None)).is_ok());
    }

    #[test]
    fn thread_headers_are_preserved_in_the_message() {
        let message = build_message(
            &config("smtp.gmail.com", 587, false, Some("u")),
            "dest@example.com",
            "Re: assunto",
            "<p>ok</p>".into(),
            Vec::new(),
            Some(&EmailThread {
                in_reply_to: "<root@example.com>".into(),
                references: vec!["<root@example.com>".into()],
            }),
        )
        .expect("message");
        let formatted = String::from_utf8_lossy(&message.formatted()).to_string();
        assert!(
            formatted.contains("In-Reply-To: <root@example.com>"),
            "{formatted}"
        );
        assert!(
            formatted.contains("References: <root@example.com>"),
            "{formatted}"
        );
    }

    #[test]
    fn normalizes_gmail_app_passwords() {
        assert_eq!(
            normalized_credentials(Some(" user@example.com "), Some("abcd efgh ijkl mnop")),
            Some(("user@example.com".into(), "abcdefghijklmnop".into()))
        );
        assert_eq!(normalized_credentials(Some("user"), Some("   ")), None);
        assert_eq!(normalized_credentials(None, Some("secret")), None);
        assert_eq!(
            normalized_credentials(Some("user"), Some(" minha senha ")),
            Some(("user".into(), "minha senha".into()))
        );
    }
}
