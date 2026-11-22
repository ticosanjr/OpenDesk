use crate::{
    conversation::{Conversation, Message, NewMessage},
    error::DomainResult,
    ids::{ConversationId, TaskId, WorkspaceId},
    task::TaskStatus,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationLevel {
    Info,
    Warning,
    Critical,
}

/// A channel-agnostic notification request. `NotificationRouter` (channels)
/// decides who receives it; the engine only fills the intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationIntent {
    pub workspace_id: WorkspaceId,
    pub level: NotificationLevel,
    pub title: String,
    pub body: String,
    /// Opaque reference (task id, run id, artifact) for deep links.
    pub reference: Option<String>,
}

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, intent: NotificationIntent) -> DomainResult<()>;
}

#[async_trait]
pub trait TaskDispatcher: Send + Sync {
    async fn dispatch(&self, task_id: &TaskId) -> DomainResult<()>;
}

/// Persistence contract for the unified conversation stream.
#[async_trait]
pub trait ConversationRepository: Send + Sync {
    async fn get_or_create(
        &self,
        workspace_id: &WorkspaceId,
        task_id: Option<&TaskId>,
        channel: &str,
        thread_key: Option<&str>,
        title: &str,
    ) -> DomainResult<Conversation>;

    async fn append(
        &self,
        conversation_id: &ConversationId,
        message: NewMessage,
    ) -> DomainResult<Message>;

    /// Cursor pagination: `after` is the last message id (UUIDv7, time ordered).
    async fn list(
        &self,
        conversation_id: &ConversationId,
        after: Option<&str>,
        limit: u32,
    ) -> DomainResult<Vec<Message>>;
}

/// Minimal task lifecycle contract; the storage repo validates transitions
/// through `TaskStatus` before writing.
#[async_trait]
pub trait TaskStatusRepository: Send + Sync {
    async fn status(&self, task_id: &TaskId) -> DomainResult<TaskStatus>;
    async fn transition(&self, task_id: &TaskId, to: TaskStatus) -> DomainResult<TaskStatus>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_moves_forward() {
        let clock = SystemClock;
        let first = clock.now();
        let second = clock.now();
        assert!(second >= first);
    }

    #[test]
    fn notification_intent_serializes_without_channel_names() {
        let intent = NotificationIntent {
            workspace_id: WorkspaceId::from("ws-1"),
            level: NotificationLevel::Critical,
            title: "Falha".into(),
            body: "A execução falhou".into(),
            reference: Some("run-1".into()),
        };
        let json = serde_json::to_string(&intent).expect("serialize");
        assert!(json.contains("\"CRITICAL\""));
        assert!(!json.contains("telegram"));
        assert!(!json.contains("whatsapp"));
    }
}
