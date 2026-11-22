use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
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

impl From<opendesk_storage::error::AppError> for AppError {
    fn from(error: opendesk_storage::error::AppError) -> Self {
        use opendesk_storage::error::AppError as Storage;
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

impl From<opendesk_engine::error::AppError> for AppError {
    fn from(error: opendesk_engine::error::AppError) -> Self {
        use opendesk_engine::error::AppError as Engine;
        match error {
            Engine::Io(error) => Self::Io(error),
            Engine::Db(error) => Self::Db(error),
            Engine::Http(error) => Self::Http(error),
            Engine::BadRequest(message) => Self::BadRequest(message),
            Engine::NotFound(message) => Self::NotFound(message),
            Engine::Conflict(message) => Self::Conflict(message),
            Engine::Daemon(message) => Self::Daemon(message),
            Engine::Internal(message) => Self::Internal(message),
        }
    }
}

impl From<AppError> for opendesk_engine::error::AppError {
    fn from(error: AppError) -> Self {
        use opendesk_engine::error::AppError as Engine;
        match error {
            AppError::Io(error) => Engine::Io(error),
            AppError::Db(error) => Engine::Db(error),
            AppError::Http(error) => Engine::Http(error),
            AppError::BadRequest(message) => Engine::BadRequest(message),
            AppError::NotFound(message) => Engine::NotFound(message),
            AppError::Conflict(message) => Engine::Conflict(message),
            AppError::Daemon(message) => Engine::Daemon(message),
            AppError::Internal(message) => Engine::Internal(message),
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

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl AppError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "invalid_request"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::Daemon(_) => (StatusCode::SERVICE_UNAVAILABLE, "daemon_unavailable"),
            Self::Http(_) => (StatusCode::BAD_GATEWAY, "opencode_error"),
            Self::Io(_) | Self::Db(_) | Self::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        if status.is_server_error() {
            tracing::error!(error = %self, "request failed");
        } else {
            tracing::warn!(error = %self, "request rejected");
        }
        (
            status,
            Json(ErrorBody {
                code,
                message: self.to_string(),
            }),
        )
            .into_response()
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
