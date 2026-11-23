use crate::{
    app_state::AppState,
    db::chat as chat_db,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct EmailReplyBody {
    pub to: String,
    pub subject: String,
    #[serde(default)]
    pub body_html: Option<String>,
    /// Paths relative to the workspace root.
    #[serde(default)]
    pub attachments: Vec<String>,
}

/// Sends the last assistant message (or the supplied HTML) as an email reply in
/// the thread identified by `chat_threads.external_id`.
pub async fn send_email_reply(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<EmailReplyBody>,
) -> AppResult<Json<serde_json::Value>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    let in_reply_to = thread
        .external_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::BadRequest(
                "thread sem Message-ID de e-mail; não é uma thread de e-mail".into(),
            )
        })?;
    let html = match body.body_html {
        Some(html) => html,
        None => {
            let messages = chat_db::list_messages(&state.db, &id).await?;
            let content = messages
                .iter()
                .rev()
                .find(|message| message.role == "assistant")
                .map(|message| message.content.clone())
                .ok_or_else(|| AppError::BadRequest("thread sem resposta do assistente".into()))?;
            format!(
                "<pre style=\"white-space:pre-wrap\">{}</pre>",
                html_escape::encode_text(&content)
            )
        }
    };
    let secrets = crate::secrets::SecretStore::new(&state.paths);
    let config =
        crate::integrations::smtp_mailer::load_config(&state.db, &secrets, &thread.workspace_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("SMTP não está configurado".into()))?;
    let workspace = crate::db::workspaces::get(&state.db, &thread.workspace_id).await?;
    let root = std::path::Path::new(&workspace.root_path);
    let mut attachments = Vec::new();
    for path in body.attachments.iter().take(10) {
        let relative = std::path::Path::new(path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(AppError::BadRequest(format!(
                "anexo fora do workspace: {path}"
            )));
        }
        let target = root.join(relative);
        let canonical = std::fs::canonicalize(&target)?;
        let canonical_root = std::fs::canonicalize(root)?;
        if !canonical.starts_with(&canonical_root) {
            return Err(AppError::BadRequest(format!(
                "anexo fora do workspace: {path}"
            )));
        }
        let bytes = std::fs::read(&canonical)?;
        if !crate::integrations::smtp_mailer::attachment_limit(bytes.len()) {
            return Err(AppError::BadRequest(format!("anexo excede 10 MB: {path}")));
        }
        let filename = canonical
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "anexo".into());
        let mime = match canonical
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "pdf" => "application/pdf",
            "txt" | "md" => "text/plain",
            "csv" => "text/csv",
            "json" => "application/json",
            _ => "application/octet-stream",
        };
        attachments.push(crate::integrations::smtp_mailer::EmailAttachment {
            filename,
            bytes,
            mime: mime.into(),
        });
    }
    crate::integrations::smtp_mailer::send_email_threaded(
        &config,
        &body.to,
        &body.subject,
        html,
        attachments,
        Some(&crate::integrations::smtp_mailer::EmailThread {
            in_reply_to: in_reply_to.to_string(),
            references: vec![in_reply_to.to_string()],
        }),
    )
    .await
    .map_err(|error| AppError::Daemon(format!("{error:#}")))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
