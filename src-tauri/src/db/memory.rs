//! Structured, agent-scoped memory.
//!
//! `MEMORY.md` remains the human-editable workspace file. This module adds
//! queryable notes with namespaces, trust levels, expiry and per-agent ACL, so
//! an agent only receives context it is allowed to read.

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

pub async fn get_note(pool: &SqlitePool, id: &str) -> AppResult<MemoryNote> {
    let query = format!("SELECT {NOTE_COLUMNS} FROM memory_notes n WHERE n.id = ?");
    sqlx::query_as::<_, MemoryNote>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("memory note {id}")))
}

pub async fn update_note(
    pool: &SqlitePool,
    id: &str,
    input: UpdateNoteInput,
) -> AppResult<MemoryNote> {
    let current = get_note(pool, id).await?;
    let trust = match input.trust_level {
        Some(value) => {
            let value = value.trim().to_lowercase();
            if !TRUST_LEVELS.contains(&value.as_str()) {
                return Err(AppError::BadRequest(format!(
                    "trust_level inválido '{value}'"
                )));
            }
            value
        }
        None => current.trust_level,
    };
    let tags_json = match input.tags {
        Some(tags) => serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into()),
        None => current.tags_json,
    };
    sqlx::query(
        "UPDATE memory_notes SET title = ?, body = ?, tags_json = ?, confidence = ?,
                trust_level = ?, expires_at = ?,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(input.title.unwrap_or(current.title))
    .bind(input.body.unwrap_or(current.body))
    .bind(&tags_json)
    .bind(
        input
            .confidence
            .unwrap_or(current.confidence)
            .clamp(0.0, 1.0),
    )
    .bind(&trust)
    .bind(if input.clear_expiry {
        None
    } else {
        input.expires_at.or(current.expires_at)
    })
    .bind(id)
    .execute(pool)
    .await?;
    get_note(pool, id).await
}

