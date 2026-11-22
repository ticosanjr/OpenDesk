use crate::error::{DomainError, DomainResult};
use crate::ids::{ConversationId, MessageId, TaskId, WorkspaceId};
use serde::{Deserialize, Serialize};

/// Who wrote a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl MessageRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
        }
    }

    pub fn parse(value: &str) -> DomainResult<Self> {
        match value.trim().to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "system" => Ok(Self::System),
            other => Err(DomainError::validation(format!(
                "role de mensagem inválido '{other}'"
            ))),
        }
    }
}

/// What the message represents. Chat, task comments, HITL requests and run
/// turns share the same table so every surface can paginate one stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Chat,
    Comment,
    HitlRequest,
    Turn,
    System,
}

impl MessageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Comment => "comment",
            Self::HitlRequest => "hitl_request",
            Self::Turn => "turn",
            Self::System => "system",
        }
    }

    pub fn parse(value: &str) -> DomainResult<Self> {
        match value.trim().to_lowercase().as_str() {
            "chat" => Ok(Self::Chat),
            "comment" => Ok(Self::Comment),
            "hitl_request" => Ok(Self::HitlRequest),
            "turn" => Ok(Self::Turn),
            "system" => Ok(Self::System),
            other => Err(DomainError::validation(format!(
                "kind de mensagem inválido '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl MessageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Running => "RUNNING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn parse(value: &str) -> DomainResult<Self> {
        match value.trim().to_uppercase().as_str() {
            "PENDING" => Ok(Self::Pending),
            "RUNNING" => Ok(Self::Running),
            "COMPLETED" => Ok(Self::Completed),
            "FAILED" => Ok(Self::Failed),
            "CANCELLED" | "CANCELED" => Ok(Self::Cancelled),
            other => Err(DomainError::validation(format!(
                "status de mensagem inválido '{other}'"
            ))),
        }
    }
}

/// A conversation groups every message of a chat thread, task or run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: ConversationId,
    pub workspace_id: WorkspaceId,
    pub task_id: Option<TaskId>,
    pub run_id: Option<String>,
    pub channel: String,
    pub thread_key: Option<String>,
    pub title: String,
    pub status: String,
    /// Stable idempotency key for channel deliveries.
    pub external_id: Option<String>,
}

impl Conversation {
    /// The conversation that owns a task's comments and HITL requests.
    pub fn task_key(task_id: &TaskId) -> String {
        format!("task:{task_id}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub conversation_id: ConversationId,
    pub role: MessageRole,
    pub kind: MessageKind,
    pub body: String,
    pub payload: serde_json::Value,
    pub status: MessageStatus,
    pub external_id: Option<String>,
    pub created_at: String,
}

/// New message before persistence assigns an id/timestamp.
#[derive(Debug, Clone)]
pub struct NewMessage {
    pub role: MessageRole,
    pub kind: MessageKind,
    pub body: String,
    pub payload: serde_json::Value,
    pub dedupe_key: Option<String>,
    pub external_id: Option<String>,
}

impl NewMessage {
    pub fn text(role: MessageRole, kind: MessageKind, body: impl Into<String>) -> Self {
        Self {
            role,
            kind,
            body: body.into(),
            payload: serde_json::Value::Null,
            dedupe_key: None,
            external_id: None,
        }
    }

    pub fn validate(&self) -> DomainResult<()> {
        if self.body.trim().is_empty() && self.payload.is_null() {
            return Err(DomainError::validation("mensagem vazia"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_and_kind_round_trip() {
        for role in [
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::System,
        ] {
            assert_eq!(MessageRole::parse(role.as_str()).expect("role"), role);
        }
        for kind in [
            MessageKind::Chat,
            MessageKind::Comment,
            MessageKind::HitlRequest,
            MessageKind::Turn,
            MessageKind::System,
        ] {
            assert_eq!(MessageKind::parse(kind.as_str()).expect("kind"), kind);
        }
        assert!(MessageRole::parse("bot").is_err());
    }

    #[test]
    fn message_needs_content_or_payload() {
        let empty = NewMessage::text(MessageRole::User, MessageKind::Chat, "   ");
        assert!(empty.validate().is_err());
        let mut with_payload = empty;
        with_payload.payload = serde_json::json!({ "options": ["a"] });
        assert!(with_payload.validate().is_ok());
    }

    #[test]
    fn task_conversation_key_is_stable() {
        let task = TaskId::from("task-1");
        assert_eq!(Conversation::task_key(&task), "task:task-1");
    }
}
