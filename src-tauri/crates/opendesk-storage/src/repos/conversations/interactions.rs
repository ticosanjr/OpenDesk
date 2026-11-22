use super::task;
use crate::error::{AppError, AppResult};
use opendesk_domain::{HitlKind, HitlStatus};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskInteraction {
    pub id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub question: String,
    pub context_markdown: Option<String>,
    pub options_json: String,
    pub kind: String,
    pub multiple: bool,
    pub allow_custom: bool,
    pub status: String,
    pub answer_json: Option<String>,
    pub expires_at: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateInteractionInput {
    pub workspace_id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub question: String,
    pub context_markdown: Option<String>,
    pub options: Vec<String>,
    pub kind: String,
    pub multiple: bool,
    pub allow_custom: bool,
    pub expires_at: String,
}

const SELECT: &str = "SELECT m.id, m.workspace_id, COALESCE(m.task_id, '') AS task_id, m.run_id,
    m.body AS question,
    json_extract(m.payload_json, '$.context_markdown') AS context_markdown,
    COALESCE(json_extract(m.payload_json, '$.options'), '[]') AS options_json,
    COALESCE(json_extract(m.payload_json, '$.hitl_kind'), 'TEXT') AS kind,
    CAST(COALESCE(json_extract(m.payload_json, '$.multiple'), 0) AS INTEGER) AS multiple,
    CAST(COALESCE(json_extract(m.payload_json, '$.allow_custom'), 1) AS INTEGER) AS allow_custom,
    m.status, m.choice_json AS answer_json,
    COALESCE(m.expires_at, '') AS expires_at, m.created_at, m.resolved_at
  FROM messages m";

pub async fn create(
    pool: &SqlitePool,
    input: CreateInteractionInput,
) -> AppResult<TaskInteraction> {
    let question = input.question.trim();
    if question.is_empty() {
        return Err(AppError::BadRequest(
            "interaction question is required".into(),
        ));
    }
    if input.expires_at.trim().is_empty() {
        return Err(AppError::BadRequest(
            "interaction expires_at is required".into(),
        ));
    }
    let conversation = task::ensure(pool, &input.task_id).await?;
    if conversation.workspace_id != input.workspace_id {
        return Err(AppError::BadRequest(format!(
            "tarefa {} não pertence ao workspace {}",
            input.task_id, input.workspace_id
        )));
    }
    let options = serde_json::to_string(&input.options)
        .map_err(|error| AppError::Internal(format!("options inválidas: {error}")))?;
    let kind = HitlKind::parse(&input.kind)
        .map(|kind| kind.as_str().to_string())
        .unwrap_or_else(|| {
            let value = input.kind.trim().to_uppercase();
            if value.is_empty() {
                "TEXT".into()
            } else {
                value
            }
        });
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO messages (
            id, conversation_id, workspace_id, task_id, run_id, role, kind, body,
            payload_json, status, expires_at
         ) VALUES (?, ?, ?, ?, ?, 'assistant', 'hitl_request', ?,
            json_object(
                'context_markdown', ?, 'options', json(?), 'multiple', ?,
                'allow_custom', ?, 'hitl_kind', ?
            ), 'PENDING', ?)",
    )
    .bind(&id)
    .bind(&conversation.id)
    .bind(&conversation.workspace_id)
    .bind(&input.task_id)
    .bind(input.run_id.as_deref())
    .bind(question)
    .bind(input.context_markdown.as_deref())
    .bind(&options)
    .bind(input.multiple)
    .bind(input.allow_custom)
    .bind(&kind)
    .bind(input.expires_at.trim())
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE conversations SET last_message_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&conversation.id)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<TaskInteraction> {
    let query = format!("{SELECT} WHERE m.id = ? AND m.kind = 'hitl_request'");
    sqlx::query_as::<_, TaskInteraction>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("interaction {id}")))
}

pub async fn pending_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<TaskInteraction>> {
    let query = format!(
        "{SELECT} WHERE m.task_id = ? AND m.kind = 'hitl_request' AND m.status = 'PENDING'
         ORDER BY m.created_at ASC, m.id ASC"
    );
    Ok(sqlx::query_as::<_, TaskInteraction>(&query)
        .bind(task_id)
        .fetch_all(pool)
        .await?)
}

