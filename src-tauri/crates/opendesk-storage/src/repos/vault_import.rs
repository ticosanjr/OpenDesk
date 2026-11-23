//! `.env` import for the Vault.

use super::{VaultEntryInput, VaultEntryMetadata, create_entry, get_metadata, validate_id};
use crate::error::AppResult;
use sqlx::SqlitePool;

/// Imports `KEY=VALUE` lines as API keys. Existing ids are skipped.
pub async fn import_env(
    pool: &SqlitePool,
    workspace_id: Option<&str>,
    content: &str,
) -> AppResult<Vec<VaultEntryMetadata>> {
    let mut imported = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let id = validate_id(key)?;
        if get_metadata(pool, &id).await.is_ok() {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if value.len() < 8 {
            continue;
        }
        let entry = create_entry(
            pool,
            VaultEntryInput {
                id,
                workspace_id: workspace_id.map(str::to_string),
                kind: "api_key".into(),
                label: key.trim().to_string(),
                username: None,
                allowed_domains: Vec::new(),
                secret: value.to_string(),
                requires_hitl: true,
                expires_at: None,
                acls: Vec::new(),
            },
        )
        .await?;
        imported.push(entry);
    }
    Ok(imported)
}
