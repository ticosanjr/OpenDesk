//! Foundation shared by every OpenDesk shell.
//!
//! This crate must never depend on Tauri or on GUI types. It currently owns the
//! error contract, application paths, the realtime event envelope, text helpers
//! and the CLI token/cost estimator. Domain modules (database, queue, adapters,
//! recipes, DSH) move here in subsequent slices of the strangler refactor.

pub mod error;
pub mod events;
pub mod paths;
pub mod text;
pub mod token_estimate;
