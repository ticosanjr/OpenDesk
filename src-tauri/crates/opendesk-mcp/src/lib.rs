//! OpenDesk MCP gateway: config server, pool, transports and guardrails.

pub mod config;
pub mod error;
pub mod pool;
pub mod rate;
pub mod rbac;
pub mod server;

pub use rate::{McpRateConfig, McpRateLimiter};
pub use rbac::{ToolPolicy, policy_for_agent};
