use opendesk_domain::DomainError;
use thiserror::Error;

/// Storage error with the same shape the V1 repositories used, so modules can
/// move here without rewriting every `?` in the shell. The core converts it
/// into its public `AppError`.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("invalid request: {0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl From<DomainError> for AppError {
    fn from(error: DomainError) -> Self {
        match error {
            DomainError::Validation(message) => Self::BadRequest(message),
            DomainError::NotFound(message) => Self::NotFound(message),
            DomainError::Conflict(message) => Self::Conflict(message),
            DomainError::InvalidTransition { from, to } => {
                Self::Conflict(format!("transição inválida: {from} -> {to}"))
            }
            DomainError::Internal(message) => Self::Internal(message),
        }
    }
}

impl From<AppError> for DomainError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Io(error) => Self::Internal(error.to_string()),
            AppError::Db(error) => Self::Internal(error.to_string()),
            AppError::BadRequest(message) => Self::Validation(message),
            AppError::NotFound(message) => Self::NotFound(message),
            AppError::Conflict(message) => Self::Conflict(message),
            AppError::Internal(message) => Self::Internal(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_errors_map_to_actionable_variants() {
        let error: AppError = DomainError::validation("vazio").into();
        assert!(matches!(error, AppError::BadRequest(ref message) if message == "vazio"));
        let back: DomainError = error.into();
        assert!(matches!(back, DomainError::Validation(ref message) if message == "vazio"));
    }
}
