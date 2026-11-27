use super::model::*;
use crate::error::{AppError, AppResult};
use opendesk_domain::MemoryUsageScope;
use sqlx::SqlitePool;

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
    let usage_scope = match input.usage_scope.as_deref() {
        Some(value) => parse_usage_scope(value)?,
        None => parse_usage_scope(&current.usage_scope).unwrap_or(MemoryUsageScope::Both),
    };
    sqlx::query(
        "UPDATE memory_notes SET title = ?, body = ?, tags_json = ?, confidence = ?,
                trust_level = ?, expires_at = ?, usage_scope = ?,
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
    .bind(usage_scope.as_str())
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
    list_notes_for(pool, workspace_id, namespace, agent, limit, None).await
}

pub async fn list_notes_for(
    pool: &SqlitePool,
    workspace_id: &str,
    namespace: Option<&str>,
    agent: Option<&str>,
    limit: i64,
    usage_scope: Option<MemoryUsageScope>,
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
    query.push_str(&scope_clause("n", usage_scope));
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
    search_for(pool, workspace_id, term, agent, limit, None).await
}

pub async fn search_for(
    pool: &SqlitePool,
    workspace_id: &str,
    term: &str,
    agent: Option<&str>,
    limit: i64,
    usage_scope: Option<MemoryUsageScope>,
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
    query.push_str(&scope_clause("n", usage_scope));
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
    render_context_for(pool, workspace_id, agent, query, limit, None).await
}

pub async fn render_context_for(
    pool: &SqlitePool,
    workspace_id: &str,
    agent: &str,
    query: &str,
    limit: i64,
    usage_scope: Option<MemoryUsageScope>,
) -> AppResult<String> {
    let mut notes = if query.trim().is_empty() {
        Vec::new()
    } else {
        search_for(pool, workspace_id, query, Some(agent), limit, usage_scope).await?
    };
    if notes.len() < limit as usize {
        let seen = notes.iter().map(|note| note.id.clone()).collect::<Vec<_>>();
        for note in
            list_notes_for(pool, workspace_id, None, Some(agent), limit, usage_scope).await?
        {
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
