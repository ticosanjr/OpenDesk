use crate::{
    app_state::AppState,
    db::{artifacts, deliveries, runs, schedules, tasks, workspaces},
    error::{AppError, AppResult},
    integrations::{
        google_api::GoogleApi,
        google_oauth::GoogleOAuth,
        signal,
        smtp_mailer::{self, EmailAttachment},
    },
    paths::AppPaths,
    secrets::SecretStore,
};
use html_escape::encode_text;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub async fn deliver_run(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    task_id: Option<&str>,
    schedule_id: Option<&str>,
    run_id: &str,
    google: Arc<GoogleOAuth>,
) -> AppResult<()> {
    let run = match runs::get(pool, run_id).await {
        Ok(run) => run,
        Err(_) => return Ok(()),
    };

    if run.status != "SUCCESS" {
        return Ok(());
    }

    let schedule = match schedule_id {
        Some(id) => schedules::get(pool, id).await.ok(),
        None => None,
    };

    let mut recipients = Vec::new();
    let mut export_to_gdocs = false;
    let mut sync_to_drive = false;
    let mut title = run
        .task_title
        .clone()
        .unwrap_or_else(|| "OpenDesk run".to_string());

    if let Some(task_id) = task_id
        && let Ok(task) = tasks::get(pool, task_id).await
    {
        title = task.title.clone();
        export_to_gdocs = task.export_to_gdocs;
        sync_to_drive = task.export_to_gdocs;
        if let Some(email) = task.notify_email {
            recipients.extend(split_recipients(&email));
        }
    }

    if let Some(schedule) = &schedule {
        title = schedule.name.clone();
        export_to_gdocs = schedule.sync_to_google_docs;
        sync_to_drive = schedule.sync_to_google_docs;
        if let Some(email) = &schedule.notify_email_recipients {
            recipients.extend(split_recipients(email));
        }
    }

    recipients.sort();
    recipients.dedup();

    let workspace_root = workspaces::get(pool, workspace_id)
        .await
        .ok()
        .map(|workspace| PathBuf::from(workspace.root_path));

    let artifact_list = artifacts::list(pool, workspace_id, None, task_id, None, 50)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|artifact| artifact.run_id == run_id)
        .collect::<Vec<_>>();

    let novelty_hash = novelty_hash(&artifact_list, run.summary_response.as_deref());
    let unchanged_output = schedule.as_ref().is_some_and(|schedule| {
        schedule.notify_only_on_change
            && novelty_hash.is_some()
            && schedule.last_output_hash.as_deref() == novelty_hash.as_deref()
    });
    if unchanged_output {
        tracing::info!(
            run = run_id,
            "no output change; keeping exports and skipping notifications"
        );
    }

    if export_to_gdocs || sync_to_drive {
        export_google(
            pool,
            google.clone(),
            workspace_id,
            task_id,
            run_id,
            &title,
            &artifact_list,
            export_to_gdocs,
            sync_to_drive,
        )
        .await;
    }

    let subject = format!("[OpenDesk] {title} · concluído");
    let html = build_html(&title, &run, &artifact_list);
    let attachments = build_attachments(&artifact_list, workspace_root.as_deref());
    let retry_artifacts = artifact_list
        .iter()
        .filter(|artifact| artifact.action_type != "DELETED")
        .map(|artifact| artifact.absolute_path.clone())
        .collect::<Vec<_>>();
    let secrets = SecretStore::new(paths);
    let smtp_config = smtp_mailer::load_config(pool, &secrets, workspace_id)
        .await
        .ok()
        .flatten();

    if !unchanged_output && !recipients.is_empty() && smtp_config.is_none() {
        tracing::info!(
            workspace = workspace_id,
            "SMTP is not configured; skipping email delivery"
        );
    }

    if let Some(config) = &smtp_config
        && !unchanged_output
    {
        for recipient in recipients {
            let key = format!("smtp:{run_id}:{recipient}");
            let Some(delivery) = deliveries::create_pending(
                pool,
                workspace_id,
                artifact_list.first().map(|artifact| artifact.id.as_str()),
                "SMTP",
                &key,
            )
            .await?
            else {
                continue;
            };

            match with_retry(3, || {
                smtp_mailer::send_email(
                    config,
                    &recipient,
                    &subject,
                    html.clone(),
                    attachments.clone(),
                )
            })
            .await
            {
                Ok(()) => {
                    let _ = deliveries::mark_success(pool, &delivery.id, None, None).await;
                    tracing::info!(run = run_id, recipient, "run summary emailed");
                }
                Err(error) => {
                    let payload = serde_json::json!({
                        "recipient": recipient,
                        "subject": subject,
                        "html": html,
                        "artifact_paths": retry_artifacts,
                    })
                    .to_string();
                    let _ = deliveries::mark_failed_retryable(
                        pool,
                        &delivery.id,
                        &format!("{error:#}"),
                        &payload,
                        5,
                    )
                    .await;
                    tracing::warn!(run = run_id, recipient, error = %error, "email delivery failed; scheduled for retry");
                }
            }
        }
    }

    if let Some(schedule) = &schedule
        && schedule.notify_signal
        && !unchanged_output
    {
        let config = signal::load(pool).await?;
        let mut signal_recipients = schedule
            .notify_signal_recipients
            .as_deref()
            .map(split_recipients)
            .unwrap_or_default();
        if signal_recipients.is_empty() {
            signal_recipients = config.allowlist.clone();
        }
        let message = format!(
            "[OpenDesk] {title}\nstatus: {}\n{}\n{}",
            run.status,
            run.summary_response.as_deref().unwrap_or("sem resumo"),
            artifact_list
                .first()
                .map(|artifact| artifact.absolute_path.as_str())
                .unwrap_or("sem artefato")
        );
        if config.account.trim().is_empty() {
            tracing::warn!(schedule = %schedule.id, "Signal notification skipped: account is not configured");
        } else {
            for recipient in signal_recipients {
                let key = format!("signal:{run_id}:{recipient}");
                let Some(delivery) = deliveries::create_pending(
                    pool,
                    workspace_id,
                    artifact_list.first().map(|artifact| artifact.id.as_str()),
                    "SIGNAL",
                    &key,
                )
                .await?
                else {
                    continue;
                };
                match signal::send_text(&config.cli_path, &config.account, &recipient, &message)
                    .await
                {
                    Ok(()) => {
                        let _ = deliveries::mark_success(pool, &delivery.id, None, None).await;
                    }
                    Err(error) => {
                        let _ = deliveries::mark_failed(pool, &delivery.id, &format!("{error:#}"))
                            .await;
                        tracing::warn!(run = run_id, recipient, error = %error, "Signal notification failed");
                    }
                }
            }
        }
    }

    if let (Some(schedule), Some(hash)) = (&schedule, novelty_hash.as_deref()) {
        let _ = schedules::set_last_output_hash(pool, &schedule.id, Some(hash)).await;
    }

    Ok(())
}

