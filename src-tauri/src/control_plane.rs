//! Shared control-plane bootstrap.
//!
//! Both the desktop shell and `opendeskd` use this sequence. The goal is that
//! headless mode is not a divergent copy: queue, cron, push, IO bus, recipes,
//! governance, starter packs and MCP bridges start exactly the same way. Tauri
//! only adds window, tray, shortcuts and shutdown wiring on top.

use crate::{
    app_state::AppState, db::maintenance, governance, integrations, proposals, push, recipes,
    starter_packs,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct StartOptions {
    /// Start the OpenCode daemon supervisor instead of marking it offline.
    pub daemon: bool,
    /// Start Telegram/Signal bridges when configured.
    pub integrations: bool,
}

impl Default for StartOptions {
    fn default() -> Self {
        Self {
            daemon: true,
            integrations: true,
        }
    }
}

pub async fn start(state: Arc<AppState>, options: StartOptions) -> anyhow::Result<()> {
    state.cron.attach_state(&state);

    if options.daemon {
        state.daemon.spawn_supervisor();
    } else {
        state
            .daemon
            .mark_unavailable("OpenCode daemon disabled by OPENDESK_SKIP_DAEMON")
            .await;
    }
    tokio::spawn(integrations::delivery::retry_loop(state.clone()));
    tokio::spawn(integrations::google_calendar::sync_loop(state.clone()));

    if options.integrations {
        if let Err(error) = state.signal.start_if_enabled(state.clone()).await {
            tracing::warn!(error = %error, "Signal bridge was not started");
        }
        if let Err(error) = state.telegram.start_if_enabled(state.clone()).await {
            tracing::warn!(error = %error, "Telegram bridge was not started");
        }
    }

    state.refresh_watcher().await?;
    state.queue.spawn_worker();
    state.cron.spawn();
    push::start(state.clone());
    tokio::spawn(maintenance::run_loop(state.db.clone(), state.paths.clone()));

    state.io_bus.start(state.clone()).await?;
    recipes::runtime::start(state.clone());
    match recipes::runtime::resume_runs(&state).await {
        Ok(0) => {}
        Ok(count) => tracing::info!(count, "resumed durable recipe runs"),
        Err(error) => tracing::warn!(error = %error, "could not resume recipe runs"),
    }
    governance::start_heartbeat(state.clone());

    if let Err(error) = crate::api::schedules::sync_all_trigger_sources(&state).await {
        tracing::warn!(error = %error, "routine trigger source sync failed");
    }
    if let Err(error) = crate::api::recipes::sync_all_recipe_trigger_sources(&state).await {
        tracing::warn!(error = %error, "recipe trigger source sync failed");
    }
    if let Err(error) = starter_packs::ensure_default_factory_workspace(&state).await {
        tracing::warn!(error = %error, "default factory workspace failed");
    }
    if let Err(error) = proposals::ensure_all(&state).await {
        tracing::warn!(error = %error, "could not register config MCP bridge");
    }
    Ok(())
}
