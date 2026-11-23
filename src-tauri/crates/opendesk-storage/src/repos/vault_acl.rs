//! ACL validation and replacement for Vault entries.

use super::{SUBJECT_KINDS, VaultAcl, VaultAclInput, VaultEntry, get_metadata};
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

fn validate_acl(entry: &VaultEntry, acls: &[VaultAclInput]) -> AppResult<()> {
    for acl in acls {
        let kind = acl.subject_kind.trim().to_lowercase();
        if !SUBJECT_KINDS.contains(&kind.as_str()) {
            return Err(AppError::BadRequest(format!(
                "vault: subject_kind inválido '{}'",
                acl.subject_kind
            )));
        }
        if acl.subject_id.trim().is_empty() {
            return Err(AppError::BadRequest(
                "vault: subject_id é obrigatório".into(),
            ));
        }
        if acl.subject_id.trim() == "*" && !entry.requires_hitl {
            return Err(AppError::BadRequest(
                "vault: subject_id '*' exige requires_hitl=true".into(),
            ));
        }
    }
    Ok(())
}

pub async fn set_acl(
    pool: &SqlitePool,
    id: &str,
    entry: &VaultEntry,
    acls: &[VaultAclInput],
) -> AppResult<()> {
    validate_acl(entry, acls)?;
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM vault_acls WHERE entry_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for acl in acls {
        sqlx::query(
            "INSERT INTO vault_acls (entry_id, subject_kind, subject_id, can_inject)
             VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(acl.subject_kind.trim().to_lowercase())
        .bind(acl.subject_id.trim())
        .bind(acl.can_inject)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn list_acl(pool: &SqlitePool, id: &str) -> AppResult<Vec<VaultAcl>> {
    get_metadata(pool, id).await?;
    Ok(sqlx::query_as::<_, VaultAcl>(
        "SELECT entry_id, subject_kind, subject_id, can_inject
         FROM vault_acls WHERE entry_id = ? ORDER BY subject_kind, subject_id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?)
}
