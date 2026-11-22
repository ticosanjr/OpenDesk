use crate::{
    conversation::MessageKind,
    hitl::{HitlKind, HitlOutcome, HitlStatus},
    ids::{ConversationId, MessageId, RunId, TaskId, WorkspaceId},
    run::RunStatus,
    task::TaskStatus,
};
use serde::{Deserialize, Serialize};

/// State changes the domain publishes. Transport (WebSocket, push, channels)
/// is a subscriber concern; the domain never names a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DomainEvent {
    TaskCreated {
        task_id: TaskId,
        workspace_id: WorkspaceId,
        title: String,
    },
    TaskStatusChanged {
        task_id: TaskId,
        workspace_id: WorkspaceId,
        from: TaskStatus,
        to: TaskStatus,
    },
    RunStarted {
        run_id: RunId,
        task_id: Option<TaskId>,
        workspace_id: WorkspaceId,
    },
    RunFinished {
        run_id: RunId,
        status: RunStatus,
    },
    HitlRequested {
        message_id: MessageId,
        conversation_id: ConversationId,
        workspace_id: WorkspaceId,
        kind: HitlKind,
    },
    HitlResolved {
        message_id: MessageId,
        conversation_id: ConversationId,
        status: HitlStatus,
        outcome: HitlOutcome,
    },
    MessageAppended {
        message_id: MessageId,
        conversation_id: ConversationId,
        kind: MessageKind,
    },
}

impl DomainEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TaskCreated { .. } => "TASK_CREATED",
            Self::TaskStatusChanged { .. } => "TASK_STATUS_CHANGED",
            Self::RunStarted { .. } => "RUN_STARTED",
            Self::RunFinished { .. } => "RUN_FINISHED",
            Self::HitlRequested { .. } => "HITL_REQUESTED",
            Self::HitlResolved { .. } => "HITL_RESOLVED",
            Self::MessageAppended { .. } => "MESSAGE_APPENDED",
        }
    }
}

/// Anything that broadcasts domain events (in-process bus, test recorder).
pub trait EventSink: Send + Sync {
    fn publish(&self, event: &DomainEvent);
}

pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn publish(&self, _event: &DomainEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_matches_the_variant() {
        let event = DomainEvent::RunFinished {
            run_id: RunId::from("run-1"),
            status: RunStatus::Success,
        };
        assert_eq!(event.kind(), "RUN_FINISHED");
        let json = serde_json::to_value(&event).expect("serialize");
        assert_eq!(json["type"], "RUN_FINISHED");
        assert_eq!(json["status"], "SUCCESS");
    }
}
