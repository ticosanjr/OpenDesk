use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn export_google(
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
            if let Ok(Some(delivery)) = deliveries::create_pending(
                pool,
                workspace_id,
                Some(&artifact.id),
                DeliveryChannel::GoogleDocs.as_str(),
                &key,
            )
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
            let Ok(Some(delivery)) = deliveries::create_pending(
                pool,
                workspace_id,
                Some(&artifact.id),
                DeliveryChannel::GoogleDrive.as_str(),
                &key,
            )
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
            let mime = mime_for_path(Path::new(&artifact.absolute_path));
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

pub(super) async fn retry_gdocs(
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

pub(super) async fn retry_drive(
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
    let mime = mime_for_path(path);
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
