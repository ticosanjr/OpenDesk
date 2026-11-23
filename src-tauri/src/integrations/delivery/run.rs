use super::watchers::{notification_route, notify_watchers, novelty_hash};
use super::*;

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
        if let Some(task_id) = task_id {
            let title = tasks::get(pool, task_id)
                .await
                .map(|task| task.title)
                .unwrap_or_else(|_| {
                    run.task_title
                        .clone()
                        .unwrap_or_else(|| "OpenDesk run".to_string())
                });
            let secrets = SecretStore::new(paths);
            if let Err(error) =
                notify_watchers(pool, &secrets, workspace_id, task_id, &title, &run, &[]).await
            {
                tracing::warn!(run = run_id, error = %error, "task watcher failed");
            }
        }
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

    let workspace_row = workspaces::get(pool, workspace_id).await.ok();
    let workspace_root = workspace_row
        .as_ref()
        .map(|workspace| PathBuf::from(&workspace.root_path));
    let notification_route = notification_route(
        pool,
        workspace_row
            .as_ref()
            .map(|workspace| workspace.timezone.as_str()),
    )
    .await;
    let allows_channel = |channel: &str| match &notification_route {
        Some(route) => route.deliver && route.channels.iter().any(|item| item == channel),
        None => true,
    };

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
        && allows_channel("email")
    {
        deliver_email(
            pool,
            workspace_id,
            run_id,
            artifact_list.first().map(|artifact| artifact.id.as_str()),
            &subject,
            &html,
            attachments,
            &retry_artifacts,
            &recipients,
            config,
        )
        .await?;
    }

    if let Some(schedule) = &schedule
        && schedule.notify_signal
        && !unchanged_output
        && allows_channel("signal")
    {
        deliver_signal(
            pool,
            workspace_id,
            run_id,
            &title,
            &run,
            artifact_list.first(),
            schedule,
        )
        .await?;
    }

    if let Some(schedule) = &schedule
        && schedule.notify_telegram
        && !unchanged_output
        && allows_channel("telegram")
    {
        deliver_telegram(
            pool,
            &secrets,
            workspace_id,
            run_id,
            &title,
            &run,
            artifact_list.first(),
            schedule,
        )
        .await?;
    }

    if let Some(task_id) = task_id
        && let Err(error) = notify_watchers(
            pool,
            &secrets,
            workspace_id,
            task_id,
            &title,
            &run,
            &artifact_list,
        )
        .await
    {
        tracing::warn!(run = run_id, error = %error, "task watcher failed");
    }

    if let (Some(schedule), Some(hash)) = (&schedule, novelty_hash.as_deref()) {
        let _ = schedules::set_last_output_hash(pool, &schedule.id, Some(hash)).await;
    }

    Ok(())
}