fn novelty_hash(
    artifacts: &[artifacts::ArtifactProvenance],
    summary: Option<&str>,
) -> Option<String> {
    let artifact = artifacts
        .iter()
        .filter(|artifact| artifact.action_type != "DELETED")
        .find(|artifact| artifact.file_path.ends_with(".md"))
        .or_else(|| {
            artifacts
                .iter()
                .find(|artifact| artifact.action_type != "DELETED")
        });
    if let Some(artifact) = artifact
        && let Ok(bytes) = std::fs::read(&artifact.absolute_path)
    {
        return Some(hex_digest(&bytes));
    }
    summary.map(|value| hex_digest(value.as_bytes()))
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[allow(clippy::too_many_arguments)]
async fn export_google(
    pool: &SqlitePool,
    google: Arc<GoogleOAuth>,
    workspace_id: &str,
    task_id: Option<&str>,
    run_id: &str,
    title: &str,
    artifacts: &[artifacts::ArtifactProvenance],
    export_to_gdocs: bool,
    sync_to_drive: bool,
) {
    let api = GoogleApi::new(google);
    let workspace = workspaces::get(pool, workspace_id).await.ok();
    let folder_id = workspace
        .as_ref()
        .and_then(|workspace| workspace.google_drive_folder_id.as_deref());

    if export_to_gdocs {
        let main = artifacts
            .iter()
            .filter(|artifact| artifact.action_type != "DELETED")
            .find(|artifact| artifact.absolute_path.to_lowercase().ends_with(".md"));
        if let Some(artifact) = main
            && let Ok(markdown) = std::fs::read_to_string(&artifact.absolute_path)
        {
            let key = format!("gdocs:{run_id}");
            if let Ok(Some(delivery)) =
                deliveries::create_pending(pool, workspace_id, Some(&artifact.id), "GDOCS", &key)
                    .await
            {
                match with_retry(3, || {
                    api.gdocs_create_from_markdown(workspace_id, title, &markdown, folder_id)
                })
                .await
                {
                    Ok(value) => {
                        let url = value.get("webViewLink").and_then(serde_json::Value::as_str);
                        let _ =
                            artifacts::set_destinations(pool, &artifact.id, url, None, None).await;
                        let _ = deliveries::mark_success(
                            pool,
                            &delivery.id,
                            value.get("id").and_then(serde_json::Value::as_str),
                            url,
                        )
                        .await;
                    }
                    Err(error) => {
                        let payload = serde_json::json!({
                            "artifact_id": artifact.id,
                            "title": title,
                            "folder_id": folder_id,
                        })
                        .to_string();
                        let _ = deliveries::mark_failed_retryable(
                            pool,
                            &delivery.id,
                            &format!("{error:#}"),
                            &payload,
                            5,
                        )
                        .await;
                        tracing::warn!(run = run_id, error = %error, "Google Docs export failed; scheduled for retry");
                    }
                }
            }
        }
    }

    if sync_to_drive {
        for artifact in artifacts
            .iter()
            .filter(|artifact| artifact.action_type != "DELETED")
            .take(5)
        {
            let key = format!("drive:{run_id}:{}", artifact.id);
            let Ok(Some(delivery)) =
                deliveries::create_pending(pool, workspace_id, Some(&artifact.id), "DRIVE", &key)
                    .await
            else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&artifact.absolute_path) else {
                let _ = deliveries::mark_skipped(pool, &delivery.id, "file missing").await;
                continue;
            };
            let name = Path::new(&artifact.absolute_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("artifact");
            let mime = mime_for(Path::new(&artifact.absolute_path));
            let bytes_for_retry = bytes.clone();
            match with_retry(3, || {
                api.drive_upload(
                    workspace_id,
                    name,
                    mime,
                    bytes_for_retry.clone(),
                    folder_id,
                    false,
                )
            })
            .await
            {
                Ok(value) => {
                    let url = value.get("webViewLink").and_then(serde_json::Value::as_str);
                    let _ = artifacts::set_destinations(pool, &artifact.id, None, url, None).await;
                    let _ = deliveries::mark_success(
                        pool,
                        &delivery.id,
                        value.get("id").and_then(serde_json::Value::as_str),
                        url,
                    )
                    .await;
                }
                Err(error) => {
                    let payload = serde_json::json!({
                        "artifact_id": artifact.id,
                        "folder_id": folder_id,
                    })
                    .to_string();
                    let _ = deliveries::mark_failed_retryable(
                        pool,
                        &delivery.id,
                        &format!("{error:#}"),
                        &payload,
                        5,
                    )
                    .await;
                    tracing::warn!(run = run_id, error = %error, "Google Drive upload failed; scheduled for retry");
                }
            }
        }
    }

    let _ = task_id;
}

/// Durable retry worker. It survives restarts because the payload and next
/// attempt live in SQLite.
pub async fn retry_loop(state: std::sync::Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        let Ok(due) = deliveries::due_retries(&state.db, 10).await else {
            continue;
        };
        for delivery in due {
            if let Err(error) = retry_delivery(&state, &delivery).await {
                tracing::warn!(delivery = %delivery.id, error = %error, "delivery retry failed");
            }
        }
    }
}

