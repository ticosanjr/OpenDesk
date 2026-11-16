//! Tickets: the durable inbox for anything that arrives from a channel before
//! (or instead of) becoming a task.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

pub const TICKET_STATUSES: &[&str] = &["OPEN", "TRIAGED", "IN_PROGRESS", "RESOLVED", "DISMISSED"];

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Ticket {
    pub id: String,
    pub workspace_id: String,
    pub source: String,
    pub source_ref: Option<String>,
    pub title: String,
    pub body: String,
    pub priority: i64,
    pub status: String,
    pub assigned_agent: Option<String>,
    pub task_id: Option<String>,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateTicketInput {
    pub workspace_id: String,
    pub source: String,
    pub source_ref: Option<String>,
    pub title: String,
    pub body: String,
    pub priority: Option<i64>,
    pub assigned_agent: Option<String>,
    pub metadata: Value,
}

const COLUMNS: &str = "id, workspace_id, source, source_ref, title, body, priority, status, \
    assigned_agent, task_id, metadata_json, created_at, updated_at";

pub async fn list(
    pool: &SqlitePool,
    workspace_id: &str,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<Ticket>> {
    let query = format!(
        "SELECT {COLUMNS} FROM tickets
         WHERE workspace_id = ? AND (? IS NULL OR status = ?)
         ORDER BY CASE priority WHEN 1 THEN 0 ELSE priority END, created_at DESC
         LIMIT ?"
    );
    Ok(sqlx::query_as::<_, Ticket>(&query)
        .bind(workspace_id)
        .bind(status)
        .bind(status)
        .bind(limit.clamp(1, 500))
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Ticket> {
    let query = format!("SELECT {COLUMNS} FROM tickets WHERE id = ?");
    sqlx::query_as::<_, Ticket>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("ticket {id}")))
}

/// Idempotent by `(workspace, source, source_ref)` when a ref is provided.
pub async fn create(pool: &SqlitePool, input: CreateTicketInput) -> AppResult<(Ticket, bool)> {
    if let Some(reference) = input.source_ref.as_deref()
        && let Some(existing) =
            find_by_ref(pool, &input.workspace_id, &input.source, reference).await?
    {
        return Ok((existing, false));
    }
    let id = Uuid::now_v7().to_string();
    let metadata = input.metadata.to_string();
    let result = sqlx::query(
        "INSERT INTO tickets (
            id, workspace_id, source, source_ref, title, body, priority, status,
            assigned_agent, metadata_json
         ) VALUES (?, ?, ?, ?, ?, ?, ?, 'OPEN', ?, ?)
         ON CONFLICT DO NOTHING",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(input.source.trim())
    .bind(input.source_ref.as_deref())
    .bind(input.title.trim())
    .bind(&input.body)
    .bind(input.priority.unwrap_or(2).clamp(1, 4))
    .bind(input.assigned_agent.as_deref())
    .bind(&metadata)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0
        && let Some(reference) = input.source_ref.as_deref()
        && let Some(existing) =
            find_by_ref(pool, &input.workspace_id, &input.source, reference).await?
    {
        return Ok((existing, false));
    }
    Ok((get(pool, &id).await?, true))
}

async fn find_by_ref(
    pool: &SqlitePool,
    workspace_id: &str,
    source: &str,
    source_ref: &str,
) -> AppResult<Option<Ticket>> {
    let query = format!(
        "SELECT {COLUMNS} FROM tickets
         WHERE workspace_id = ? AND source = ? AND source_ref = ? LIMIT 1"
    );
    Ok(sqlx::query_as::<_, Ticket>(&query)
        .bind(workspace_id)
        .bind(source)
        .bind(source_ref)
        .fetch_optional(pool)
        .await?)
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTicketInput {
    pub status: Option<String>,
    pub priority: Option<i64>,
    pub assigned_agent: Option<String>,
    pub task_id: Option<String>,
    #[serde(default)]
    pub clear_assignee: bool,
}

pub async fn update(pool: &SqlitePool, id: &str, input: UpdateTicketInput) -> AppResult<Ticket> {
    let current = get(pool, id).await?;
    let status = match input.status {
        Some(value) => {
            let value = value.trim().to_uppercase();
            if !TICKET_STATUSES.contains(&value.as_str()) {
                return Err(AppError::BadRequest(format!(
                    "status de ticket inválido '{value}'"
                )));
            }
            value
        }
        None => current.status,
    };
    sqlx::query(
        "UPDATE tickets SET status = ?, priority = ?, assigned_agent = ?, task_id = ?,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&status)
    .bind(input.priority.unwrap_or(current.priority).clamp(1, 4))
    .bind(if input.clear_assignee {
        None
    } else {
        input.assigned_agent.or(current.assigned_agent)
    })
    .bind(input.task_id.or(current.task_id))
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deduplicates_by_source_reference() {
        let ctx = crate::test_support::context().await;
        let input = || CreateTicketInput {
            workspace_id: ctx.workspace_id.clone(),
            source: "telegram".into(),
            source_ref: Some("msg-1".into()),
            title: "Oi".into(),
            body: "corpo".into(),
            priority: Some(3),
            assigned_agent: Some("build".into()),
            metadata: serde_json::json!({ "chat": 1 }),
        };
        let (first, created) = create(&ctx.state.db, input()).await.expect("ticket");
        assert!(created);
        let (second, created) = create(&ctx.state.db, input()).await.expect("ticket");
        assert!(!created);
        assert_eq!(first.id, second.id);
        assert_eq!(
            list(&ctx.state.db, &ctx.workspace_id, Some("OPEN"), 10)
                .await
                .expect("list")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn updates_status_and_rejects_unknown() {
        let ctx = crate::test_support::context().await;
        let (ticket, _) = create(
            &ctx.state.db,
            CreateTicketInput {
                workspace_id: ctx.workspace_id.clone(),
                source: "web".into(),
                source_ref: None,
                title: "Formulário".into(),
                body: "conteúdo".into(),
                priority: None,
                assigned_agent: None,
                metadata: Value::Null,
            },
        )
        .await
        .expect("ticket");
        let updated = update(
            &ctx.state.db,
            &ticket.id,
            UpdateTicketInput {
                status: Some("triaged".into()),
                priority: None,
                assigned_agent: Some("plan".into()),
                task_id: None,
                clear_assignee: false,
            },
        )
        .await
        .expect("update");
        assert_eq!(updated.status, "TRIAGED");
        assert_eq!(updated.assigned_agent.as_deref(), Some("plan"));

        assert!(
            update(
                &ctx.state.db,
                &ticket.id,
                UpdateTicketInput {
                    status: Some("nope".into()),
                    priority: None,
                    assigned_agent: None,
                    task_id: None,
                    clear_assignee: false,
                }
            )
            .await
            .is_err()
        );
    }
}
