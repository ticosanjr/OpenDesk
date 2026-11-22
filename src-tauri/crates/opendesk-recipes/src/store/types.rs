use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecipeIndex {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub file_path: String,
    pub content_sha256: Option<String>,
    pub trigger_type: String,
    pub is_enabled: bool,
    pub last_error: Option<String>,
    pub last_run_at: Option<String>,
    pub total_runs: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecipeRun {
    pub id: String,
    pub recipe_id: String,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub status: String,
    pub current_step_id: Option<String>,
    pub trigger_context_json: String,
    pub step_outputs_json: String,
    pub output_dir_path: String,
    pub dry_run: bool,
    #[sqlx(default)]
    pub cancel_requested: bool,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct HitlRequest {
    pub token: String,
    pub recipe_run_id: String,
    pub workspace_id: String,
    pub checkpoint_id: String,
    pub interaction_type: String,
    pub title: Option<String>,
    pub question_markdown: Option<String>,
    pub context_json: String,
    pub options_json: String,
    pub status: String,
    pub on_timeout: String,
    pub dispatched_channels_json: String,
    pub resolved_by_channel: Option<String>,
    pub resolved_by_sender: Option<String>,
    pub resolution_type: Option<String>,
    pub resolution_payload: Option<String>,
    pub expires_at: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecipeRunNode {
    pub id: String,
    pub run_id: String,
    pub node_index: i64,
    #[sqlx(default)]
    pub node_path: Option<String>,
    #[sqlx(default)]
    pub parent_path: Option<String>,
    pub node_id: String,
    pub kind: String,
    #[sqlx(default)]
    pub item_key: Option<String>,
    #[sqlx(default)]
    pub join_policy: Option<String>,
    pub status: String,
    pub attempts: i64,
    pub session_id: Option<String>,
    pub output_json: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}
