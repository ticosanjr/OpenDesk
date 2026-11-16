//! Foundation shared by every OpenDesk shell.
//!
//! This crate must never depend on Tauri or on GUI types. It currently owns the
//! error contract, application paths, the realtime event envelope, text helpers,
//! the CLI token/cost estimator, run workspaces and isolated X11 displays.
//! Domain modules (database, queue, adapters, recipes, DSH) move here in
//! subsequent slices of the strangler refactor.

pub mod adapters;
pub mod api_endpoint;
pub mod audit;
pub mod backup;
pub mod browse;
pub mod browse_tags;
pub mod config_sync;
pub mod daemon_probe;
pub mod db;
pub mod dedupe;
pub mod desktop_integration;
pub mod dsh_router;
pub mod error;
pub mod events;
pub mod hitl;
pub mod internal_api;
pub mod mcp_config;
pub mod mcp_rbac;
pub mod mcp_server;
pub mod media;
pub mod metrics;
pub mod mtls;
pub mod nodes;
pub mod notifier;
pub mod opencode_client;
pub mod passkey;
pub mod paths;
pub mod personal_context;
pub mod push;
pub mod recipe_context;
pub mod recipe_dispatch;
pub mod recipes;
pub mod recurrence;
pub mod run_export;
pub mod run_timeline;
pub mod runtime;
pub mod sandbox;
pub mod secrets;
pub mod server_lock;
pub mod skill_cases;
pub mod skill_eval;
pub mod text;
pub mod token_estimate;
pub mod update_signing;
pub mod virtual_display;
pub mod workspace_sync;

#[cfg(test)]
pub mod test_support;
pub mod worktree;
