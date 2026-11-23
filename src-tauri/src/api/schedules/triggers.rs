use crate::{
    app_state::AppState,
    db::schedules::{self, Schedule},
    error::AppResult,
};
use std::sync::Arc;

/// Create, update or remove the inbound source that feeds a non-cron routine.
/// Cron routines need no source; SIGNAL_MSG is delivered by the Signal bridge.
pub(crate) async fn sync_trigger_source(
    state: &Arc<AppState>,
    schedule: &Schedule,
) -> AppResult<()> {
    let source_type = match schedule.trigger_type.as_str() {
        "FOLDER_WATCH" => Some("FOLDER_WATCH"),
        "RSYNC_FOLDER" => Some("FOLDER_WATCH"),
        "IMAP_IDLE" => Some("IMAP_IDLE"),
        "WEBHOOK" => Some("WEBHOOK_HTTP"),
        "SQL_POLL" => Some("SQL_POLLER"),
        _ => None,
    };
    let existing = crate::db::io_bus::list_inbound(&state.db, &schedule.workspace_id)
        .await?
        .into_iter()
        .find(|source| {
            serde_json::from_str::<serde_json::Value>(&source.config_json)
                .ok()
                .and_then(|config| {
                    config
                        .get("schedule_id")
                        .and_then(|value| value.as_str())
                        .map(|value| value == schedule.id)
                })
                .unwrap_or(false)
        });

    let Some(source_type) = source_type else {
        if let Some(source) = existing {
            crate::db::io_bus::delete_inbound(&state.db, &source.id).await?;
        }
        return Ok(());
    };

    let original = serde_json::from_str::<serde_json::Value>(&schedule.trigger_config_json)
        .unwrap_or_else(|_| serde_json::json!({}));
    let mut config = original.clone();
    if source_type == "WEBHOOK_HTTP" {
        crate::io_bus::ensure_webhook_secret_with(
            &state.paths,
            &mut config,
            &format!("schedule-{}", schedule.id),
        )?;
        // Do not keep the plaintext secret in the schedule row. The source
        // config carries only `secret_ref`.
        if config != original {
            let sanitized = config.to_string();
            if let Err(error) =
                crate::db::schedules::set_trigger_config(&state.db, &schedule.id, &sanitized).await
            {
                tracing::warn!(schedule = %schedule.id, error = %error, "routine config secret cleanup failed");
            }
        }
    }
    crate::io_bus::secrets_guard::validate_no_plaintext_secrets(&config)?;
    if let Some(object) = config.as_object_mut() {
        object.insert(
            "schedule_id".into(),
            serde_json::Value::String(schedule.id.clone()),
        );
    }
    let input = crate::db::io_bus::InboundInput {
        name: format!("Rotina: {}", schedule.name),
        source_type: source_type.into(),
        config,
        target_agent: schedule.agent_name.clone(),
        target_column: Some("TODO".into()),
        auto_execute: true,
        is_enabled: schedule.is_active,
    };
    if let Some(source) = existing {
        crate::db::io_bus::update_inbound(&state.db, &source.id, input).await?;
    } else {
        crate::db::io_bus::create_inbound(&state.db, &schedule.workspace_id, input).await?;
    }
    Ok(())
}

pub(crate) async fn remove_trigger_sources(
    state: &Arc<AppState>,
    schedule_id: &str,
) -> AppResult<()> {
    let schedule = schedules::get(&state.db, schedule_id).await?;
    for source in crate::db::io_bus::list_inbound(&state.db, &schedule.workspace_id).await? {
        let linked = serde_json::from_str::<serde_json::Value>(&source.config_json)
            .ok()
            .and_then(|config| {
                config
                    .get("schedule_id")
                    .and_then(|value| value.as_str())
                    .map(|value| value == schedule_id)
            })
            .unwrap_or(false);
        if linked {
            crate::db::io_bus::delete_inbound(&state.db, &source.id).await?;
        }
    }
    Ok(())
}

/// Rebuild every routine source. Called once at boot so routines created
/// before the trigger plumbing existed keep working.
pub async fn sync_all_trigger_sources(state: &Arc<AppState>) -> AppResult<()> {
    for workspace in crate::db::workspaces::list(&state.db).await? {
        for schedule in schedules::list(&state.db, &workspace.id).await? {
            if let Err(error) = sync_trigger_source(state, &schedule).await {
                tracing::warn!(schedule = %schedule.id, error = %error, "routine source sync failed");
            }
        }
    }
    Ok(())
}
