//! Shell-facing adapter that starts a recipe from a schedule.

use crate::context::RecipeContext;
use crate::{engine, parser, store};
use async_trait::async_trait;
use opendesk_engine::error::AppResult;
use opendesk_engine::scheduler::ScheduledRecipeRunner;
use opendesk_storage::repos::schedules::Schedule;

pub struct ContextRecipeRunner {
    context: RecipeContext,
}

impl ContextRecipeRunner {
    pub fn new(context: RecipeContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl ScheduledRecipeRunner for ContextRecipeRunner {
    async fn run_schedule(
        &self,
        schedule: &Schedule,
        scheduled_for: &str,
    ) -> AppResult<serde_json::Value> {
        let recipe_id = schedule.recipe_id.as_deref().ok_or_else(|| {
            crate::error::AppError::BadRequest("rotina sem receita vinculada".into())
        })?;
        let yaml = store::yaml_for(&self.context.pool, &schedule.workspace_id, recipe_id).await?;
        let recipe = parser::parse(&yaml)?;
        let trigger = serde_json::json!({
            "routine_id": schedule.id,
            "routine_name": schedule.name,
            "scheduled_for": scheduled_for,
        });
        let run = engine::start(
            self.context.clone(),
            &schedule.workspace_id,
            recipe,
            trigger,
            false,
        )
        .await?;
        Ok(serde_json::json!({
            "run_id": run.id,
            "recipe_id": recipe_id,
        }))
    }
}
