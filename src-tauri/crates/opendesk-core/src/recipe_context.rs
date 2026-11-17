//! Shared execution context for recipe engine, HITL channels and sinks.
//!
//! It intentionally contains only core-owned services plus ports implemented by
//! the shell. The shell builds it from its AppState and, in A3, injects the step
//! dispatcher as well.

use crate::{
    error::AppResult,
    events::EventBus,
    paths::AppPaths,
    recipe_dispatch::{NoopRecipeDispatcher, RecipeStepDispatcher},
    recipes::store::RecipeRun,
    secrets::SecretStore,
};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::Arc;

/// A HITL channel requested by a recipe checkpoint.
#[derive(Debug, Clone)]
pub struct HitlChannelRequest {
    pub token: String,
    pub run_id: String,
    pub workspace_id: String,
    pub interaction_type: String,
    pub title: String,
    pub message_body: String,
    pub options: Vec<String>,
    pub attachments: Vec<String>,
}

/// Delivers a HITL request through one channel (email, Signal, Telegram…).
///
/// The shell owns the integrations; the core only knows this boundary.
#[async_trait]
pub trait HitlChannelDispatcher: Send + Sync {
    async fn dispatch(
        &self,
        context: &RecipeContext,
        channel: &str,
        request: &HitlChannelRequest,
    ) -> AppResult<String>;
}

/// Executes one flow sink. The shell owns the I/O bus and its transports.
#[async_trait]
pub trait RecipeSinkExecutor: Send + Sync {
    async fn execute(
        &self,
        context: &RecipeContext,
        sink: &crate::recipes::parser::FlowSink,
        run: &RecipeRun,
        trigger: &Value,
        outputs: &Value,
    ) -> AppResult<Vec<String>>;
}

/// Used by tests and headless runs without external channels.
pub struct NoopHitlChannels;

#[async_trait]
impl HitlChannelDispatcher for NoopHitlChannels {
    async fn dispatch(
        &self,
        _context: &RecipeContext,
        channel: &str,
        _request: &HitlChannelRequest,
    ) -> AppResult<String> {
        Ok(format!("noop-{channel}"))
    }
}

/// Used by tests that never execute sinks.
pub struct NoopRecipeSinks;

#[async_trait]
impl RecipeSinkExecutor for NoopRecipeSinks {
    async fn execute(
        &self,
        _context: &RecipeContext,
        _sink: &crate::recipes::parser::FlowSink,
        _run: &RecipeRun,
        _trigger: &Value,
        _outputs: &Value,
    ) -> AppResult<Vec<String>> {
        Ok(Vec::new())
    }
}

#[derive(Clone)]
pub struct RecipeContext {
    pub pool: SqlitePool,
    pub paths: AppPaths,
    pub events: EventBus,
    pub secrets: Arc<SecretStore>,
    dispatcher: Arc<dyn RecipeStepDispatcher>,
    channels: Arc<dyn HitlChannelDispatcher>,
    sinks: Arc<dyn RecipeSinkExecutor>,
}

impl RecipeContext {
    pub fn new(
        pool: SqlitePool,
        paths: AppPaths,
        events: EventBus,
        secrets: Arc<SecretStore>,
    ) -> Self {
        Self {
            pool,
            paths,
            events,
            secrets,
            dispatcher: Arc::new(NoopRecipeDispatcher),
            channels: Arc::new(NoopHitlChannels),
            sinks: Arc::new(NoopRecipeSinks),
        }
    }

    pub fn with_dispatcher(mut self, dispatcher: Arc<dyn RecipeStepDispatcher>) -> Self {
        self.dispatcher = dispatcher;
        self
    }

    pub fn with_channels(mut self, channels: Arc<dyn HitlChannelDispatcher>) -> Self {
        self.channels = channels;
        self
    }

    pub fn with_sinks(mut self, sinks: Arc<dyn RecipeSinkExecutor>) -> Self {
        self.sinks = sinks;
        self
    }

    pub fn dispatcher(&self) -> &dyn RecipeStepDispatcher {
        self.dispatcher.as_ref()
    }

    pub fn channels(&self) -> &dyn HitlChannelDispatcher {
        self.channels.as_ref()
    }

    pub fn sinks(&self) -> &dyn RecipeSinkExecutor {
        self.sinks.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe_dispatch::RecipeStepRequest;

    #[tokio::test]
    async fn builds_from_core_services() {
        let ctx = crate::test_support::context().await;
        let secrets = Arc::new(SecretStore::new(&ctx.paths));
        let context = RecipeContext::new(
            ctx.db.clone(),
            ctx.paths.clone(),
            EventBus::new(32),
            secrets,
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT 1")
                .fetch_one(&context.pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(context.paths.data_dir, ctx.paths.data_dir);
    }

    #[tokio::test]
    async fn defaults_to_safe_noop_ports() {
        let ctx = crate::test_support::context().await;
        let context = RecipeContext::new(
            ctx.db.clone(),
            ctx.paths.clone(),
            EventBus::new(32),
            Arc::new(SecretStore::new(&ctx.paths)),
        );
        let request = HitlChannelRequest {
            token: "AB12".into(),
            run_id: "run".into(),
            workspace_id: ctx.workspace_id.clone(),
            interaction_type: "BINARY_GATE".into(),
            title: "ok?".into(),
            message_body: "ok?".into(),
            options: Vec::new(),
            attachments: Vec::new(),
        };
        assert_eq!(
            context
                .channels()
                .dispatch(&context, "DESKTOP_UI", &request)
                .await
                .unwrap(),
            "noop-DESKTOP_UI"
        );
        assert!(
            context
                .dispatcher()
                .dispatch("task", RecipeStepRequest::default(), None)
                .await
                .is_ok()
        );
    }
}
