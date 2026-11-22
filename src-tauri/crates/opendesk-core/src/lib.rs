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
pub use opendesk_storage::dedupe;
pub mod desktop_integration;
pub mod dsh_router;
pub mod error;
pub use opendesk_storage::events;
pub mod hitl;
pub mod internal_api;
pub mod mcp_config;
pub mod mcp_pool;
pub use opendesk_mcp::rate as mcp_rate;
pub use opendesk_mcp::rbac as mcp_rbac;
pub mod mcp_server;
pub mod media;
pub mod metrics;
pub use opendesk_enterprise::auth::mtls;
pub mod nodes;
pub mod notifier;
pub mod opencode_client;
pub mod passkey;
pub use opendesk_storage::paths;
pub mod personal_context;
pub mod push;
pub mod queue_ports;
pub mod queue_scheduler;
pub mod recipe_context;
pub mod recipe_dispatch;
pub mod recipes;
pub use opendesk_engine::scheduler::recurrence;
pub mod run_export;
pub mod run_timeline;
pub mod runtime;
pub mod sandbox;
pub use opendesk_storage::secrets;
pub use opendesk_storage::server_lock;
pub mod skill_cases;
pub mod skill_eval;
pub mod skill_shadow;
pub mod text;
pub mod token_estimate;
pub use opendesk_enterprise::auth::update_signing;
pub use opendesk_enterprise::computer::virtual_display;
pub mod workspace_sync;

#[cfg(test)]
#[cfg(test)]
#[path = "test_support_tests.rs"]
pub mod test_support;
pub use opendesk_engine::worktree;