pub async fn delete_note(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM memory_notes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_notes(
    pool: &SqlitePool,
    workspace_id: &str,
    namespace: Option<&str>,
    agent: Option<&str>,
    limit: i64,
) -> AppResult<Vec<MemoryNote>> {
    let mut query = format!(
        "SELECT {NOTE_COLUMNS}
         FROM memory_notes n
         JOIN memory_namespaces ns ON ns.workspace_id = n.workspace_id AND ns.name = n.namespace
         WHERE n.workspace_id = ?
           AND (? IS NULL OR n.namespace = ?)
           AND n.archived_at IS NULL
           AND (n.expires_at IS NULL OR n.expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now'))"
    );
    if agent.is_some() {
        query.push_str(" AND ");
        query.push_str(&readable_clause("ns"));
    }
    query.push_str(" ORDER BY n.confidence DESC, n.updated_at DESC LIMIT ?");
    let mut request = sqlx::query_as::<_, MemoryNote>(&query)
        .bind(workspace_id)
        .bind(namespace)
        .bind(namespace);
    if let Some(agent) = agent {
        request = request.bind(agent);
    }
    Ok(request.bind(limit.clamp(1, 500)).fetch_all(pool).await?)
}

/// Case-insensitive search over title/body/tags with ACL filtering.
pub async fn search(
    pool: &SqlitePool,
    workspace_id: &str,
    term: &str,
    agent: Option<&str>,
    limit: i64,
) -> AppResult<Vec<MemoryNote>> {
    let pattern = format!("%{}%", term.trim().to_lowercase());
    let mut query = format!(
        "SELECT {NOTE_COLUMNS}
         FROM memory_notes n
         JOIN memory_namespaces ns ON ns.workspace_id = n.workspace_id AND ns.name = n.namespace
         WHERE n.workspace_id = ?
           AND (lower(n.title) LIKE ? OR lower(n.body) LIKE ? OR lower(n.tags_json) LIKE ?)
           AND (n.expires_at IS NULL OR n.expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now'))"
    );
    if agent.is_some() {
        query.push_str(" AND ");
        query.push_str(&readable_clause("ns"));
    }
    query.push_str(" ORDER BY n.confidence DESC, n.updated_at DESC LIMIT ?");
    let mut request = sqlx::query_as::<_, MemoryNote>(&query)
        .bind(workspace_id)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern);
    if let Some(agent) = agent {
        request = request.bind(agent);
    }
    Ok(request.bind(limit.clamp(1, 100)).fetch_all(pool).await?)
}

/// Renders the compact block injected into prompts.
pub async fn render_context(
    pool: &SqlitePool,
    workspace_id: &str,
    agent: &str,
    query: &str,
    limit: i64,
) -> AppResult<String> {
    let mut notes = if query.trim().is_empty() {
        Vec::new()
    } else {
        search(pool, workspace_id, query, Some(agent), limit).await?
    };
    if notes.len() < limit as usize {
        let seen = notes.iter().map(|note| note.id.clone()).collect::<Vec<_>>();
        for note in list_notes(pool, workspace_id, None, Some(agent), limit).await? {
            if !seen.contains(&note.id) {
                notes.push(note);
            }
            if notes.len() >= limit as usize {
                break;
            }
        }
    }
    if notes.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::new();
    for note in notes.into_iter().take(limit as usize) {
        let tags = note.tags();
        let tag_line = if tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", tags.join(", "))
        };
        out.push_str(&format!(
            "- ({}, {:.0}%) **{}**{tag_line}: {}\n",
            note.trust_level,
            note.confidence * 100.0,
            note.title,
            note.body.trim()
        ));
    }
    Ok(out.chars().take(8_000).collect())
}

pub async fn delete_expired(pool: &SqlitePool) -> AppResult<u64> {
    let result = sqlx::query(
        "UPDATE memory_notes SET archived_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE archived_at IS NULL AND expires_at IS NOT NULL
           AND expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Archives expired notes, decays unconfirmed confidence and collapses exact
/// duplicates (same workspace/namespace/title/body), keeping the strongest.
pub async fn consolidate(pool: &SqlitePool) -> AppResult<u64> {
    let mut changed = delete_expired(pool).await?;
    let decayed = sqlx::query(
        "UPDATE memory_notes
         SET confidence = MAX(0.0, confidence - 0.05),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE archived_at IS NULL AND trust_level != 'verified'
           AND updated_at < strftime('%Y-%m-%dT%H:%M:%fZ','now','-30 days')",
    )
    .execute(pool)
    .await?;
    changed += decayed.rows_affected();
    let archived_low = sqlx::query(
        "UPDATE memory_notes SET archived_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE archived_at IS NULL AND trust_level != 'verified' AND confidence < 0.1",
    )
    .execute(pool)
    .await?;
    changed += archived_low.rows_affected();

    let duplicates = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT workspace_id, namespace, lower(trim(title)), lower(trim(body))
         FROM memory_notes WHERE archived_at IS NULL
         GROUP BY workspace_id, namespace, lower(trim(title)), lower(trim(body))
         HAVING COUNT(*) > 1",
    )
    .fetch_all(pool)
    .await?;
    for (workspace_id, namespace, title, body) in duplicates {
        let rows = sqlx::query_as::<_, MemoryNote>(&format!(
            "SELECT {NOTE_COLUMNS} FROM memory_notes n
             WHERE n.workspace_id = ? AND n.namespace = ?
               AND lower(trim(n.title)) = ? AND lower(trim(n.body)) = ?
               AND n.archived_at IS NULL
             ORDER BY n.confidence DESC, n.updated_at DESC"
        ))
        .bind(&workspace_id)
        .bind(&namespace)
        .bind(&title)
        .bind(&body)
        .fetch_all(pool)
        .await?;
        for duplicate in rows.into_iter().skip(1) {
            sqlx::query(
                "UPDATE memory_notes SET archived_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE id = ?",
            )
            .bind(&duplicate.id)
            .execute(pool)
            .await?;
            changed += 1;
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn note(ctx: &crate::test_support::TestContext, namespace: &str) -> MemoryNote {
        create_note(
            &ctx.state.db,
            &ctx.workspace_id,
            CreateNoteInput {
                namespace: namespace.into(),
                kind: "semantic".into(),
                title: "Preferência".into(),
                body: "Relatórios em português".into(),
                tags: vec!["relatorio".into()],
                confidence: 0.9,
                trust_level: "verified".into(),
                expires_at: None,
                source: Some("test".into()),
            },
        )
        .await
        .expect("note")
    }

    #[tokio::test]
    async fn acl_restricts_reading_and_writing() {
        let ctx = crate::test_support::context().await;
        let created = note(&ctx, "private").await;
        assert!(
            can_write(&ctx.state.db, &ctx.workspace_id, "private", "build")
                .await
                .expect("write")
        );
        let namespace = ensure_namespace(&ctx.state.db, &ctx.workspace_id, "private", None)
            .await
            .expect("namespace");
        set_acl(&ctx.state.db, &namespace.id, "review", true, false)
            .await
            .expect("acl");
        assert!(
            !can_write(&ctx.state.db, &ctx.workspace_id, "private", "review")
                .await
                .expect("write")
        );
        assert!(
            list_notes(&ctx.state.db, &ctx.workspace_id, None, Some("build"), 10)
                .await
                .expect("list")
                .is_empty()
        );
        assert_eq!(
            list_notes(&ctx.state.db, &ctx.workspace_id, None, Some("review"), 10)
                .await
                .expect("list")
                .len(),
            1
        );
        assert_eq!(created.namespace, "private");
    }

    #[tokio::test]
    async fn consolidates_duplicates_and_archives_expired() {
        let ctx = crate::test_support::context().await;
        note(&ctx, "core").await;
        note(&ctx, "core").await;
        let changed = consolidate(&ctx.state.db).await.expect("consolidate");
        assert!(changed >= 1);
        assert_eq!(
            list_notes(&ctx.state.db, &ctx.workspace_id, None, None, 10)
                .await
                .expect("list")
                .len(),
            1
        );

        let old = create_note(
            &ctx.state.db,
            &ctx.workspace_id,
            CreateNoteInput {
                namespace: "core".into(),
                kind: "semantic".into(),
                title: "Antiga".into(),
                body: "sem confirmação".into(),
                tags: vec![],
                confidence: 0.2,
                trust_level: "observed".into(),
                expires_at: None,
                source: None,
            },
        )
        .await
        .expect("old");
        sqlx::query(
            "UPDATE memory_notes SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now','-60 days')
             WHERE id = ?",
        )
        .bind(&old.id)
        .execute(&ctx.state.db)
        .await
        .expect("age");
        consolidate(&ctx.state.db).await.expect("decay");
        let decayed = get_note(&ctx.state.db, &old.id).await.expect("get");
        assert!(decayed.confidence < 0.2);
        assert!(decayed.archived_at.is_none());
    }

    #[tokio::test]
    async fn search_and_expiry() {
        let ctx = crate::test_support::context().await;
        note(&ctx, "core").await;
        let found = search(&ctx.state.db, &ctx.workspace_id, "português", None, 10)
            .await
            .expect("search");
        assert_eq!(found.len(), 1);
        let context = render_context(&ctx.state.db, &ctx.workspace_id, "build", "relatório", 5)
            .await
            .expect("context");
        assert!(context.contains("Relatórios em português"));

        create_note(
            &ctx.state.db,
            &ctx.workspace_id,
            CreateNoteInput {
                namespace: "core".into(),
                kind: "episodic".into(),
                title: "Temporária".into(),
                body: "expira".into(),
                tags: vec![],
                confidence: 0.1,
                trust_level: "inferred".into(),
                expires_at: Some("2000-01-01T00:00:00.000Z".into()),
                source: None,
            },
        )
        .await
        .expect("expired note");
        assert_eq!(delete_expired(&ctx.state.db).await.expect("cleanup"), 1);
    }
}
