//! Step dispatch boundary for recipe execution.
//!
//! The engine must not depend on HTTP handlers or AppState. A shell injects a
//! dispatcher that enqueues the step exactly like a normal task.

use crate::error::AppResult;
use async_trait::async_trait;

#[derive(Debug, Clone, Default)]
pub struct RecipeStepRequest {
    pub agent: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub execution_mode: Option<String>,
    pub prompt_append: Option<String>,
}

#[async_trait]
pub trait RecipeStepDispatcher: Send + Sync {
    async fn dispatch(
        &self,
        task_id: &str,
        request: RecipeStepRequest,
        session_id: Option<&str>,
    ) -> AppResult<()>;
}

/// Used by tests and dry flows that never dispatch a real run.
pub struct NoopRecipeDispatcher;

#[async_trait]
impl RecipeStepDispatcher for NoopRecipeDispatcher {
    async fn dispatch(
        &self,
        _task_id: &str,
        _request: RecipeStepRequest,
        _session_id: Option<&str>,
    ) -> AppResult<()> {
        Ok(())
    }
}
