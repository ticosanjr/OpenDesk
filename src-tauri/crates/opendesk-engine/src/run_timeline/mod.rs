use ::opendesk_storage::repos::runs::TaskRun;
use serde::Serialize;

mod accumulator;
mod build;
mod finish;
mod helpers;

#[cfg(test)]
mod tests;

pub use build::timeline_from_events;

const MAX_TEXT: usize = 64 * 1024;
const MAX_TOOL_OUTPUT: usize = 6 * 1024;

#[derive(Debug, Clone, Serialize, Default)]
pub struct RunTimeline {
    pub run: Option<TaskRun>,
    pub root_run_id: Option<String>,
    pub attempts: Vec<RunAttempt>,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: i64,
    pub events_count: usize,
    pub steps: Vec<TimelineStep>,
    pub thinking: TimelineSection,
    pub executed: TimelineSection,
    pub result: TimelineSection,
    pub log_missing: bool,
    pub workspace_root: Option<String>,
    pub output_directory: Option<String>,
    pub output_directory_relative: Option<String>,
    pub artifacts: Vec<TimelineArtifact>,
    pub inputs: Vec<TimelineInput>,
    pub events: Vec<TimelineEvent>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineInput {
    pub name: String,
    pub path: String,
    pub absolute_path: String,
    pub size: Option<u64>,
    pub source: Option<String>,
    pub trigger_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RunAttempt {
    pub id: String,
    pub attempt_number: i64,
    pub status: String,
    pub phase: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost_usd: f64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineArtifact {
    pub id: String,
    pub file_path: String,
    pub absolute_path: String,
    pub action_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineEvent {
    pub kind: String,
    pub role: String,
    pub text: String,
    /// Original OpenCode event type, including unknown/future events.
    pub event_type: String,
    /// Raw JSON payload for generic events, truncated for transport.
    pub data: String,
    pub tool: Option<TimelineTool>,
    pub run_id: Option<String>,
    pub attempt_number: Option<i64>,
    pub step_index: Option<usize>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineSection {
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
    pub text: String,
    pub tools: Vec<TimelineTool>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineStep {
    pub index: usize,
    pub attempt_number: Option<i64>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub cost_usd: f64,
    pub finish: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineTool {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub input: String,
    pub output: String,
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
    pub output_truncated: bool,
}
