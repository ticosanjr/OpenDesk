//! Engine error contract.
//!
//! Storage keeps its own V1-compatible shape; the engine adds the daemon/HTTP
//! variants used by the OpenCode client and converts both storage and domain
//! errors so callers can keep `?` at the boundary.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("OpenCode request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("invalid request: {0}")]
    BadRequest(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("OpenCode daemon error: {0}")]
    Daemon(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl From<::opendesk_storage::error::AppError> for AppError {
    fn from(error: ::opendesk_storage::error::AppError) -> Self {
        use ::opendesk_storage::error::AppError as Storage;
        match error {
            Storage::Io(error) => Self::Io(error),
            Storage::Db(error) => Self::Db(error),
            Storage::BadRequest(message) => Self::BadRequest(message),
            Storage::NotFound(message) => Self::NotFound(message),
            Storage::Conflict(message) => Self::Conflict(message),
            Storage::Internal(message) => Self::Internal(message),
        }
    }
}

impl From<opendesk_domain::DomainError> for AppError {
    fn from(error: opendesk_domain::DomainError) -> Self {
        use opendesk_domain::DomainError as Domain;
        match error {
            Domain::Validation(message) => Self::BadRequest(message),
            Domain::NotFound(message) => Self::NotFound(message),
            Domain::Conflict(message) => Self::Conflict(message),
            Domain::InvalidTransition { from, to } => {
                Self::Conflict(format!("transição inválida: {from} -> {to}"))
            }
            Domain::Internal(message) => Self::Internal(message),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(format!("{value:#}"))
    }
}