async fn retry_delivery(state: &AppState, delivery: &deliveries::Delivery) -> AppResult<()> {
    let Some(payload) = delivery.payload_json.as_deref() else {
        return Ok(());
    };
    let payload: serde_json::Value = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(_) => {
            let _ =
                deliveries::mark_skipped(&state.db, &delivery.id, "invalid retry payload").await;
            return Ok(());
        }
    };
    match delivery.channel.as_str() {
        "GDOCS" => retry_gdocs(state, delivery, &payload).await,
        "DRIVE" => retry_drive(state, delivery, &payload).await,
        _ => retry_email(state, delivery, &payload).await,
    }
}

/// Retry one delivery immediately, on user request. Only failed deliveries
/// with a stored payload are eligible; successful deliveries are never replayed
/// because that could duplicate a document or upload.
pub async fn retry_one(state: &AppState, id: &str) -> AppResult<deliveries::Delivery> {
    let delivery = deliveries::get(&state.db, id).await?;
    if delivery.status != "FAILED" {
        return Err(AppError::BadRequest(format!(
            "somente entregas FAILED podem ser reenviadas (status atual: {})",
            delivery.status
        )));
    }
    if delivery.payload_json.is_none() {
        return Err(AppError::BadRequest(
            "entrega não possui payload para reenvio".into(),
        ));
    }
    retry_delivery(state, &delivery).await?;
    deliveries::get(&state.db, id).await
}

