use crate::error::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};

/// Persisted run outcome (`task_runs.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunStatus {
    Running,
    Success,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub const ALL: [Self; 4] = [Self::Running, Self::Success, Self::Failed, Self::Cancelled];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "RUNNING",
            Self::Success => "SUCCESS",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn parse(value: &str) -> DomainResult<Self> {
        match value.trim().to_uppercase().as_str() {
            "RUNNING" => Ok(Self::Running),
            "SUCCESS" => Ok(Self::Success),
            "FAILED" => Ok(Self::Failed),
            "CANCELLED" | "CANCELED" => Ok(Self::Cancelled),
            other => Err(DomainError::validation(format!(
                "status de execução inválido '{other}'"
            ))),
        }
    }

    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }

    pub fn can_transition_to(self, target: Self) -> bool {
        self == target || self == Self::Running
    }

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
    fn only_running_can_change() {
        assert!(RunStatus::Running.transition(RunStatus::Success).is_ok());
        assert!(RunStatus::Success.transition(RunStatus::Failed).is_err());
        assert!(RunStatus::Failed.is_terminal());
        assert!(!RunStatus::Running.is_terminal());
    }

    #[test]
    fn parses_persisted_values() {
        for status in RunStatus::ALL {
            assert_eq!(RunStatus::parse(status.as_str()).expect("status"), status);
        }
        assert!(RunStatus::parse("QUEUED").is_err());
    }
}
