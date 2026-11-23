//! Credential Vault repository.
//!
//! The plaintext never leaves [`resolve_for_edge`]: every other function
//! returns metadata, ACLs or audit rows. Domain, expiry, ACL and HITL checks
//! are fail-closed and every attempt is audited.

use crate::{
    error::{AppError, AppResult},
    secrets::seal,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[path = "vault_acl.rs"]
mod acl;
#[path = "vault_import.rs"]
mod import;
#[path = "vault_legacy.rs"]
mod legacy;
#[path = "vault_resolve.rs"]
mod resolve;
pub use acl::{list_acl, set_acl};
pub use import::import_env;
pub use legacy::migrate_legacy_io_secrets;
pub use resolve::{list_audit, resolve_for_edge};

pub const KINDS: [&str; 6] = [
    "api_key",
    "login",
    "bearer_token",
    "totp_seed",
    "ssh_key",
    "header_bundle",
];
pub const SUBJECT_KINDS: [&str; 4] = ["agent", "recipe", "mcp", "sink"];
pub const OUTCOMES: [&str; 5] = [
    "INJECTED",
    "DENIED_ACL",
    "DENIED_DOMAIN",
    "DENIED_HITL",
    "EXPIRED",
];

const METADATA_COLUMNS: &str = "id, workspace_id, kind, label, username, allowed_domains_json, \
    requires_hitl, expires_at, last_used_at, created_at, updated_at";
const ENTRY_COLUMNS: &str = "id, workspace_id, kind, label, username, allowed_domains_json, \
    secret_ciphertext, secret_nonce, requires_hitl, expires_at, last_used_at, created_at, updated_at";

/// Full Vault row, including the sealed payload. Never serialize this into an
/// API response, an MCP tool result or a rendered prompt.
#[derive(Debug, Clone, FromRow)]
pub struct VaultEntry {
    pub id: String,
    pub workspace_id: Option<String>,
    pub kind: String,
    pub label: String,
    pub username: Option<String>,
    pub allowed_domains_json: String,
    pub secret_ciphertext: Vec<u8>,
    pub secret_nonce: Vec<u8>,
    pub requires_hitl: bool,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Safe projection for lists, detail views and MCP discovery.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VaultEntryMetadata {
    pub id: String,
    pub workspace_id: Option<String>,
    pub kind: String,
    pub label: String,
    pub username: Option<String>,
    pub allowed_domains_json: String,
    pub requires_hitl: bool,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<VaultEntry> for VaultEntryMetadata {
    fn from(entry: VaultEntry) -> Self {
        Self {
            id: entry.id,
            workspace_id: entry.workspace_id,
            kind: entry.kind,
            label: entry.label,
            username: entry.username,
            allowed_domains_json: entry.allowed_domains_json,
            requires_hitl: entry.requires_hitl,
            expires_at: entry.expires_at,
            last_used_at: entry.last_used_at,
            created_at: entry.created_at,
            updated_at: entry.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VaultAcl {
    pub entry_id: String,
    pub subject_kind: String,
    pub subject_id: String,
    pub can_inject: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VaultAudit {
    pub id: String,
    pub entry_id: String,
    pub actor_kind: String,
    pub actor_id: String,
    pub run_id: Option<String>,
    pub target_domain: Option<String>,
    pub outcome: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VaultAclInput {
    pub subject_kind: String,
    pub subject_id: String,
    #[serde(default = "default_true")]
    pub can_inject: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VaultEntryInput {
    pub id: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub kind: String,
    pub label: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    pub secret: String,
    #[serde(default = "default_true")]
    pub requires_hitl: bool,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub acls: Vec<VaultAclInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VaultMetadataInput {
    pub label: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default = "default_true")]
    pub requires_hitl: bool,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub secret: Option<String>,
}

fn default_true() -> bool {
    true
}

pub(super) fn validate_id(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 120 {
        return Err(AppError::BadRequest(
            "vault: id deve ter entre 1 e 120 caracteres".into(),
        ));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(AppError::BadRequest(
            "vault: id aceita apenas letras, números, '-', '_' e '.'".into(),
        ));
    }
    Ok(value.to_string())
}

fn validate_kind(value: &str) -> AppResult<String> {
    let value = value.trim().to_lowercase();
    if KINDS.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "vault: kind inválido '{value}'"
        )))
    }
}

pub fn normalize_domain(raw: &str) -> Option<String> {
    let value = raw.trim().to_lowercase();
    let value = value.split("://").last().unwrap_or(&value);
    let value = value.split('/').next().unwrap_or(value);
    let value = value.split(':').next().unwrap_or(value);
    let value = value.trim_end_matches('.');
    if value.is_empty()
        || value.contains('*')
        || value.contains(' ')
        || !value.contains('.') && value != "localhost"
    {
        return None;
    }
    Some(value.to_string())
}

fn validate_domains(kind: &str, domains: &[String]) -> AppResult<Vec<String>> {
    let mut output = Vec::new();
    for domain in domains {
        let domain = normalize_domain(domain)
            .ok_or_else(|| AppError::BadRequest(format!("vault: domínio inválido '{domain}'")))?;
        if !output.contains(&domain) {
            output.push(domain);
        }
    }
    if output.is_empty() && matches!(kind, "login" | "bearer_token" | "header_bundle") {
        return Err(AppError::BadRequest(
            "vault: este tipo exige ao menos um domínio permitido".into(),
        ));
    }
    Ok(output)
}

fn validate_secret(kind: &str, secret: &str) -> AppResult<()> {
    let minimum = if kind == "totp_seed" { 16 } else { 8 };
    if secret.len() < minimum {
        return Err(AppError::BadRequest(format!(
            "vault: segredo curto demais (mínimo {minimum} caracteres)"
        )));
    }
    if secret.chars().any(char::is_control) {
        return Err(AppError::BadRequest(
            "vault: segredo não pode conter caracteres de controle".into(),
        ));
    }
    Ok(())
}

fn validate_expiry(value: Option<&str>) -> AppResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|_| AppError::BadRequest("vault: expires_at deve ser RFC3339".into()))?;
    Ok(Some(parsed.with_timezone(&Utc).to_rfc3339()))
}

fn domains_json(domains: &[String]) -> String {
    serde_json::to_string(domains).unwrap_or_else(|_| "[]".into())
}

pub async fn create_entry(
    pool: &SqlitePool,
    input: VaultEntryInput,
) -> AppResult<VaultEntryMetadata> {
    let id = validate_id(&input.id)?;
    let kind = validate_kind(&input.kind)?;
    let label = input.label.trim();
    if label.is_empty() {
        return Err(AppError::BadRequest("vault: label é obrigatório".into()));
    }
    let domains = validate_domains(&kind, &input.allowed_domains)?;
    validate_secret(&kind, &input.secret)?;
    let expires_at = validate_expiry(input.expires_at.as_deref())?;
    let sealed = seal(&input.secret)?;
    sqlx::query(
        "INSERT INTO vault_entries
            (id, workspace_id, kind, label, username, allowed_domains_json,
             secret_ciphertext, secret_nonce, requires_hitl, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(input.workspace_id.as_deref())
    .bind(&kind)
    .bind(label)
    .bind(input.username.as_deref())
    .bind(domains_json(&domains))
    .bind(&sealed.ciphertext)
    .bind(sealed.nonce.to_vec())
    .bind(input.requires_hitl)
    .bind(expires_at.as_deref())
    .execute(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(database) if database.is_unique_violation() => {
            AppError::Conflict(format!("vault: já existe uma entrada '{id}'"))
        }
        other => AppError::Db(other),
    })?;
    if !input.acls.is_empty() {
        let entry = get_entry(pool, &id).await?;
        set_acl(pool, &id, &entry, &input.acls).await?;
    }
    get_metadata(pool, &id).await
}

pub async fn list_entries(
    pool: &SqlitePool,
    workspace_id: Option<&str>,
) -> AppResult<Vec<VaultEntryMetadata>> {
    let query = match workspace_id {
        Some(_) => format!(
            "SELECT {METADATA_COLUMNS} FROM vault_entries
             WHERE workspace_id IS NULL OR workspace_id = ? ORDER BY label"
        ),
        None => format!(
            "SELECT {METADATA_COLUMNS} FROM vault_entries
             WHERE workspace_id IS NULL ORDER BY label"
        ),
    };
    let mut request = sqlx::query_as::<_, VaultEntryMetadata>(&query);
    if let Some(workspace_id) = workspace_id {
        request = request.bind(workspace_id);
    }
    Ok(request.fetch_all(pool).await?)
}

pub async fn get_metadata(pool: &SqlitePool, id: &str) -> AppResult<VaultEntryMetadata> {
    let query = format!("SELECT {METADATA_COLUMNS} FROM vault_entries WHERE id = ?");
    sqlx::query_as::<_, VaultEntryMetadata>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("vault {id}")))
}

pub async fn get_entry(pool: &SqlitePool, id: &str) -> AppResult<VaultEntry> {
    let query = format!("SELECT {ENTRY_COLUMNS} FROM vault_entries WHERE id = ?");
    sqlx::query_as::<_, VaultEntry>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("vault {id}")))
}

