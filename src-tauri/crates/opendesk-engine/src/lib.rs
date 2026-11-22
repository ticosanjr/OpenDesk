//! OpenDesk engine.
//!
//! Wave 2 starts with the adapters that only need domain + storage:
//! recurrence resolution and per-run worktree isolation. Queue, scheduler,
//! runner, telemetry, router and adapters follow as their repositories move
//! out of `opendesk-core`.

pub mod adapters;
pub mod api_endpoint;
pub mod browse;
pub mod browse_tags;
pub mod config_sync;
pub mod daemon_probe;
pub mod dsh_router;
pub mod error;
pub mod media;
pub mod notifier;
pub mod opencode_client;
pub mod personal_context;
pub mod process;
pub mod queue_ports;
pub mod recipe_dispatch;
pub mod run_timeline;
pub mod runtime;
pub mod sandbox;
pub mod scheduler;
pub mod text;
pub mod token_estimate;
pub mod worktree;

#[cfg(test)]
#[path = "test_support.rs"]
pub mod test_support;
