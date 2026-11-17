use crate::{
    error::{AppError, AppResult},
    integrations::smtp_mailer,
    recipe_context::RecipeContext,
    recipes::hitl::{HitlRequestPayload, parse_reply, resolve},
    recipes::store,
};

pub async fn dispatch(ctx: &RecipeContext, payload: &HitlRequestPayload) -> AppResult<String> {
    let workspace = crate::db::workspaces::get(&ctx.pool, &payload.workspace_id).await?;
    let to = workspace
        .default_email_recipient
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AppError::BadRequest(
                "workspace sem e-mail padrão para HITL; configure em Configurações → Workspaces"
                    .into(),
            )
        })?;
    let config = smtp_mailer::load_config(&ctx.pool, &ctx.secrets, &payload.workspace_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("SMTP não configurado".into()))?;
    let instructions =
        crate::recipes::hitl::reply_instructions(&payload.interaction_type, &payload.options);
    let html = format!(
        "<html><body><pre style=\"white-space:pre-wrap\">{}</pre><p style=\"white-space:pre-wrap\">{}</p></body></html>",
        html_escape::encode_text(&payload.message_body),
        html_escape::encode_text(&instructions)
    );
    let attachments = payload
        .attachments
        .iter()
        .take(5)
        .filter_map(|path| {
            let bytes = std::fs::read(path).ok()?;
            if !smtp_mailer::attachment_limit(bytes.len()) {
                return None;
            }
            Some(smtp_mailer::EmailAttachment {
                filename: std::path::Path::new(path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("anexo")
                    .to_string(),
                mime: crate::media::mime_for_path(std::path::Path::new(path)).to_string(),
                bytes,
            })
        })
        .collect();
    smtp_mailer::send_email(
        &config,
        to,
        &format!("[OD-#{}] {}", payload.token, payload.title),
        html,
        attachments,
    )
    .await
    .map_err(|error| AppError::Daemon(format!("HITL SMTP: {error:#}")))?;
    Ok(format!("hitl-{}@opendesk.local", payload.token))
}

pub async fn handle_reply(
    ctx: &RecipeContext,
    subject: &str,
    from: &str,
    text: &str,
    attachments: &[String],
) -> AppResult<bool> {
    let Some(token) = crate::recipes::hitl::extract_token(subject) else {
        return Ok(false);
    };
    let Ok(request) = store::get_hitl(&ctx.pool, &token).await else {
        return Ok(false);
    };
    if request.status != "PENDING" {
        return Ok(true);
    }
    let (resolution_type, payload) = if let Some(path) = attachments.first() {
        (
            "ARTIFACT".to_string(),
            Some(serde_json::json!({ "path": path }).to_string()),
        )
    } else {
        let options: Vec<String> = serde_json::from_str(&request.options_json).unwrap_or_default();
        let (kind, payload) = parse_reply(&request.interaction_type, &options, text);
        (kind, payload)
    };
    resolve(
        ctx,
        &token,
        "EMAIL",
        Some(from),
        &resolution_type,
        payload.as_deref(),
    )
    .await?;
    Ok(true)
}

#[allow(dead_code)]
pub async fn send_ack(
    ctx: &RecipeContext,
    workspace_id: &str,
    subject: &str,
    message: &str,
) -> AppResult<()> {
    let workspace = crate::db::workspaces::get(&ctx.pool, workspace_id).await?;
    let Some(to) = workspace.default_email_recipient else {
        return Ok(());
    };
    let Some(config) = smtp_mailer::load_config(&ctx.pool, &ctx.secrets, workspace_id).await?
    else {
        return Ok(());
    };
    let html = format!(
        "<html><body><pre>{}</pre></body></html>",
        html_escape::encode_text(message)
    );
    smtp_mailer::send_email(&config, &to, &format!("[OD-ACK] {subject}"), html, vec![])
        .await
        .map_err(|error| AppError::Daemon(format!("HITL ACK: {error:#}")))?;
    Ok(())
}
