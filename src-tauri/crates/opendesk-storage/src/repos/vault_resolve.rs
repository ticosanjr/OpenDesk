//! Domain/ACL/HITL checks and audit for Vault edge resolution.

use super::{VaultAudit, VaultEntry, get_entry, normalize_domain};
use crate::{
    error::{AppError, AppResult},
    secrets::open,
};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(super) struct EdgeActor<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub run_id: Option<&'a str>,
    pub target_domain: Option<&'a str>,
}

fn allowed_domains(entry: &VaultEntry) -> Vec<String> {
    serde_json::from_str(&entry.allowed_domains_json).unwrap_or_default()
}

async fn record_audit(
    pool: &SqlitePool,
    entry_id: &str,
    actor: EdgeActor<'_>,
    outcome: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO vault_audit
            (id, entry_id, actor_kind, actor_id, run_id, target_domain, outcome)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(entry_id)
    .bind(actor.kind)
    .bind(actor.id)
    .bind(actor.run_id)
    .bind(actor.target_domain)
    .bind(outcome)
    .execute(pool)
    .await?;
    Ok(())
}

async fn deny(
    pool: &SqlitePool,
    entry_id: &str,
    actor: EdgeActor<'_>,
    outcome: &str,
    message: &str,
) -> AppError {
    let _ = record_audit(pool, entry_id, actor, outcome).await;
    AppError::BadRequest(format!("vault: {message}"))
}

pub async fn list_audit(
    pool: &SqlitePool,
    entry_id: Option<&str>,
    limit: i64,
) -> AppResult<Vec<VaultAudit>> {
    let limit = limit.clamp(1, 200);
    match entry_id {
        Some(entry_id) => Ok(sqlx::query_as::<_, VaultAudit>(
            "SELECT id, entry_id, actor_kind, actor_id, run_id, target_domain, outcome, created_at
             FROM vault_audit WHERE entry_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(entry_id)
        .bind(limit)
        .fetch_all(pool)
        .await?),
        None => Ok(sqlx::query_as::<_, VaultAudit>(
            "SELECT id, entry_id, actor_kind, actor_id, run_id, target_domain, outcome, created_at
             FROM vault_audit ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?),
    }
}

/// Resolves a secret for a specific edge. The only function that decrypts.
pub async fn resolve_for_edge(
    pool: &SqlitePool,
    id: &str,
    actor_kind: &str,
    actor_id: &str,
    run_id: Option<&str>,
    target_domain: Option<&str>,
    hitl_approved: bool,
) -> AppResult<String> {
    let entry = get_entry(pool, id).await?;
    let actor = EdgeActor {
        kind: actor_kind,
        id: actor_id,
        run_id,
        target_domain,
    };
    if let Some(expires_at) = entry.expires_at.as_deref()
        && let Ok(expires_at) = DateTime::parse_from_rfc3339(expires_at)
        && Utc::now() > expires_at.with_timezone(&Utc)
    {
        return Err(deny(pool, id, actor, "EXPIRED", "credencial expirada").await);
    }
    let allowed: Option<bool> = sqlx::query_scalar(
        "SELECT can_inject FROM vault_acls
         WHERE entry_id = ? AND subject_kind = ? AND subject_id = ?",
    )
    .bind(id)
    .bind(actor_kind)
    .bind(actor_id)
    .fetch_optional(pool)
    .await?;
    if !allowed.unwrap_or(false) {
        return Err(deny(pool, id, actor, "DENIED_ACL", "ACL negou a injeção").await);
    }
    if let Some(target) = target_domain {
        let target = normalize_domain(target).ok_or_else(|| {
            AppError::BadRequest(format!("vault: domínio de destino inválido '{target}'"))
        })?;
        if !allowed_domains(&entry)
            .iter()
            .any(|domain| domain == &target)
        {
            return Err(deny(
                pool,
                id,
                EdgeActor {
                    target_domain: Some(&target),
                    ..actor
                },
                "DENIED_DOMAIN",
                "domínio de destino não permitido",
            )
            .await);
        }
    }
    if entry.requires_hitl && !hitl_approved {
        return Err(deny(
            pool,
            id,
            actor,
            "DENIED_HITL",
            "confirmação HITL obrigatória",
        )
        .await);
    }
    let secret = open(&entry.secret_nonce, &entry.secret_ciphertext)?;
    record_audit(pool, id, actor, "INJECTED").await?;
    let _ = sqlx::query(
        "UPDATE vault_entries SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await;
    Ok(secret)
}
