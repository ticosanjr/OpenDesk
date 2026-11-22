use crate::error::AppResult;
use sqlx::SqlitePool;

use crate::repos::conversations::threads as repo;
pub use crate::repos::conversations::threads::{ChatMessage, ChatThread, CreateThreadInput};

pub async fn list_threads(
    pool: &SqlitePool,
    workspace_id: &str,
    source: Option<&str>,
) -> AppResult<Vec<ChatThread>> {
    repo::list_threads(pool, workspace_id, source).await
}

pub async fn get_thread(pool: &SqlitePool, id: &str) -> AppResult<ChatThread> {
    repo::get_thread(pool, id).await
}

pub async fn find_external(
    pool: &SqlitePool,
    workspace_id: &str,
    source: &str,
    external_id: &str,
) -> AppResult<Option<ChatThread>> {
    repo::find_external(pool, workspace_id, source, external_id).await
}

pub async fn find_external_any_workspace(
    pool: &SqlitePool,
    source: &str,
    external_id: &str,
) -> AppResult<Option<ChatThread>> {
    repo::find_external_any_workspace(pool, source, external_id).await
}

pub async fn create_thread(pool: &SqlitePool, input: CreateThreadInput) -> AppResult<ChatThread> {
    repo::create_thread(pool, input).await
}

pub async fn update_session(pool: &SqlitePool, id: &str, session_id: &str) -> AppResult<()> {
    repo::update_session(pool, id, session_id).await
}

pub async fn update_title(pool: &SqlitePool, id: &str, title: &str) -> AppResult<()> {
    repo::update_title(pool, id, title).await
}

pub async fn add_message(
    pool: &SqlitePool,
    thread_id: &str,
    role: &str,
    content: &str,
    metadata: Option<&str>,
) -> AppResult<ChatMessage> {
    repo::add_message(pool, thread_id, role, content, metadata).await
}

pub async fn list_messages(pool: &SqlitePool, thread_id: &str) -> AppResult<Vec<ChatMessage>> {
    repo::list_messages(pool, thread_id).await
}

pub async fn list_messages_page(
    pool: &SqlitePool,
    thread_id: &str,
    after: Option<&str>,
    limit: u32,
) -> AppResult<Vec<ChatMessage>> {
    repo::list_messages_page(pool, thread_id, after, limit).await
}

pub async fn get_message(pool: &SqlitePool, id: &str) -> AppResult<ChatMessage> {
    repo::get_message(pool, id).await
}

pub async fn delete_thread(pool: &SqlitePool, id: &str) -> AppResult<()> {
    repo::delete_thread(pool, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::init(&temp.path().join("chat.db")).await.expect("db");
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
        assert!(get_message(&pool, &user.id).await.is_err());
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
}
