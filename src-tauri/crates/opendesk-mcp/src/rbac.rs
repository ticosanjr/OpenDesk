//! Per-agent MCP tool policy.
//!
//! Patterns are exact names or prefixes ending in `*` (e.g. `task_*`). Deny
//! wins over allow; an empty allow list means "everything not denied".

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ToolPolicy {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

impl ToolPolicy {
    pub fn allows(&self, tool: &str) -> bool {
        if self.deny.iter().any(|pattern| matches(pattern, tool)) {
            return false;
        }
        self.allow.is_empty() || self.allow.iter().any(|pattern| matches(pattern, tool))
    }
}

pub fn policy_for_agent(config: &Value, agent: &str) -> Option<ToolPolicy> {
    let entry = config.get(agent)?;
    serde_json::from_value(entry.clone()).ok()
}

fn matches(pattern: &str, tool: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        tool.starts_with(prefix)
    } else {
        pattern == tool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_and_deny_patterns_are_applied() {
        let policy = ToolPolicy {
            allow: vec!["task_*".into(), "memory_search".into()],
            deny: vec!["task_delete".into()],
        };
        assert!(policy.allows("task_list"));
        assert!(policy.allows("memory_search"));
        assert!(!policy.allows("task_delete"));
        assert!(!policy.allows("queue_control"));
        let unrestricted = ToolPolicy::default();
        assert!(unrestricted.allows("anything"));
    }

    #[test]
    fn reads_agent_policy_from_json() {
        let config = serde_json::json!({
            "build": { "allow": ["task_*"], "deny": ["task_delete"] }
        });
        let policy = policy_for_agent(&config, "build").expect("policy");
        assert!(policy.allows("task_create"));
        assert!(!policy.allows("task_delete"));
        assert!(policy_for_agent(&config, "audit").is_none());
    }
}
