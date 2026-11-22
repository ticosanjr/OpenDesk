//! MCP gateway guardrails: per-agent/tool rate limits and RBAC policies.
//!
//! These modules are pure; the pool, transports and server stay in
//! `opendesk-core` until the remaining repositories move to storage.

pub mod rate;
pub mod rbac;

pub use rate::{McpRateConfig, McpRateLimiter};
pub use rbac::{ToolPolicy, policy_for_agent};
