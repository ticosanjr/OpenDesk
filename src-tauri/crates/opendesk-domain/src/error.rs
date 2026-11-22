use thiserror::Error;

/// Errors that belong to the domain, independent of storage or transport.
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid input: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },
    #[error("internal error: {0}")]
    Internal(String),
}

pub type DomainResult<T> = Result<T, DomainError>;

impl DomainError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn invalid_transition(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self::InvalidTransition {
            from: from.into(),
            to: to.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_actionable() {
        let error = DomainError::invalid_transition("DONE", "TODO");
        assert_eq!(error.to_string(), "invalid transition: DONE -> TODO");
        assert_eq!(
            DomainError::validation("campo vazio").to_string(),
            "invalid input: campo vazio"
        );
    }
}
