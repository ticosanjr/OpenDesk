use crate::error::{AppError, AppResult};
use opendesk_domain::MessageRole;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatThread {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub agent: String,
    pub model: Option<String>,
    pub opencode_session_id: Option<String>,
    pub source: String,
    pub external_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_message_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMessage {
    pub id: String,
    pub thread_id: String,
    pub role: String,
    pub content: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateThreadInput {
    pub workspace_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub agent: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
}

const THREAD_COLUMNS: &str = "id, workspace_id, title, COALESCE(agent, '') AS agent, model, \
    opencode_session_id, source, external_id, created_at, updated_at, last_message_at";
const MESSAGE_COLUMNS: &str = "id, conversation_id AS thread_id, role, body AS content, \
    metadata_json, created_at";

pub async fn list_threads(
    pool: &SqlitePool,
    workspace_id: &str,
    source: Option<&str>,
) -> AppResult<Vec<ChatThread>> {
    let query = format!(
        "SELECT {THREAD_COLUMNS} FROM conversations
         WHERE workspace_id = ? AND channel = 'chat' AND (? IS NULL OR source = ?)
         ORDER BY COALESCE(last_message_at, updated_at) DESC LIMIT 100"
    );
    Ok(sqlx::query_as::<_, ChatThread>(&query)
        .bind(workspace_id)
        .bind(source)
        .bind(source)
        .fetch_all(pool)
        .await?)
}

pub async fn get_thread(pool: &SqlitePool, id: &str) -> AppResult<ChatThread> {
    let query =
        format!("SELECT {THREAD_COLUMNS} FROM conversations WHERE id = ? AND channel = 'chat'");
    sqlx::query_as::<_, ChatThread>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("chat thread {id}")))
}

