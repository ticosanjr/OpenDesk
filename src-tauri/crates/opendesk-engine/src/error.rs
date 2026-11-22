//! Engine-facing error aliases. The domain keeps `DomainError`; storage owns the
//! V1-compatible shape while the remaining repositories migrate.
pub use opendesk_storage::error::{AppError, AppResult};
