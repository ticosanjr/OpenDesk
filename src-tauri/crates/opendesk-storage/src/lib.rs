//! OpenDesk SQLite storage.
//!
//! - `pool`: connection options, pragmas, legacy-V1 guard and migrations;
//! - `writer`: the serialized write side (one connection);
//! - `write_buffer`: coalescing batch for heartbeats and presence;
//! - `repos`: repositories. `conversations` is the unified chat/comment/HITL/
//!   turn stream; the remaining V1 repositories migrate here across waves.

pub mod db;
pub mod dedupe;
pub mod error;
pub mod events;
pub mod integrity;
pub mod legacy;
pub mod maintenance;
pub mod paths;
pub mod pool;
pub mod repos;
pub mod secrets;
pub mod server_lock;
pub mod write_buffer;
pub mod writer;

#[cfg(test)]
#[path = "test_support_tests.rs"]
pub mod test_support;

#[cfg(test)]
mod migration_tests;

pub use error::{AppError, AppResult};
pub use pool::{Storage, StorageOptions, init};
pub use writer::Writer;
