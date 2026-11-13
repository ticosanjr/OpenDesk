use crate::{
    db::io_bus::OutboundSink,
    error::{AppError, AppResult},
    integrations::smtp_mailer,
    io_bus::{IoContext, RunOutput, config_value, outbound::artifact_paths},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    attach_artifacts: Option<bool>,
    #[serde(default)]
    token_ref: Option<String>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    chat_id: Option<String>,
    #[serde(default)]
    recipient: Option<String>,
    #[serde(default)]
    homeserver: Option<String>,
    #[serde(default)]
    room_id: Option<String>,
    #[serde(default)]
    webhook_url: Option<String>,
}

pub async fn run(
    ctx: &IoContext,
    sink: &OutboundSink,
    output: &RunOutput,
) -> AppResult<Vec<String>> {
    let config: Config = config_value(sink)?;
    let message = message_markdown(output);
    match sink.sink_type.as_str() {
        "SMTP_EMAIL" => smtp(ctx, &config, output, &message).await,
        "SIGNAL_MSG" => signal_channel(ctx, &config, &message, output).await,
        "TELEGRAM_MSG" => telegram(ctx, &config, &message).await,
        "MATRIX_MSG" => matrix(ctx, &config, &message).await,
        "MATTERMOST_MSG" => mattermost(&config, &message).await,
        other => Err(AppError::BadRequest(format!(
            "mensageria não suportada: {other}"
        ))),
    }
}

fn message_markdown(output: &RunOutput) -> String {
    let mut text = format!("## {}\n\n{}", output.title, output.summary.trim());
    for artifact in &output.artifacts {
        text.push_str(&format!("\n- `{}`", artifact.file_path));
    }
    text
}

fn secret(ctx: &IoContext, config: &Config) -> AppResult<String> {
    if let Some(token) = config
        .token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(token.to_string());
    }
    let key = config
        .token_ref
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("token_ref é obrigatório".into()))?;
    ctx.secrets()
        .get("io_bus", key)?
        .ok_or_else(|| AppError::NotFound(format!("segredo {key}")))
}

async fn smtp(
    ctx: &IoContext,
    config: &Config,
    output: &RunOutput,
    message: &str,
) -> AppResult<Vec<String>> {
    let to = match config
        .to
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => value.to_string(),
        None => crate::db::workspaces::get(&ctx.pool, &output.workspace_id)
            .await?
            .default_email_recipient
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                AppError::BadRequest("SMTP sink exige 'to' ou um e-mail padrão no workspace".into())
            })?,
    };
    let smtp = smtp_mailer::load_config(&ctx.pool, &ctx.secrets(), &output.workspace_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("SMTP não configurado no workspace".into()))?;
    let attachments = if config.attach_artifacts.unwrap_or(true) {
        artifact_paths(output)
            .into_iter()
            .take(5)
            .filter_map(|path| {
                let bytes = std::fs::read(path).ok()?;
                if !smtp_mailer::attachment_limit(bytes.len()) {
                    return None;
                }
                Some(smtp_mailer::EmailAttachment {
                    filename: path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("artifact")
                        .to_string(),
                    mime: crate::media::mime_for_path(path).to_string(),
                    bytes,
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let body = config
        .body
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|intro| format!("{intro}\n\n{message}"))
        .unwrap_or_else(|| message.to_string());
    let html = format!(
        "<html><body><pre style=\"white-space:pre-wrap\">{}</pre></body></html>",
        html_escape::encode_text(&body)
    );
    smtp_mailer::send_email(
        &smtp,
        &to,
        config
            .subject
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&format!("[OpenDesk] {}", output.title)),
        html,
        attachments,
    )
    .await
    .map_err(|error| AppError::Daemon(format!("SMTP sink: {error:#}")))?;
    Ok(vec![format!("smtp:{to}")])
}

async fn signal_channel(
    ctx: &IoContext,
    config: &Config,
    message: &str,
    output: &RunOutput,
) -> AppResult<Vec<String>> {
    let signal = crate::integrations::signal::load(&ctx.pool).await?;
    if !signal.enabled {
        return Err(AppError::BadRequest("Signal não está habilitado".into()));
    }
    let recipient = config
        .recipient
        .as_deref()
        .or_else(|| signal.allowlist.first().map(String::as_str))
        .ok_or_else(|| AppError::BadRequest("SIGNAL_MSG sem destinatário".into()))?;
    let attachments = artifact_paths(output)
        .into_iter()
        .take(5)
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    crate::integrations::signal::send_with_attachments(
        &signal.cli_path,
        &signal.account,
        recipient,
        message,
        &attachments,
    )
    .await?;
    Ok(vec![format!("signal:{recipient}")])
}

async fn telegram(ctx: &IoContext, config: &Config, message: &str) -> AppResult<Vec<String>> {
    let token = secret(ctx, config)?;
    let chat_id = config
        .chat_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Telegram sink exige chat_id".into()))?;
    crate::io_bus::chat::send_telegram(&token, chat_id, message, Some("HTML")).await?;
    Ok(vec![format!("telegram:{chat_id}")])
}

async fn matrix(ctx: &IoContext, config: &Config, message: &str) -> AppResult<Vec<String>> {
    let token = secret(ctx, config)?;
    let homeserver = config
        .homeserver
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Matrix sink exige homeserver".into()))?;
    let room = config
        .room_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Matrix sink exige room_id".into()))?;
    crate::io_bus::chat::send_matrix(homeserver, &token, room, message).await?;
    Ok(vec![format!("matrix:{room}")])
}

async fn mattermost(config: &Config, message: &str) -> AppResult<Vec<String>> {
    if let Some(webhook) = config.webhook_url.as_deref() {
        let response = crate::io_bus::http_client(30)
            .post(webhook)
            .json(&serde_json::json!({ "text": message }))
            .send()
            .await
            .map_err(|error| AppError::Daemon(format!("Mattermost: {error}")))?;
        if !response.status().is_success() {
            return Err(AppError::Daemon(format!(
                "Mattermost respondeu {}",
                response.status()
            )));
        }
        return Ok(vec!["mattermost:webhook".into()]);
    }
    Err(AppError::BadRequest(
        "Mattermost sink exige webhook_url".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io_bus::{RunArtifact, RunOutput};

    #[test]
    fn builds_markdown_with_artifacts() {
        let output = RunOutput {
            workspace_id: "ws".into(),
            workspace_root: "/tmp/ws".into(),
            output_directory: None,
            task_id: None,
            run_id: "run".into(),
            title: "Relatório".into(),
            summary: "Resumo".into(),
            artifacts: vec![RunArtifact {
                file_path: "out/a.md".into(),
                absolute_path: "/tmp/ws/out/a.md".into(),
                action_type: "CREATED".into(),
            }],
        };
        let text = message_markdown(&output);
        assert!(text.contains("## Relatório"));
        assert!(text.contains("out/a.md"));
    }
}
