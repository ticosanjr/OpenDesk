//! MCP control plane, bridge, hub, sampling and session registry.
//!
//! Protocol helpers that do not need shell state live in `opendesk-mcp`; the
//! modules here own the OpenDesk-specific tools, HTTP bridge and in-memory
//! session/async-request state.

pub mod bridge;
pub mod control;
pub mod hub;
pub mod sampling;
pub use opendesk_mcp::session;
