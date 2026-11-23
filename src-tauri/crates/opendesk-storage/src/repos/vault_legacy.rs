//! One-time import of legacy `io_bus` `*_ref` secrets into the Vault.
//!
//! The `SecretStore` remains the source of truth for edges that were not
//! migrated yet. The import is idempotent and never deletes the legacy file.

use super::{VaultAclInput, VaultEntryInput, create_entry, get_metadata, normalize_domain};
use crate::{error::AppResult, paths::AppPaths, repos::io_bus, secrets::SecretStore};
use serde_json::Value;
use sqlx::SqlitePool;

pub async fn migrate_legacy_io_secrets(pool: &SqlitePool, paths: &AppPaths) -> AppResult<usize> {
    let store = SecretStore::new(paths);
    let mut imported = 0;
    for source in io_bus::all_inbound(pool).await? {
        imported += migrate_config(
            pool,
            &store,
            Some(&source.workspace_id),
            &source.config_json,
            "sink",
            &source.id,
        )
        .await;
    }
    for sink in io_bus::all_outbound(pool).await? {
        imported += migrate_config(
            pool,
            &store,
            Some(&sink.workspace_id),
            &sink.config_json,
            "sink",
            &sink.id,
        )
        .await;
    }
    Ok(imported)
}

async fn migrate_config(
    pool: &SqlitePool,
    store: &SecretStore,
    workspace_id: Option<&str>,
    config_json: &str,
    actor_kind: &str,
    actor_id: &str,
) -> usize {
    let Ok(config) = serde_json::from_str::<Value>(config_json) else {
        return 0;
    };
    let Some(object) = config.as_object() else {
        return 0;
    };
    let mut imported = 0;
    for (key, value) in object {
        if !key.ends_with("_ref") || key == "vault_ref" {
            continue;
        }
        let Some(reference) = value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if get_metadata(pool, reference).await.is_ok() {
            continue;
        }
        let Ok(Some(secret)) = store.get("io_bus", reference) else {
            continue;
        };
        let mut acls = vec![VaultAclInput {
            subject_kind: actor_kind.into(),
            subject_id: actor_id.into(),
            can_inject: true,
        }];
        if let Some(recipe) = config.get("recipe_id").and_then(Value::as_str) {
            acls.push(VaultAclInput {
                subject_kind: "recipe".into(),
                subject_id: recipe.into(),
                can_inject: true,
            });
        }
        if let Some(agent) = config
            .get("target_agent")
            .or_else(|| config.get("agent"))
            .and_then(Value::as_str)
        {
            acls.push(VaultAclInput {
                subject_kind: "agent".into(),
                subject_id: agent.into(),
                can_inject: true,
            });
        }
        let mut domains = Vec::new();
        for field in ["url", "webhook_url", "endpoint", "homeserver", "host"] {
            if let Some(value) = config.get(field).and_then(Value::as_str)
                && let Some(domain) = normalize_domain(value)
            {
                domains.push(domain);
            }
        }
        let kind = if key.contains("ssh") {
            "ssh_key"
        } else if key.contains("totp") {
            "totp_seed"
        } else {
            "api_key"
        };
        match create_entry(
            pool,
            VaultEntryInput {
                id: reference.to_string(),
                workspace_id: workspace_id.map(str::to_string),
                kind: kind.into(),
                label: format!("Legacy {reference}"),
                username: None,
                allowed_domains: domains,
                secret,
                requires_hitl: false,
                expires_at: None,
                acls,
            },
        )
        .await
        {
            Ok(_) => imported += 1,
            Err(error) => {
                tracing::warn!(reference, error = %error, "vault legacy import skipped");
            }
        }
    }
    imported
}
