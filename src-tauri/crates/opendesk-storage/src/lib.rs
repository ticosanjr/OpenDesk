//! OpenDesk SQLite storage.
//!
//! - `pool`: connection options, pragmas, legacy-V1 guard and migrations;
//! - `writer`: the serialized write side (one connection);
//! - `write_buffer`: coalescing batch for heartbeats and presence;
//! - `repos`: repositories. `conversations` is the unified chat/comment/HITL/
//!   turn stream; the remaining V1 repositories migrate here across waves.

pub mod error;
pub mod pool;
pub mod repos;
pub mod write_buffer;
pub mod writer;

pub use error::{AppError, AppResult};
pub use pool::{Storage, StorageOptions, init};
pub use writer::Writer;
