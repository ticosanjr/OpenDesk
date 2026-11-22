//! Notification boundary for run-lifecycle events.
//!
//! The core runtime must not depend on channel integrations (Telegram, Signal,
//! WhatsApp). Shells inject a notifier so permission requests can be delivered
//! without pulling the integration modules into every harness/daemon build.

use crate::error::AppResult;
use opendesk_storage::secrets::SecretStore;

use async_trait::async_trait;
use sqlx::SqlitePool;

pub struct PermissionNotice<'a> {
    pub pool: &'a SqlitePool,
    pub secrets: &'a SecretStore,
    pub workspace_id: &'a str,
    pub run_id: &'a str,
    pub action: &'a str,
    pub resources: &'a [String],
    pub request_id: &'a str,
}

#[async_trait]
pub trait RunNotifier: Send + Sync {
    async fn permission_requested(&self, notice: PermissionNotice<'_>) -> AppResult<()>;
}

/// Used by tests and headless runs without channels.
pub struct NoopNotifier;

#[async_trait]
impl RunNotifier for NoopNotifier {
    async fn permission_requested(&self, _notice: PermissionNotice<'_>) -> AppResult<()> {
        Ok(())
    }
}