pub async fn update_entry(
    pool: &SqlitePool,
    id: &str,
    input: VaultMetadataInput,
) -> AppResult<VaultEntryMetadata> {
    let current = get_entry(pool, id).await?;
    let kind = current.kind.clone();
    let label = input.label.trim();
    if label.is_empty() {
        return Err(AppError::BadRequest("vault: label é obrigatório".into()));
    }
    let domains = validate_domains(&kind, &input.allowed_domains)?;
    let expires_at = validate_expiry(input.expires_at.as_deref())?;
    if let Some(secret) = input.secret.as_deref() {
        validate_secret(&kind, secret)?;
        let sealed = seal(secret)?;
        sqlx::query(
            "UPDATE vault_entries SET label = ?, username = ?, allowed_domains_json = ?,
                secret_ciphertext = ?, secret_nonce = ?, requires_hitl = ?, expires_at = ?,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?",
        )
        .bind(label)
        .bind(input.username.as_deref())
        .bind(domains_json(&domains))
        .bind(&sealed.ciphertext)
        .bind(sealed.nonce.to_vec())
        .bind(input.requires_hitl)
        .bind(expires_at.as_deref())
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE vault_entries SET label = ?, username = ?, allowed_domains_json = ?,
                requires_hitl = ?, expires_at = ?,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?",
        )
        .bind(label)
        .bind(input.username.as_deref())
        .bind(domains_json(&domains))
        .bind(input.requires_hitl)
        .bind(expires_at.as_deref())
        .bind(id)
        .execute(pool)
        .await?;
    }
    get_metadata(pool, id).await
}

pub async fn delete_entry(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let affected = sqlx::query("DELETE FROM vault_entries WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound(format!("vault {id}")));
    }
    Ok(())
}

#[cfg(test)]
#[path = "vault_tests.rs"]
mod tests;