async fn retry_email(
    state: &AppState,
    delivery: &deliveries::Delivery,
    payload: &serde_json::Value,
) -> AppResult<()> {
    let recipient = payload
        .get("recipient")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let subject = payload
        .get("subject")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("[OpenDesk] entrega pendente");
    let html = payload
        .get("html")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let paths = payload
        .get("artifact_paths")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let attachments = attachments_from_paths(&paths);

    let Some(config) =
        smtp_mailer::load_config(&state.db, &state.secrets, &delivery.workspace_id).await?
    else {
        let _ = deliveries::reschedule(&state.db, &delivery.id, "SMTP not configured", 60).await;
        return Ok(());
    };

    match smtp_mailer::send_email(&config, recipient, subject, html.to_string(), attachments).await
    {
        Ok(()) => {
            let _ = deliveries::mark_success(&state.db, &delivery.id, None, None).await;
        }
        Err(error) => {
            let _ = deliveries::reschedule(
                &state.db,
                &delivery.id,
                &format!("{error:#}"),
                retry_delay(delivery.attempts),
            )
            .await;
        }
    }
    Ok(())
}

async fn retry_gdocs(
    state: &AppState,
    delivery: &deliveries::Delivery,
    payload: &serde_json::Value,
) -> AppResult<()> {
    let Some(artifact_id) = payload
        .get("artifact_id")
        .and_then(serde_json::Value::as_str)
    else {
        let _ = deliveries::mark_skipped(&state.db, &delivery.id, "missing artifact id").await;
        return Ok(());
    };
    let Ok(artifact) = artifacts::get(&state.db, artifact_id).await else {
        let _ = deliveries::mark_skipped(&state.db, &delivery.id, "artifact not found").await;
        return Ok(());
    };
    let Ok(markdown) = std::fs::read_to_string(&artifact.absolute_path) else {
        let _ = deliveries::mark_skipped(&state.db, &delivery.id, "artifact file missing").await;
        return Ok(());
    };
    let title = payload
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("OpenDesk run");
    let folder_id = payload.get("folder_id").and_then(serde_json::Value::as_str);
    let api = GoogleApi::new(state.google.clone());

    match api
        .gdocs_create_from_markdown(&delivery.workspace_id, title, &markdown, folder_id)
        .await
    {
        Ok(value) => {
            let url = value.get("webViewLink").and_then(serde_json::Value::as_str);
            let _ = artifacts::set_destinations(&state.db, &artifact.id, url, None, None).await;
            let _ = deliveries::mark_success(
                &state.db,
                &delivery.id,
                value.get("id").and_then(serde_json::Value::as_str),
                url,
            )
            .await;
        }
        Err(error) => {
            let _ = deliveries::reschedule(
                &state.db,
                &delivery.id,
                &format!("{error:#}"),
                retry_delay(delivery.attempts),
            )
            .await;
        }
    }
    Ok(())
}

