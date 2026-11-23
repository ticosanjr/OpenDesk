//! Agent governance: hierarchy, goals, budgets, heartbeats and delegations.
//! Agent definitions remain native OpenCode files; this module stores only
//! operational governance state in SQLite.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[path = "governance_delegation.rs"]
mod delegation;
#[path = "governance_goals.rs"]
mod goals;
#[path = "governance_heartbeat.rs"]
mod heartbeat;
#[path = "governance_metrics.rs"]
mod metrics;
#[path = "governance_profiles.rs"]
mod profiles;

pub use delegation::*;
pub use goals::*;
pub use heartbeat::*;
pub use metrics::*;
pub use profiles::*;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentProfile {
    pub workspace_id: String,
    pub agent_id: String,
    pub role: Option<String>,
    pub team: Option<String>,
    pub manager_id: Option<String>,
    pub monthly_budget_usd: Option<f64>,
    pub concurrency_quota: i64,
    pub heartbeat_enabled: bool,
    pub heartbeat_minutes: i64,
    pub last_heartbeat_at: Option<String>,
    pub next_heartbeat_at: Option<String>,
    pub notes: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentProfileInput {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub manager_id: Option<String>,
    #[serde(default)]
    pub monthly_budget_usd: Option<f64>,
    #[serde(default)]
    pub concurrency_quota: Option<i64>,
    #[serde(default)]
    pub heartbeat_enabled: Option<bool>,
    #[serde(default)]
    pub heartbeat_minutes: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentGoal {
    pub id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub title: String,
    pub description: Option<String>,
    pub metric: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: f64,
    pub unit: Option<String>,
    pub due_at: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentGoalInput {
    pub agent_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub target_value: Option<f64>,
    #[serde(default)]
    pub current_value: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentDelegation {
    pub id: String,
    pub workspace_id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub title: String,
    pub prompt: String,
    pub status: String,
    pub task_id: Option<String>,
    pub requested_by: Option<String>,
    pub requested_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DelegationInput {
    pub from_agent: String,
    pub to_agent: String,
    pub title: String,
    pub prompt: String,
    #[serde(default)]
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AgentPerformance {
    pub agent_id: String,
    pub runs: i64,
    pub success: i64,
    pub failed: i64,
    pub cancelled: i64,
    pub cost_usd: f64,
    pub avg_duration_ms: f64,
    pub tokens: i64,
    #[sqlx(default)]
    pub success_rate: f64,
}

#[cfg(test)]
#[path = "governance_tests.rs"]
mod tests;
