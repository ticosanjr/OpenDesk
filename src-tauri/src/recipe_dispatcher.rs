use crate::{
    api::tasks::{self, DispatchBody},
    app_state::AppState,
    recipe_dispatch::{RecipeStepDispatcher, RecipeStepRequest},
};
use async_trait::async_trait;
use std::sync::Arc;

pub struct ShellRecipeDispatcher {
    pub state: Arc<AppState>,
}

#[async_trait]
impl RecipeStepDispatcher for ShellRecipeDispatcher {
    async fn dispatch(
        &self,
        task_id: &str,
        request: RecipeStepRequest,
        session_id: Option<&str>,
    ) -> opendesk_engine::error::AppResult<()> {
        tasks::dispatch_task_with_session(
            &self.state,
            task_id,
            DispatchBody {
                policy: None,
                manual_agent: request.agent,
                manual_model: request.model,
                manual_effort: request.effort,
                execution_mode: request.execution_mode,
                prompt_append: request.prompt_append,
            },
            None,
            session_id,
        )
        .await
        .map(|_| ())
        .map_err(Into::into)
    }
}
