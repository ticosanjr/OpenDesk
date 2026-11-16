//! Duplicate prevention settings shared by task creation and queue enqueue.

use crate::{
    db::settings,
    error::{AppError, AppResult},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const KEY: &str = "task_dedupe";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeSettings {
    pub enabled: bool,
    pub cooldown_minutes: i64,
}

impl Default for DedupeSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown_minutes: 5,
        }
    }
}

pub async fn get_settings(pool: &SqlitePool) -> AppResult<DedupeSettings> {
    let raw = settings::get(pool, KEY).await?;
    let Some(value) = raw else {
        return Ok(DedupeSettings::default());
    };
    let mut parsed: DedupeSettings = serde_json::from_value(value).unwrap_or_default();
    parsed.cooldown_minutes = parsed.cooldown_minutes.clamp(0, 24 * 60);
    Ok(parsed)
}

pub async fn set_settings(
    pool: &SqlitePool,
    mut settings: DedupeSettings,
) -> AppResult<DedupeSettings> {
    settings.cooldown_minutes = settings.cooldown_minutes.clamp(0, 24 * 60);
    let value = serde_json::to_value(&settings)
        .map_err(|error| AppError::Internal(format!("invalid dedupe settings: {error}")))?;
    settings::set(pool, KEY, &value).await?;
    Ok(settings)
}

/// Stable fingerprint from already normalised fields. SHA-256 keeps the key
/// short and avoids exposing task descriptions in logs or indexes.
pub fn fingerprint(parts: &[&str]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn settings_default_to_five_minutes_and_roundtrip() {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("d.db"))
            .await
            .expect("db");
        let defaults = get_settings(&pool).await.expect("defaults");
        assert!(defaults.enabled);
        assert_eq!(defaults.cooldown_minutes, 5);

        let saved = set_settings(
            &pool,
            DedupeSettings {
                enabled: false,
                cooldown_minutes: 5000,
            },
        )
        .await
        .expect("save");
        assert!(!saved.enabled);
        assert_eq!(saved.cooldown_minutes, 1440);
    }

    #[test]
    fn normalizes_whitespace_and_case() {
        assert_eq!(normalize_text("  Revisar   Proposta "), "revisar proposta");
        assert_eq!(fingerprint(&["a", "b"]), fingerprint(&["a", "b"]));
        assert_ne!(fingerprint(&["a", "b"]), fingerprint(&["ab", ""]));
    }
}
