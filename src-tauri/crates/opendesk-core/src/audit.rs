//! Local API hardening: a small fixed-window rate limiter and an audit trail
//! for mutations and sensitive reads. Request bodies are never recorded.

use crate::error::AppResult;
use axum::http::Method;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateClass {
    Read,
    Write,
    Sensitive,
}

#[derive(Debug, Clone, Copy)]
pub struct RateLimits {
    pub read_per_minute: u32,
    pub write_per_minute: u32,
    pub sensitive_per_minute: u32,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            read_per_minute: 900,
            write_per_minute: 300,
            sensitive_per_minute: 120,
        }
    }
}

impl RateLimits {
    pub fn from_env() -> Self {
        let read = std::env::var("OPENDESK_RATE_LIMIT_READ")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(900);
        let write = std::env::var("OPENDESK_RATE_LIMIT_WRITE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(300);
        let sensitive = std::env::var("OPENDESK_RATE_LIMIT_SENSITIVE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(120);
        Self {
            read_per_minute: read.max(1),
            write_per_minute: write.max(1),
            sensitive_per_minute: sensitive.max(1),
        }
    }

    fn limit(&self, class: RateClass) -> u32 {
        match class {
            RateClass::Read => self.read_per_minute,
            RateClass::Write => self.write_per_minute,
            RateClass::Sensitive => self.sensitive_per_minute,
        }
    }
}

struct Window {
    started: Instant,
    count: u32,
}

#[derive(Default)]
pub struct RateLimiter {
    limits: RateLimits,
    windows: Mutex<HashMap<RateClass, Window>>,
}

impl RateLimiter {
    pub fn new(limits: RateLimits) -> Self {
        Self {
            limits,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `Err(retry_after_seconds)` when the fixed window is exhausted.
    pub fn check(&self, class: RateClass) -> Result<(), u64> {
        let limit = self.limits.limit(class);
        let mut windows = self
            .windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Instant::now();
        let window = windows.entry(class).or_insert_with(|| Window {
            started: now,
            count: 0,
        });
        if now.duration_since(window.started) >= Duration::from_secs(60) {
            window.started = now;
            window.count = 0;
        }
        if window.count >= limit {
            let elapsed = now.duration_since(window.started).as_secs();
            return Err(60u64.saturating_sub(elapsed).max(1));
        }
        window.count += 1;
        Ok(())
    }

    pub fn limits(&self) -> RateLimits {
        self.limits
    }
}

pub fn rate_class(method: &Method, path: &str) -> Option<RateClass> {
    if method == Method::OPTIONS
        || path == "/healthz"
        || path == "/api/health"
        || path == "/ws"
        || path.starts_with("/api/webhooks/")
    {
        return None;
    }
    if method == Method::GET || method == Method::HEAD {
        // The Studio loads several small read-only endpoints at once. They are
        // local, token-protected and idempotent; counting them with the global
        // read limiter caused false "rate_limited" errors while browsing.
        if path.starts_with("/api/studio")
            || path.starts_with("/api/agents")
            || path.starts_with("/api/models")
        {
            return None;
        }
    }
    if is_sensitive(method, path) {
        return Some(RateClass::Sensitive);
    }
    if method == Method::GET || method == Method::HEAD {
        Some(RateClass::Read)
    } else {
        Some(RateClass::Write)
    }
}

fn is_sensitive(method: &Method, path: &str) -> bool {
    if method == Method::GET || method == Method::HEAD {
        return path.ends_with("/download") || path.ends_with("/export");
    }
    path.starts_with("/api/backups")
        || path.starts_with("/api/browse")
        || path.starts_with("/api/integrations")
        || path.starts_with("/api/settings")
        || path.starts_with("/api/studio")
        || path.starts_with("/api/starter-packs")
        || path.starts_with("/api/tasks")
        || path.starts_with("/api/recipes")
        || path.starts_with("/api/schedules")
        || path.starts_with("/api/permissions")
        || path.starts_with("/api/proposals")
        || path.starts_with("/api/governance")
        || path.starts_with("/api/learning")
}

pub fn category(path: &str) -> &'static str {
    if path.starts_with("/api/backups") {
        "backup"
    } else if path.starts_with("/api/browse") {
        "browse"
    } else if path.starts_with("/api/governance") {
        "governance"
    } else if path.starts_with("/api/learning") {
        "learning"
    } else if path.starts_with("/api/tasks") {
        "task"
    } else if path.starts_with("/api/proposals") {
        "proposal"
    } else if path.starts_with("/api/recipes/hitl") {
        "hitl"
    } else if path.starts_with("/api/recipes") {
        "recipe"
    } else if path.starts_with("/api/schedules") {
        "routine"
    } else if path.starts_with("/api/permissions") {
        "permission"
    } else if path.starts_with("/api/integrations") {
        "integration"
    } else if path.starts_with("/api/settings") {
        "settings"
    } else if path.starts_with("/api/studio") {
        "studio"
    } else if path.starts_with("/api/starter-packs") {
        "pack"
    } else if path.starts_with("/api/queue") {
        "queue"
    } else if path.starts_with("/api/artifacts") {
        "artifact"
    } else if path.starts_with("/api/calendar") {
        "calendar"
    } else if path.starts_with("/api/runs") {
        "run"
    } else if path.starts_with("/api/io") || path.starts_with("/api/webhooks") {
        "io"
    } else {
        "api"
    }
}

pub fn should_audit(method: &Method, path: &str) -> bool {
    if method == Method::OPTIONS || path == "/healthz" || path == "/api/health" || path == "/ws" {
        return false;
    }
    method != Method::GET && method != Method::HEAD
        || path.ends_with("/download")
        || path.ends_with("/export")
}

fn workspace_from_query(uri: &axum::http::Uri) -> Option<String> {
    let query = uri.query()?;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key != "workspace_id" {
            return None;
        }
        urlencoding::decode(value)
            .ok()
            .map(|value| value.into_owned())
    })
}

pub fn action(method: &Method, uri: &axum::http::Uri) -> String {
    format!("{}:{}", category(uri.path()), method.as_str())
}

pub async fn record(
    pool: &SqlitePool,
    method: &Method,
    uri: &axum::http::Uri,
    status: u16,
    duration_ms: i64,
) -> AppResult<()> {
    if !should_audit(method, uri.path()) {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO api_audit (
            id, workspace_id, action, method, path, status, duration_ms
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(workspace_from_query(uri))
    .bind(action(method, uri))
    .bind(method.as_str())
    .bind(uri.path())
    .bind(i64::from(status))
    .bind(duration_ms.max(0))
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AuditEntry {
    pub id: String,
    pub workspace_id: Option<String>,
    pub action: String,
    pub method: String,
    pub path: String,
    pub status: i64,
    pub duration_ms: i64,
    pub actor: String,
    pub created_at: String,
}

pub async fn list(
    pool: &SqlitePool,
    workspace_id: Option<&str>,
    action: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AuditEntry>> {
    Ok(sqlx::query_as::<_, AuditEntry>(
        "SELECT id, workspace_id, action, method, path, status, duration_ms, actor, created_at
         FROM api_audit
         WHERE (? IS NULL OR workspace_id = ?)
           AND (? IS NULL OR action = ?)
         ORDER BY created_at DESC LIMIT ?",
    )
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(action)
    .bind(action)
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?)
}

/// Records one MCP tool decision (`allow`, `deny` or `rate_limited`).
pub async fn record_mcp_tool(
    pool: &SqlitePool,
    workspace_id: &str,
    actor: &str,
    tool: &str,
    status: u16,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO api_audit
            (id, workspace_id, action, method, path, status, duration_ms, actor)
         VALUES (?, ?, ?, 'POST', '/api/mcp', ?, 0, ?)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(workspace_id)
    .bind(format!("mcp:{tool}"))
    .bind(i64::from(status))
    .bind(actor)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn classifies_routes_and_sensitive_reads() {
        assert_eq!(
            rate_class(&Method::GET, "/api/tasks"),
            Some(RateClass::Read)
        );
        assert_eq!(
            rate_class(&Method::POST, "/api/tasks"),
            Some(RateClass::Sensitive)
        );
        assert_eq!(
            rate_class(&Method::GET, "/api/runs/1/export"),
            Some(RateClass::Sensitive)
        );
        assert_eq!(rate_class(&Method::GET, "/healthz"), None);
        assert_eq!(rate_class(&Method::GET, "/api/studio/agents"), None);
        assert_eq!(rate_class(&Method::GET, "/api/agents"), None);
        assert_eq!(
            rate_class(&Method::POST, "/api/studio/agent"),
            Some(RateClass::Sensitive)
        );
        assert!(should_audit(&Method::POST, "/api/tasks"));
        assert!(!should_audit(&Method::GET, "/api/tasks"));
    }

    #[test]
    fn rate_limiter_blocks_after_the_limit_and_resets() {
        let limiter = RateLimiter::new(RateLimits {
            read_per_minute: 2,
            write_per_minute: 1,
            sensitive_per_minute: 1,
        });
        assert!(limiter.check(RateClass::Read).is_ok());
        assert!(limiter.check(RateClass::Read).is_ok());
        assert!(limiter.check(RateClass::Read).is_err());
        assert!(limiter.check(RateClass::Sensitive).is_ok());
        assert!(limiter.check(RateClass::Sensitive).is_err());
    }

    #[tokio::test]
    async fn records_and_lists_audit_entries() {
        let ctx = test_support::context().await;
        let uri: axum::http::Uri = format!("/api/backups?workspace_id={}", ctx.workspace_id)
            .parse()
            .expect("uri");
        record(&ctx.db, &Method::POST, &uri, 200, 12)
            .await
            .expect("record");
        record(&ctx.db, &Method::GET, &uri, 200, 1)
            .await
            .expect("ignored");
        let entries = list(&ctx.db, Some(&ctx.workspace_id), None, 10)
            .await
            .expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "backup:POST");
        assert_eq!(entries[0].status, 200);
    }
}
