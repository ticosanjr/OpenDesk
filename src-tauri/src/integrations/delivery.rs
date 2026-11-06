use crate::{
    db::{artifacts, deliveries, runs, schedules, tasks, workspaces},
    error::AppResult,
    integrations::{
        google_api::GoogleApi,
        google_oauth::GoogleOAuth,
        smtp_mailer::{self, EmailAttachment},
    },
    paths::AppPaths,
    secrets::SecretStore,
};
use html_escape::encode_text;
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

    let mut recipients = Vec::new();
    let mut export_to_gdocs = false;
    let mut sync_to_drive = false;
    let mut title = run
        .task_title
        .clone()
        .unwrap_or_else(|| "OpenDesk run".to_string());

    if let Some(task_id) = task_id {
        if let Ok(task) = tasks::get(pool, task_id).await {
            title = task.title.clone();
            export_to_gdocs = task.export_to_gdocs;
            sync_to_drive = task.export_to_gdocs;
            if let Some(email) = task.notify_email {
                recipients.extend(split_recipients(&email));
            }
        }
    }

    if let Some(schedule_id) = schedule_id {
        if let Ok(schedule) = schedules::get(pool, schedule_id).await {
            title = schedule.name.clone();
            export_to_gdocs = schedule.sync_to_google_docs;
            sync_to_drive = schedule.sync_to_google_docs;
            if let Some(email) = schedule.notify_email_recipients {
                recipients.extend(split_recipients(&email));
            }
        }
    }

    recipients.sort();
    recipients.dedup();

    if recipients.is_empty() {
        return Ok(());
    }

    let secrets = SecretStore::new(paths);
    let Some(config) = smtp_mailer::load_config(pool, &secrets, workspace_id).await? else {
        tracing::info!(
            workspace = workspace_id,
            "SMTP is not configured; skipping delivery"
        );
        return Ok(());
    };

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

        match smtp_mailer::send_email(
            &config,
            &recipient,
            &subject,
            html.clone(),
            attachments.clone(),
        )
        .await
        {
            Ok(()) => {
                let _ = deliveries::mark_success(pool, &delivery.id, None, None).await;
                tracing::info!(run = run_id, recipient, "run summary emailed");
            }
            Err(error) => {
                let _ = deliveries::mark_failed(pool, &delivery.id, &format!("{error:#}")).await;
                tracing::warn!(run = run_id, recipient, error = %error, "email delivery failed");
            }
        }
    }

    Ok(())
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
        if let Some(artifact) = main {
            if let Ok(markdown) = std::fs::read_to_string(&artifact.absolute_path) {
                let key = format!("gdocs:{run_id}");
                if let Ok(Some(delivery)) = deliveries::create_pending(
                    pool,
                    workspace_id,
                    Some(&artifact.id),
                    "GDOCS",
                    &key,
                )
                .await
                {
                    match api
                        .gdocs_create_from_markdown(workspace_id, title, &markdown, folder_id)
                        .await
                    {
                        Ok(value) => {
                            let url = value.get("webViewLink").and_then(serde_json::Value::as_str);
                            let _ =
                                artifacts::set_destinations(pool, &artifact.id, url, None, None)
                                    .await;
                            let _ = deliveries::mark_success(
                                pool,
                                &delivery.id,
                                value.get("id").and_then(serde_json::Value::as_str),
                                url,
                            )
                            .await;
                        }
                        Err(error) => {
                            let _ =
                                deliveries::mark_failed(pool, &delivery.id, &format!("{error:#}"))
                                    .await;
                            tracing::warn!(run = run_id, error = %error, "Google Docs export failed");
                        }
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
            match api
                .drive_upload(workspace_id, name, mime, bytes, folder_id, false)
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
                    let _ =
                        deliveries::mark_failed(pool, &delivery.id, &format!("{error:#}")).await;
                    tracing::warn!(run = run_id, error = %error, "Google Drive upload failed");
                }
            }
        }
    }

    let _ = task_id;
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
            if let Some(root) = &root {
                if !canonical.starts_with(root) {
                    return None;
                }
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