async fn retry_drive(
    state: &AppState,
    delivery: &deliveries::Delivery,
    payload: &serde_json::Value,
) -> AppResult<()> {
    let Some(artifact_id) = payload
        .get("artifact_id")
        .and_then(serde_json::Value::as_str)
    else {
        let _ = deliveries::mark_skipped(&state.db, &delivery.id, "missing artifact id").await;
        return Ok(());
    };
    let Ok(artifact) = artifacts::get(&state.db, artifact_id).await else {
        let _ = deliveries::mark_skipped(&state.db, &delivery.id, "artifact not found").await;
        return Ok(());
    };
    let Ok(bytes) = std::fs::read(&artifact.absolute_path) else {
        let _ = deliveries::mark_skipped(&state.db, &delivery.id, "artifact file missing").await;
        return Ok(());
    };
    let path = Path::new(&artifact.absolute_path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact");
    let mime = mime_for(path);
    let folder_id = payload.get("folder_id").and_then(serde_json::Value::as_str);
    let api = GoogleApi::new(state.google.clone());

    match api
        .drive_upload(&delivery.workspace_id, name, mime, bytes, folder_id, false)
        .await
    {
        Ok(value) => {
            let url = value.get("webViewLink").and_then(serde_json::Value::as_str);
            let _ = artifacts::set_destinations(&state.db, &artifact.id, None, url, None).await;
            let _ = deliveries::mark_success(
                &state.db,
                &delivery.id,
                value.get("id").and_then(serde_json::Value::as_str),
                url,
            )
            .await;
        }
        Err(error) => {
            let _ = deliveries::reschedule(
                &state.db,
                &delivery.id,
                &format!("{error:#}"),
                retry_delay(delivery.attempts),
            )
            .await;
        }
    }
    Ok(())
}

fn retry_delay(attempts: i64) -> i64 {
    (5i64 * 2i64.pow(attempts.clamp(0, 6) as u32)).min(360)
}

fn attachments_from_paths(paths: &[String]) -> Vec<EmailAttachment> {
    paths
        .iter()
        .take(5)
        .filter_map(|path| {
            let path = PathBuf::from(path);
            let bytes = std::fs::read(&path).ok()?;
            if !smtp_mailer::attachment_limit(bytes.len()) {
                return None;
            }
            Some(EmailAttachment {
                filename: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("artifact")
                    .to_string(),
                mime: mime_for(&path).to_string(),
                bytes,
            })
        })
        .collect()
}

/// Best-effort retry for transient network failures. The delivery rows are
/// idempotent, so a retry never duplicates a successful destination.
async fn with_retry<F, Fut, T>(attempts: usize, mut operation: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let attempts = attempts.max(1);
    let mut last_error = None;
    for attempt in 1..=attempts {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                last_error = Some(error);
                if attempt < attempts {
                    tokio::time::sleep(std::time::Duration::from_millis(400 * attempt as u64))
                        .await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("retry operation produced no result")))
}

fn split_recipients(value: &str) -> Vec<String> {
    value
        .split([',', ';'])
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_string)
        .collect()
}

fn build_html(
    title: &str,
    run: &runs::TaskRun,
    artifacts: &[artifacts::ArtifactProvenance],
) -> String {
    let summary = run
        .summary_response
        .as_deref()
        .unwrap_or("Sem resumo retornado pelo agente.")
        .chars()
        .take(4000)
        .collect::<String>();
    let rows = artifacts
        .iter()
        .map(|artifact| {
            format!(
                "<tr><td style=\"padding:6px;border-bottom:1px solid #27272a;font-family:monospace\">{}</td>\
                 <td style=\"padding:6px;border-bottom:1px solid #27272a\">{}</td>\
                 <td style=\"padding:6px;border-bottom:1px solid #27272a\">{}</td></tr>",
                encode_text(&artifact.file_path),
                encode_text(&artifact.created_by_agent),
                artifact.file_size_bytes
            )
        })
        .collect::<String>();

    format!(
        "<div style=\"font-family:Inter,Arial,sans-serif;background:#09090b;color:#e4e4e7;padding:24px\">\
         <h2 style=\"margin:0 0 8px\">{title}</h2>\
         <p style=\"color:#a1a1aa;margin:0 0 16px\">Run concluído no OpenDesk. Modelo <code>{}</code> · agente <code>@{}</code> · custo <code>${:.4}</code>.</p>\
         <h3>Resumo</h3><pre style=\"white-space:pre-wrap;background:#18181b;padding:12px;border-radius:8px;color:#d4d4d8\">{}</pre>\
         <h3>Artefatos</h3>\
         <table style=\"width:100%;border-collapse:collapse;font-size:12px\">\
         <thead><tr><th align=\"left\">Arquivo</th><th align=\"left\">Agente</th><th align=\"left\">Bytes</th></tr></thead>\
         <tbody>{rows}</tbody></table>\
         <p style=\"color:#71717a;font-size:11px;margin-top:24px\">Gerado automaticamente pelo OpenDesk.</p>\
         </div>",
        encode_text(&run.model_id),
        encode_text(&run.agent_name),
        run.estimated_cost_usd,
        encode_text(&summary)
    )
}

