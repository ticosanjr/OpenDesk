//! OpenDesk engine.
//!
//! Wave 2 starts with the adapters that only need domain + storage:
//! recurrence resolution and per-run worktree isolation. Queue, scheduler,
//! runner, telemetry, router and adapters follow as their repositories move
//! out of `opendesk-core`.

pub mod error;
pub mod scheduler;
pub mod worktree;
