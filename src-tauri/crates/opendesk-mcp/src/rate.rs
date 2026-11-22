//! Per-agent/tool rate limits for the MCP gateway (E3).
//!
//! The limiter is in-memory: a restart resets windows. Configuration lives in
//! the `mcp_tool_rates` setting so operators can tune it without redeploying.

use serde::Deserialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

const WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default, Deserialize)]
pub struct McpRateConfig {
    #[serde(default)]
    pub default: Option<u32>,
    /// `{ "build": { "task_list": 30 } }`.
    #[serde(default)]
    pub agents: HashMap<String, HashMap<String, u32>>,
}

impl McpRateConfig {
    pub fn limit_for(&self, agent: &str, tool: &str) -> u32 {
        self.agents
            .get(agent)
            .and_then(|tools| tools.get(tool).copied())
            .or(self.default)
            .unwrap_or(0)
    }
}

pub struct McpRateLimiter {
    windows: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl Default for McpRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl McpRateLimiter {
    pub fn new() -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// A limit of 0 means "unlimited". Returns the retry delay in seconds when
    /// the caller is over the window.
    pub fn check(&self, agent: &str, tool: &str, limit: u32) -> Result<(), u64> {
        if limit == 0 {
            return Ok(());
        }
        let now = Instant::now();
        let key = format!("{agent}:{tool}");
        let mut windows = self
            .windows
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let window = windows.entry(key).or_default();
        while window
            .front()
            .is_some_and(|started| now.duration_since(*started) >= WINDOW)
        {
            window.pop_front();
        }
        if window.len() >= limit as usize {
            let retry = window
                .front()
                .map(|started| WINDOW.saturating_sub(now.duration_since(*started)))
                .unwrap_or(WINDOW);
            return Err(retry.as_secs().max(1));
        }
        window.push_back(now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_per_agent_tool_limits() {
        let config: McpRateConfig = serde_json::from_value(serde_json::json!({
            "default": 2,
            "agents": { "audit": { "task_list": 1 } }
        }))
        .expect("config");
        assert_eq!(config.limit_for("build", "task_list"), 2);
        assert_eq!(config.limit_for("audit", "task_list"), 1);
        let limiter = McpRateLimiter::new();
        assert!(limiter.check("build", "task_list", 2).is_ok());
        assert!(limiter.check("build", "task_list", 2).is_ok());
        assert!(limiter.check("build", "task_list", 2).is_err());
        assert!(limiter.check("audit", "task_list", 1).is_ok());
        assert!(limiter.check("audit", "task_list", 1).is_err());
        assert!(limiter.check("build", "task_create", 2).is_ok());
        assert!(limiter.check("build", "task_create", 0).is_ok());
    }
}
