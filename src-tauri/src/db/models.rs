use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub description: Option<String>,
    pub color_hex: String,
    pub default_router_policy: String,
    pub default_email_recipient: Option<String>,
    pub google_drive_folder_id: Option<String>,
    pub timezone: String,
    pub is_active: bool,
    pub created_at: String,
    pub last_opened_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorkspaceInput {
    pub name: String,
    pub root_path: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color_hex: Option<String>,
    #[serde(default)]
    pub default_router_policy: Option<String>,
    #[serde(default)]
    pub default_email_recipient: Option<String>,
    #[serde(default)]
    pub google_drive_folder_id: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWorkspaceInput {
    pub name: Option<String>,
    pub root_path: Option<String>,
    pub description: Option<String>,
    pub color_hex: Option<String>,
    pub default_router_policy: Option<String>,
    pub default_email_recipient: Option<String>,
    pub google_drive_folder_id: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AppSetting {
    pub key: String,
    pub value_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, FromRow)]
pub struct DashboardStats {
    pub runs_total: i64,
    pub runs_success: i64,
    pub runs_failed: i64,
    pub runs_running: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_cache_read: i64,
    pub tokens_cache_write: i64,
    pub estimated_cost_usd: f64,
    pub daily_cost_usd: f64,
    pub queue_running: i64,
    pub queue_waiting: i64,
    pub artifacts_total: i64,
}
