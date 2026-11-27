pub mod ab;
pub mod about;
pub mod adapters;
pub mod artifacts;
pub mod attachments;
pub mod attention;
pub mod audit;
pub mod auth;
pub mod backups;
pub mod blueprints;
pub mod browse;
pub mod budget;
pub mod bundles;
pub mod calendar;
pub mod capture;
pub mod channel_bots;
pub mod channels;
pub mod chat;
#[cfg(feature = "desktop")]
pub mod commands;
pub mod config_bundle;
pub mod core;
pub mod cost;
pub mod dedupe;
pub mod engine;
pub mod files;
pub mod google;
pub mod governance;
pub mod handoffs;
pub mod integrations;
pub mod interactions;
pub mod io_bus;
pub mod learning;
pub mod mcp;
pub mod mcp_hub;
pub mod memory;
pub mod meta;
pub mod metrics;
pub mod missed_runs;
pub mod nodes;
pub mod onboarding;
pub mod passkey;
pub mod permissions;
pub mod personal_context;
pub mod prompt_layer;
pub mod prompt_preview;
pub mod prompt_templates;
pub mod proposals;
pub mod push;
pub mod queue;
pub mod recipes;
pub mod router;
mod router_table;
pub mod runs;
pub mod sandbox;
pub mod schedules;
pub mod script_tools;
pub mod search;
pub mod seed;
pub mod self_test;
pub mod signal;
pub mod skills;
pub mod starter_packs;
pub mod studio;
pub(crate) mod studio_github;
pub(crate) mod studio_install_safety;
pub mod studio_ops;
pub mod sync;
pub mod task_watchers;
pub mod tasks;
pub mod telegram;
pub mod tickets;
pub mod tls;
pub mod updates;
pub mod vault;
pub mod voice;
pub mod worktrees;
pub mod ws;

use crate::app_state::AppState;
use std::sync::Arc;

pub async fn spawn_server(state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = state
        .take_listener()
        .ok_or_else(|| anyhow::anyhow!("OpenDesk API listener was already taken"))?;
    if crate::api::tls::TlsEnv::from_env()?.is_some() {
        let address = listener.local_addr()?;
        tracing::info!(%address, "OpenDesk local API listening with mTLS");
        crate::api::tls::start_if_configured(listener, state).await?;
        return Ok(());
    }
    let listener = tokio::net::TcpListener::from_std(listener)?;
    let address = listener.local_addr()?;
    let app = router(state);

    tracing::info!(%address, "OpenDesk local API listening");

    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            tracing::error!(error = %error, "OpenDesk API stopped");
        }
    });

    Ok(())
}

pub(crate) use router_table::router;

#[cfg(test)]
mod tests;
