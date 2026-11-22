//! Signal transport and protocol adapter.
//!
//! Host-agnostic half of the Signal integration: configuration persistence,
//! envelope parsing and allowlist policy, `signal-cli` invocation, and the
//! daemon/link bridge. The shell keeps only the callbacks that need
//! `AppState` (chat routing, HITL, routines, task notifications).

mod bridge;
mod config;
mod protocol;
mod send;

pub use bridge::{SignalBridge, SignalEventHandler};
pub use config::{
    SignalConfig, SignalStatus, default_cli_path, detect_cli, load, save, signal_path,
};
pub use protocol::{IncomingSignal, allowed, parse_incoming};
pub use send::{send_text, send_with_attachments};