pub async fn find_external(
    pool: &SqlitePool,
    workspace_id: &str,
    source: &str,
    external_id: &str,
) -> AppResult<Option<ChatThread>> {
    let query = format!(
        "SELECT {THREAD_COLUMNS} FROM conversations
         WHERE workspace_id = ? AND channel = 'chat' AND source = ? AND external_id = ?"
    );
    Ok(sqlx::query_as::<_, ChatThread>(&query)
        .bind(workspace_id)
        .bind(source)
        .bind(external_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn find_external_any_workspace(
    pool: &SqlitePool,
    source: &str,
    external_id: &str,
) -> AppResult<Option<ChatThread>> {
    let query = format!(
        "SELECT {THREAD_COLUMNS} FROM conversations
         WHERE channel = 'chat' AND source = ? AND external_id = ?
         ORDER BY COALESCE(last_message_at, updated_at) DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, ChatThread>(&query)
        .bind(source)
        .bind(external_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn create_thread(pool: &SqlitePool, input: CreateThreadInput) -> AppResult<ChatThread> {
    let agent = input.agent.trim();
    if agent.is_empty() {
        return Err(AppError::BadRequest("chat agent is required".into()));
    }
    let title = input
        .title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Nova conversa".to_string());
    let source = input.source.as_deref().unwrap_or("desktop");
    let external_id = input
        .external_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let thread_key = external_id.map(|value| format!("{source}:{value}"));
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO conversations (
            id, workspace_id, title, agent, model, channel, thread_key, source, external_id
         ) VALUES (?, ?, ?, ?, ?, 'chat', ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(&title)
    .bind(agent)
    .bind(
        input
            .model
            .as_deref()
            .filter(|value| !value.trim().is_empty()),
    )
    .bind(thread_key)
    .bind(source)
    .bind(external_id)
    .execute(pool)
    .await?;
    get_thread(pool, &id).await
}

pub async fn update_session(pool: &SqlitePool, id: &str, session_id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE conversations SET opencode_session_id = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND channel = 'chat'",
    )
    .bind(session_id)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_title(pool: &SqlitePool, id: &str, title: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE conversations SET title = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND channel = 'chat'",
    )
    .bind(title)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_message(
    pool: &SqlitePool,
    thread_id: &str,
    role: &str,
    content: &str,
    metadata: Option<&str>,
) -> AppResult<ChatMessage> {
    let role = MessageRole::parse(role)?;
    let id = Uuid::now_v7().to_string();
    let inserted = sqlx::query(
        "INSERT INTO messages
            (id, conversation_id, workspace_id, role, kind, body, metadata_json, status)
         SELECT ?, c.id, c.workspace_id, ?, 'chat', ?, ?, 'COMPLETED'
         FROM conversations c WHERE c.id = ? AND c.channel = 'chat'",
    )
    .bind(&id)
    .bind(role.as_str())
    .bind(content)
    .bind(metadata)
    .bind(thread_id)
    .execute(pool)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("chat thread {thread_id}")));
    }
    sqlx::query(
        "UPDATE conversations SET last_message_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(thread_id)
    .execute(pool)
    .await?;
    get_message(pool, &id).await
}

pub async fn list_messages(pool: &SqlitePool, thread_id: &str) -> AppResult<Vec<ChatMessage>> {
    let query = format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages
         WHERE conversation_id = ? AND kind = 'chat'
         ORDER BY created_at ASC, id ASC LIMIT 500"
    );
    Ok(sqlx::query_as::<_, ChatMessage>(&query)
        .bind(thread_id)
        .fetch_all(pool)
        .await?)
}

/// Cursor pagination by message id (UUIDv7 is time ordered).
pub async fn list_messages_page(
    pool: &SqlitePool,
    thread_id: &str,
    after: Option<&str>,
    limit: u32,
) -> AppResult<Vec<ChatMessage>> {
    let query = format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages
         WHERE conversation_id = ? AND kind = 'chat' AND (? IS NULL OR id > ?)
         ORDER BY id ASC LIMIT ?"
    );
    Ok(sqlx::query_as::<_, ChatMessage>(&query)
        .bind(thread_id)
        .bind(after)
        .bind(after)
        .bind(i64::from(limit.clamp(1, 500)))
        .fetch_all(pool)
        .await?)
}

pub async fn get_message(pool: &SqlitePool, id: &str) -> AppResult<ChatMessage> {
    let query = format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE id = ? AND kind = 'chat'");
    sqlx::query_as::<_, ChatMessage>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("chat message {id}")))
}

pub async fn delete_thread(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM conversations WHERE id = ? AND channel = 'chat'")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::pool::init(&temp.path().join("chat.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    fn input() -> CreateThreadInput {
        CreateThreadInput {
            workspace_id: "ws-1".into(),
            title: Some("Conversa".into()),
            agent: "build".into(),
            model: Some("opencode-go/glm-5.3".into()),
            source: Some("desktop".into()),
            external_id: None,
        }
    }

    #[tokio::test]
    async fn thread_and_message_lifecycle() {
        let (_temp, pool) = setup().await;
        let thread = create_thread(&pool, input()).await.expect("thread");
        assert_eq!(thread.agent, "build");
        assert_eq!(
            list_threads(&pool, "ws-1", None).await.expect("list").len(),
            1
        );

        let user = add_message(&pool, &thread.id, "user", "Olá", None)
            .await
            .expect("user");
        let assistant = add_message(&pool, &thread.id, "assistant", "Oi!", None)
            .await
            .expect("assistant");
        let messages = list_messages(&pool, &thread.id).await.expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "Olá");
        assert_eq!(messages[1].id, assistant.id);

        let page = list_messages_page(&pool, &thread.id, Some(&user.id), 10)
            .await
            .expect("page");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, assistant.id);

        update_session(&pool, &thread.id, "ses-1")
            .await
            .expect("session");
        update_title(&pool, &thread.id, "Novo título")
            .await
            .expect("title");
        let updated = get_thread(&pool, &thread.id).await.expect("get");
        assert_eq!(updated.opencode_session_id.as_deref(), Some("ses-1"));
        assert_eq!(updated.title, "Novo título");

        delete_thread(&pool, &thread.id).await.expect("delete");
        assert!(get_thread(&pool, &thread.id).await.is_err());
        assert!(matches!(
            get_message(&pool, &user.id).await,
            Err(AppError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn external_threads_are_unique_and_findable() {
        let (_temp, pool) = setup().await;
        let mut value = input();
        value.source = Some("signal".into());
        value.external_id = Some("+1555".into());
        let thread = create_thread(&pool, value.clone()).await.expect("thread");
        assert_eq!(
            find_external(&pool, "ws-1", "signal", "+1555")
                .await
                .expect("find")
                .map(|item| item.id),
            Some(thread.id)
        );
        assert!(create_thread(&pool, value).await.is_err());

        sqlx::query(
            "INSERT INTO workspaces (id, name, root_path) VALUES ('ws-2', 'WS 2', '/tmp/ws-2')",
        )
        .execute(&pool)
        .await
        .expect("workspace");
        let mut other = input();
        other.workspace_id = "ws-2".into();
        other.source = Some("telegram".into());
        other.external_id = Some("42".into());
        let other_thread = create_thread(&pool, other).await.expect("other");
        assert_eq!(
            find_external_any_workspace(&pool, "telegram", "42")
                .await
                .expect("find any")
                .map(|item| item.id),
            Some(other_thread.id)
        );
    }

    #[tokio::test]
    async fn same_external_id_on_two_sources_is_two_threads() {
        let (_temp, pool) = setup().await;
        let mut first = input();
        first.source = Some("telegram".into());
        first.external_id = Some("42".into());
        let mut second = first.clone();
        second.source = Some("signal".into());
        assert!(create_thread(&pool, first).await.is_ok());
        assert!(create_thread(&pool, second).await.is_ok());
    }
}