pub async fn pending_for_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<TaskInteraction>> {
    let query = format!(
        "{SELECT} WHERE m.workspace_id = ? AND m.kind = 'hitl_request' AND m.status = 'PENDING'
         ORDER BY m.created_at ASC, m.id ASC"
    );
    Ok(sqlx::query_as::<_, TaskInteraction>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

/// Finds a pending interaction by the short code sent to remote chats.
pub async fn find_pending_by_prefix(
    pool: &SqlitePool,
    prefix: &str,
) -> AppResult<Option<TaskInteraction>> {
    let prefix = prefix.trim();
    if prefix.len() < 4 {
        return Ok(None);
    }
    let query = format!(
        "{SELECT} WHERE m.kind = 'hitl_request' AND m.status = 'PENDING' AND m.id LIKE ?
         ORDER BY m.created_at ASC, m.id ASC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, TaskInteraction>(&query)
        .bind(format!("{prefix}%"))
        .fetch_optional(pool)
        .await?)
}

pub async fn resolve(
    pool: &SqlitePool,
    id: &str,
    answer: &serde_json::Value,
    status: &str,
) -> AppResult<TaskInteraction> {
    let status = HitlStatus::parse(status)?;
    if !matches!(
        status,
        HitlStatus::Resolved | HitlStatus::Expired | HitlStatus::Cancelled
    ) {
        return Err(AppError::BadRequest(format!(
            "status de resolução inválido '{}'",
            status.as_str()
        )));
    }
    let answer_json = serde_json::to_string(answer)
        .map_err(|error| AppError::Internal(format!("invalid interaction answer: {error}")))?;
    let result = sqlx::query(
        "UPDATE messages SET status = ?, choice_json = ?,
            resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND kind = 'hitl_request' AND status = 'PENDING'",
    )
    .bind(status.as_str())
    .bind(&answer_json)
    .bind(id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "esta interação já foi respondida ou expirou".into(),
        ));
    }
    get(pool, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::pool::init(&temp.path().join("interactions.db"))
            .await
            .expect("db");
        sqlx::query(
            "INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', '/tmp/ws-1')",
        )
        .execute(&pool)
        .await
        .expect("workspace");
        sqlx::query(
            "INSERT INTO tasks (id, workspace_id, title, description)
             VALUES ('task-1', 'ws-1', 'Tarefa', 'Descrição')",
        )
        .execute(&pool)
        .await
        .expect("task");
        (temp, pool)
    }

    fn input() -> CreateInteractionInput {
        CreateInteractionInput {
            workspace_id: "ws-1".into(),
            task_id: "task-1".into(),
            run_id: None,
            question: "Aprovar o plano?".into(),
            context_markdown: Some("contexto".into()),
            options: vec!["Sim".into(), "Não".into()],
            kind: "SINGLE_CHOICE".into(),
            multiple: false,
            allow_custom: true,
            expires_at: "2999-01-01T00:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn creates_finds_and_resolves_an_interaction() {
        let (_temp, pool) = setup().await;
        let interaction = create(&pool, input()).await.expect("interaction");
        assert_eq!(interaction.status, "PENDING");
        assert_eq!(interaction.kind, "SINGLE_CHOICE");
        assert_eq!(interaction.options_json, "[\"Sim\",\"Não\"]");
        assert!(!interaction.multiple);
        assert!(interaction.allow_custom);

        let pending = pending_for_task(&pool, "task-1").await.expect("pending");
        assert_eq!(pending.len(), 1);
        let prefix = &interaction.id[..8];
        assert_eq!(
            find_pending_by_prefix(&pool, prefix)
                .await
                .expect("find")
                .map(|item| item.id),
            Some(interaction.id.clone())
        );
        assert!(
            find_pending_by_prefix(&pool, "abc")
                .await
                .expect("short")
                .is_none()
        );

        let resolved = resolve(
            &pool,
            &interaction.id,
            &serde_json::json!(["Sim"]),
            "RESOLVED",
        )
        .await
        .expect("resolve");
        assert_eq!(resolved.status, "RESOLVED");
        assert_eq!(resolved.answer_json.as_deref(), Some("[\"Sim\"]"));
        assert!(resolved.resolved_at.is_some());
        assert!(
            resolve(
                &pool,
                &interaction.id,
                &serde_json::json!(["Não"]),
                "RESOLVED"
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_wrong_workspace_and_invalid_status() {
        let (_temp, pool) = setup().await;
        let mut wrong = input();
        wrong.workspace_id = "ws-2".into();
        assert!(create(&pool, wrong).await.is_err());
        let interaction = create(&pool, input()).await.expect("interaction");
        assert!(
            resolve(&pool, &interaction.id, &serde_json::json!({}), "PENDING")
                .await
                .is_err()
        );
    }
}
