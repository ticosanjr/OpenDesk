use crate::error::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};

/// Kanban lifecycle. The string values match the SQLite rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Backlog,
    Todo,
    InProgress,
    InReview,
    Blocked,
    Done,
    Cancelled,
}

impl TaskStatus {
    pub const ALL: [Self; 7] = [
        Self::Backlog,
        Self::Todo,
        Self::InProgress,
        Self::InReview,
        Self::Blocked,
        Self::Done,
        Self::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "BACKLOG",
            Self::Todo => "TODO",
            Self::InProgress => "IN_PROGRESS",
            Self::InReview => "IN_REVIEW",
            Self::Blocked => "BLOCKED",
            Self::Done => "DONE",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn parse(value: &str) -> DomainResult<Self> {
        match value.trim().to_uppercase().as_str() {
            "BACKLOG" => Ok(Self::Backlog),
            "TODO" => Ok(Self::Todo),
            "IN_PROGRESS" => Ok(Self::InProgress),
            "IN_REVIEW" => Ok(Self::InReview),
            "BLOCKED" => Ok(Self::Blocked),
            "DONE" => Ok(Self::Done),
            "CANCELLED" | "CANCELED" => Ok(Self::Cancelled),
            other => Err(DomainError::validation(format!(
                "status de tarefa inválido '{other}'"
            ))),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }

    /// Workflow stages accepted by the Kanban UI. `DONE`/`CANCELLED` are
    /// terminal, but a fork may reopen a task, which the caller decides.
    pub fn allowed_targets(self) -> &'static [Self] {
        match self {
            Self::Backlog => &[Self::Todo, Self::Cancelled],
            Self::Todo => &[Self::InProgress, Self::Blocked, Self::Cancelled],
            Self::InProgress => &[Self::InReview, Self::Blocked, Self::Done, Self::Cancelled],
            Self::InReview => &[Self::InProgress, Self::Blocked, Self::Done, Self::Cancelled],
            Self::Blocked => &[Self::Todo, Self::InProgress, Self::Cancelled],
            Self::Done | Self::Cancelled => &[],
        }
    }

    pub fn can_transition_to(self, target: Self) -> bool {
        self == target || self.allowed_targets().contains(&target)
    }

    /// Validates a transition; an invalid move is a domain error.
    pub fn transition(self, target: Self) -> DomainResult<Self> {
        if self.can_transition_to(target) {
            Ok(target)
        } else {
            Err(DomainError::invalid_transition(
                self.as_str(),
                target.as_str(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_every_status() {
        for status in TaskStatus::ALL {
            assert_eq!(TaskStatus::parse(status.as_str()).expect("status"), status);
        }
        assert_eq!(TaskStatus::parse(" done ").expect("case").as_str(), "DONE");
        assert!(TaskStatus::parse("NOPE").is_err());
    }

    #[test]
    fn terminal_statuses_have_no_exit() {
        assert!(TaskStatus::Done.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
        assert!(TaskStatus::Done.allowed_targets().is_empty());
        assert!(TaskStatus::Done.transition(TaskStatus::Todo).is_err());
    }

    #[test]
    fn happy_path_transitions_are_allowed() {
        let status = TaskStatus::Todo
            .transition(TaskStatus::InProgress)
            .and_then(|status| status.transition(TaskStatus::InReview))
            .and_then(|status| status.transition(TaskStatus::Done))
            .expect("workflow");
        assert_eq!(status, TaskStatus::Done);
    }

    #[test]
    fn skipping_the_workflow_is_rejected() {
        let error = TaskStatus::Backlog
            .transition(TaskStatus::Done)
            .expect_err("invalid");
        assert!(error.to_string().contains("BACKLOG -> DONE"), "{error}");
        assert!(!TaskStatus::Backlog.can_transition_to(TaskStatus::Done));
    }
}
