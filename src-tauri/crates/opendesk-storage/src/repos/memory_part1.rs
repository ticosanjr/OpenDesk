
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

pub const KINDS: &[&str] = &["semantic", "episodic", "procedural", "preference"];
pub const TRUST_LEVELS: &[&str] = &["observed", "verified", "inferred"];

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MemoryNamespace {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MemoryNote {
    pub id: String,
    pub workspace_id: String,
    pub namespace: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub tags_json: String,
    pub confidence: f64,
    pub trust_level: String,
    pub expires_at: Option<String>,
    pub source: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[sqlx(default)]
    pub access_count: i64,
    #[sqlx(default)]
    pub last_accessed_at: Option<String>,
    #[sqlx(default)]
    pub archived_at: Option<String>,
}

impl MemoryNote {
    pub fn tags(&self) -> Vec<String> {
        serde_json::from_str(&self.tags_json).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateNoteInput {
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_trust")]
    pub trust_level: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateNoteInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
    pub expires_at: Option<String>,
    pub clear_expiry: bool,
}

fn default_namespace() -> String {
    "core".into()
}
fn default_kind() -> String {
    "semantic".into()
}
fn default_confidence() -> f64 {
    0.5
}
fn default_trust() -> String {
    "observed".into()
}

const NOTE_COLUMNS: &str = "n.id, n.workspace_id, n.namespace, n.kind, n.title, n.body, n.tags_json, \
    n.confidence, n.trust_level, n.expires_at, n.source, n.created_at, n.updated_at, \
    n.access_count, n.last_accessed_at, n.archived_at";
const NS_COLUMNS: &str = "id, workspace_id, name, description, created_at, updated_at";

pub async fn list_namespaces(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<MemoryNamespace>> {
    let query =
        format!("SELECT {NS_COLUMNS} FROM memory_namespaces WHERE workspace_id = ? ORDER BY name");
    Ok(sqlx::query_as::<_, MemoryNamespace>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn ensure_namespace(
    pool: &SqlitePool,
    workspace_id: &str,
    name: &str,
    description: Option<&str>,
) -> AppResult<MemoryNamespace> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("namespace vazio".into()));
    }
    if let Some(existing) = sqlx::query_as::<_, MemoryNamespace>(&format!(
        "SELECT {NS_COLUMNS} FROM memory_namespaces WHERE workspace_id = ? AND name = ?"
    ))
    .bind(workspace_id)
    .bind(name)
    .fetch_optional(pool)
    .await?
    {
        return Ok(existing);
    }
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO memory_namespaces (id, workspace_id, name, description) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(name)
    .bind(description)
    .execute(pool)
    .await?;
    Ok(sqlx::query_as::<_, MemoryNamespace>(&format!(
        "SELECT {NS_COLUMNS} FROM memory_namespaces WHERE id = ?"
    ))
    .bind(&id)
    .fetch_one(pool)
    .await?)
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MemoryAcl {
    pub namespace_id: String,
    pub agent: String,
    pub can_read: bool,
    pub can_write: bool,
}

pub async fn list_acl(pool: &SqlitePool, namespace_id: &str) -> AppResult<Vec<MemoryAcl>> {
    Ok(sqlx::query_as::<_, MemoryAcl>(
        "SELECT namespace_id, agent, can_read, can_write FROM memory_acl
         WHERE namespace_id = ? ORDER BY agent",
    )
    .bind(namespace_id)
    .fetch_all(pool)
    .await?)
}

pub async fn set_acl(
    pool: &SqlitePool,
    namespace_id: &str,
    agent: &str,
    can_read: bool,
    can_write: bool,
) -> AppResult<()> {
    let agent = agent.trim();
    if agent.is_empty() {
        return Err(AppError::BadRequest("agente vazio".into()));
    }
    sqlx::query(
        "INSERT INTO memory_acl (namespace_id, agent, can_read, can_write)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(namespace_id, agent) DO UPDATE SET
             can_read = excluded.can_read, can_write = excluded.can_write",
    )
    .bind(namespace_id)
    .bind(agent)
    .bind(can_read)
    .bind(can_write)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_acl(pool: &SqlitePool, namespace_id: &str, agent: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM memory_acl WHERE namespace_id = ? AND agent = ?")
        .bind(namespace_id)
        .bind(agent)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// A namespace with no ACL rows is open to every agent. Once any row exists,
/// only agents with `can_read = 1` may read it.
fn readable_clause(alias: &str) -> String {
    format!(
        "(NOT EXISTS (SELECT 1 FROM memory_acl a WHERE a.namespace_id = {alias}.id)
          OR EXISTS (SELECT 1 FROM memory_acl a WHERE a.namespace_id = {alias}.id
                     AND a.agent = ? AND a.can_read = 1))"
    )
}

pub async fn can_write(
    pool: &SqlitePool,
    workspace_id: &str,
    namespace: &str,
    agent: &str,
) -> AppResult<bool> {
    let rows: Option<(String,)> = sqlx::query_as(
        "SELECT n.id FROM memory_namespaces n WHERE n.workspace_id = ? AND n.name = ?",
    )
    .bind(workspace_id)
    .bind(namespace)
    .fetch_optional(pool)
    .await?;
    let Some((namespace_id,)) = rows else {
        return Ok(true);
    };
    let acl_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM memory_acl WHERE namespace_id = ?")
            .bind(&namespace_id)
            .fetch_one(pool)
            .await?;
    if acl_count == 0 {
        return Ok(true);
    }
    let allowed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memory_acl WHERE namespace_id = ? AND agent = ? AND can_write = 1",
    )
    .bind(&namespace_id)
    .bind(agent)
    .fetch_one(pool)
    .await?;
    Ok(allowed > 0)
}

pub async fn create_note(
    pool: &SqlitePool,
    workspace_id: &str,
    input: CreateNoteInput,
) -> AppResult<MemoryNote> {
    let kind = input.kind.trim().to_lowercase();
    if !KINDS.contains(&kind.as_str()) {
        return Err(AppError::BadRequest(format!(
            "kind de memória inválido '{kind}'"
        )));
    }
    let trust = input.trust_level.trim().to_lowercase();
    if !TRUST_LEVELS.contains(&trust.as_str()) {
        return Err(AppError::BadRequest(format!(
            "trust_level de memória inválido '{trust}'"
        )));
    }
    if input.title.trim().is_empty() || input.body.trim().is_empty() {
        return Err(AppError::BadRequest(
            "memória precisa de título e corpo".into(),
        ));
    }
    ensure_namespace(pool, workspace_id, &input.namespace, None).await?;
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO memory_notes (
            id, workspace_id, namespace, kind, title, body, tags_json, confidence,
            trust_level, expires_at, source
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(input.namespace.trim())
    .bind(&kind)
    .bind(input.title.trim())
    .bind(input.body.trim())
    .bind(serde_json::to_string(&input.tags).unwrap_or_else(|_| "[]".into()))
    .bind(input.confidence.clamp(0.0, 1.0))
    .bind(&trust)
    .bind(input.expires_at.as_deref())
    .bind(input.source.as_deref())
    .execute(pool)
    .await?;
    get_note(pool, &id).await
}