fn build_attachments(
    artifacts: &[artifacts::ArtifactProvenance],
    workspace_root: Option<&Path>,
) -> Vec<EmailAttachment> {
    let root = workspace_root.and_then(|root| std::fs::canonicalize(root).ok());

    artifacts
        .iter()
        .filter(|artifact| artifact.action_type != "DELETED")
        .take(5)
        .filter_map(|artifact| {
            let path = PathBuf::from(&artifact.absolute_path);
            let canonical = std::fs::canonicalize(&path).ok()?;
            if let Some(root) = &root
                && !canonical.starts_with(root)
            {
                return None;
            }
            let bytes = std::fs::read(&path).ok()?;
            if !smtp_mailer::attachment_limit(bytes.len()) {
                return None;
            }
            Some(EmailAttachment {
                filename: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("artifact")
                    .to_string(),
                mime: mime_for(&path).to_string(),
                bytes,
            })
        })
        .collect()
}

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("md") | Some("markdown") => "text/markdown",
        Some("json") => "application/json",
        Some("html") | Some("htm") => "text/html",
        Some("txt") | Some("log") => "text/plain",
        Some("csv") => "text/csv",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn retries_until_the_operation_succeeds() {
        let attempts = AtomicUsize::new(0);
        let result: anyhow::Result<&'static str> = with_retry(3, || {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            async move {
                if attempt < 3 {
                    anyhow::bail!("transient")
                }
                Ok("ok")
            }
        })
        .await;
        assert_eq!(result.expect("third attempt"), "ok");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn returns_the_last_error_after_all_attempts() {
        let attempts = AtomicUsize::new(0);
        let result: anyhow::Result<()> = with_retry(2, || {
            attempts.fetch_add(1, Ordering::SeqCst);
            async move { anyhow::bail!("still down") }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    async fn due_delivery(
        channel: &str,
        key: &str,
        payload: serde_json::Value,
    ) -> (crate::test_support::TestContext, deliveries::Delivery) {
        let context = crate::test_support::context().await;
        let delivery = deliveries::create_pending(
            &context.state.db,
            &context.workspace_id,
            None,
            channel,
            key,
        )
        .await
        .expect("create")
        .expect("created");
        deliveries::mark_failed_retryable(
            &context.state.db,
            &delivery.id,
            "transient",
            &payload.to_string(),
            0,
        )
        .await
        .expect("schedule");
        let due = deliveries::due_retries(&context.state.db, 10)
            .await
            .expect("due");
        let row = due
            .into_iter()
            .find(|row| row.id == delivery.id)
            .expect("due row");
        (context, row)
    }

    #[tokio::test]
    async fn gdocs_retry_skips_when_the_artifact_is_gone() {
        let (context, delivery) = due_delivery(
            "GDOCS",
            "gdocs:retry:missing",
            serde_json::json!({ "artifact_id": "missing" }),
        )
        .await;
        retry_delivery(&context.state, &delivery)
            .await
            .expect("retry");
        let stored = deliveries::get_by_key(&context.state.db, "gdocs:retry:missing")
            .await
            .expect("get")
            .expect("row");
        assert_eq!(stored.status, "SKIPPED");
        assert_eq!(stored.last_error.as_deref(), Some("artifact not found"));
    }

    #[tokio::test]
    async fn drive_retry_skips_when_the_artifact_is_gone() {
        let (context, delivery) = due_delivery(
            "DRIVE",
            "drive:retry:missing",
            serde_json::json!({ "artifact_id": "missing" }),
        )
        .await;
        retry_delivery(&context.state, &delivery)
            .await
            .expect("retry");
        let stored = deliveries::get_by_key(&context.state.db, "drive:retry:missing")
            .await
            .expect("get")
            .expect("row");
        assert_eq!(stored.status, "SKIPPED");
        assert_eq!(stored.last_error.as_deref(), Some("artifact not found"));
    }

    #[tokio::test]
    async fn retry_rejects_invalid_payloads_without_touching_google() {
        let (context, delivery) = due_delivery(
            "DRIVE",
            "drive:retry:invalid",
            serde_json::json!({ "artifact_id": "unused" }),
        )
        .await;
        let mut broken = delivery.clone();
        broken.payload_json = Some("not-json".into());
        retry_delivery(&context.state, &broken)
            .await
            .expect("retry");
        let stored = deliveries::get_by_key(&context.state.db, "drive:retry:invalid")
            .await
            .expect("get")
            .expect("row");
        assert_eq!(stored.status, "SKIPPED");
        assert_eq!(stored.last_error.as_deref(), Some("invalid retry payload"));
    }
}
