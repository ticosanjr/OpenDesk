use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkspaceIntegration {
    pub workspace_id: String,
    pub smtp_host: Option<String>,
    pub smtp_port: i64,
    pub smtp_username: Option<String>,
    pub smtp_from_email: Option<String>,
    pub smtp_use_tls: bool,
    pub smtp_secret_ref: Option<String>,
    pub google_account_email: Option<String>,
    pub google_calendar_id: String,
    pub google_scopes: Option<String>,
    pub google_keep_sync_enabled: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SmtpSettingsInput {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i64,
    pub username: Option<String>,
    pub from_email: String,
    #[serde(default = "default_tls")]
    pub use_tls: bool,
}

fn default_port() -> i64 {
    587
}

fn default_tls() -> bool {
    true
}

const COLUMNS: &str = "workspace_id, smtp_host, smtp_port, smtp_username, smtp_from_email, \
    smtp_use_tls, smtp_secret_ref, google_account_email, google_calendar_id, google_scopes, \
    google_keep_sync_enabled, updated_at";

pub async fn get(pool: &SqlitePool, workspace_id: &str) -> AppResult<Option<WorkspaceIntegration>> {
    let query = format!("SELECT {COLUMNS} FROM workspace_integrations WHERE workspace_id = ?");
    Ok(sqlx::query_as::<_, WorkspaceIntegration>(&query)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn upsert_smtp(
    pool: &SqlitePool,
    workspace_id: &str,
    input: &SmtpSettingsInput,
    secret_ref: Option<&str>,
) -> AppResult<WorkspaceIntegration> {
    sqlx::query(
        "INSERT INTO workspace_integrations (
            workspace_id, smtp_host, smtp_port, smtp_username, smtp_from_email,
            smtp_use_tls, smtp_secret_ref, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(workspace_id) DO UPDATE SET
            smtp_host = excluded.smtp_host,
            smtp_port = excluded.smtp_port,
            smtp_username = excluded.smtp_username,
            smtp_from_email = excluded.smtp_from_email,
            smtp_use_tls = excluded.smtp_use_tls,
            smtp_secret_ref = COALESCE(excluded.smtp_secret_ref, workspace_integrations.smtp_secret_ref),
            updated_at = excluded.updated_at",
    )
    .bind(workspace_id)
    .bind(input.host.trim())
    .bind(input.port)
    .bind(input.username.as_deref())
    .bind(input.from_email.trim())
    .bind(input.use_tls)
    .bind(secret_ref)
    .execute(pool)
    .await?;

    get(pool, workspace_id)
        .await?
        .ok_or_else(|| crate::error::AppError::Internal("integration row missing".into()))
}

pub async fn upsert_google(
    pool: &SqlitePool,
    workspace_id: &str,
    email: &str,
    scopes: &str,
) -> AppResult<WorkspaceIntegration> {
    sqlx::query(
        "INSERT INTO workspace_integrations (
            workspace_id, google_account_email, google_scopes, updated_at
         ) VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(workspace_id) DO UPDATE SET
            google_account_email = excluded.google_account_email,
            google_scopes = excluded.google_scopes,
            updated_at = excluded.updated_at",
    )
    .bind(workspace_id)
    .bind(email)
    .bind(scopes)
    .execute(pool)
    .await?;

    get(pool, workspace_id)
        .await?
        .ok_or_else(|| crate::error::AppError::Internal("integration row missing".into()))
}

pub async fn clear_google(pool: &SqlitePool, workspace_id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE workspace_integrations SET
            google_account_email = NULL, google_scopes = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE workspace_id = ?",
    )
    .bind(workspace_id)
    .execute(pool)
    .await?;
    Ok(())
}
