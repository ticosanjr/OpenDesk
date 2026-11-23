//! Lightweight continuous learning: deterministic lessons derived from run
//! outcomes. Nothing is installed automatically; promoting a lesson creates a
//! normal Studio proposal that still needs human approval.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[path = "learning_capture.rs"]
mod capture;
#[path = "learning_lessons.rs"]
mod lessons;
#[path = "learning_manual.rs"]
mod manual;
#[path = "learning_memory.rs"]
mod memory;
#[path = "learning_proposals.rs"]
mod proposals;
#[path = "learning_settings.rs"]
mod settings;

pub use capture::*;
pub use lessons::*;
pub use manual::*;
pub use memory::*;
pub use proposals::*;
pub use settings::*;

pub(super) const LESSON_COLUMNS: &str = "id, workspace_id, run_id, task_id, agent_name, model_id, category, \
    severity, title, detail, evidence_json, fingerprint, occurrences, status, proposal_id, \
    remembered_at, last_seen_at, created_at, updated_at";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSettings {
    pub enabled: bool,
    pub capture_successes: bool,
    pub auto_propose: bool,
    pub min_occurrences: i64,
}

impl Default for LearningSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_successes: false,
            auto_propose: false,
            min_occurrences: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LearningLesson {
    pub id: String,
    pub workspace_id: String,
    pub run_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_name: Option<String>,
    pub model_id: Option<String>,
    pub category: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub evidence_json: String,
    pub fingerprint: String,
    pub occurrences: i64,
    pub status: String,
    pub proposal_id: Option<String>,
    pub remembered_at: Option<String>,
    pub last_seen_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManualLessonInput {
    pub title: String,
    pub detail: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub remember: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningStats {
    pub total: i64,
    pub open: i64,
    pub promoted: i64,
    pub dismissed: i64,
    pub runs_analyzed: i64,
    pub failed_runs: i64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub analyzed: i64,
    pub lessons: i64,
}

#[cfg(test)]
#[path = "learning_tests.rs"]
mod tests;
