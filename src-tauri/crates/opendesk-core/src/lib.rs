//! Foundation shared by every OpenDesk shell.
//!
//! This crate must never depend on Tauri or on GUI types. It currently owns the
//! error contract, application paths, the realtime event envelope, text helpers,
//! the CLI token/cost estimator, run workspaces and isolated X11 displays.
//! Domain modules (database, queue, adapters, recipes, DSH) move here in
//! subsequent slices of the strangler refactor.

pub mod error;
pub mod events;
pub mod paths;
pub mod server_lock;
pub mod text;
pub mod token_estimate;
pub mod virtual_display;
pub mod workspace_sync;
pub mod worktree;
