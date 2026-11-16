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
