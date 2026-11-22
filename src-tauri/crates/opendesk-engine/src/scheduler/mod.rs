pub mod recurrence;

pub use recurrence::next_occurrence;

/// Port used by the cron engine to start a recipe run without the engine
/// depending on the recipes crate.
#[async_trait::async_trait]
pub trait ScheduledRecipeRunner: Send + Sync {
    async fn run_schedule(
        &self,
        schedule: &opendesk_storage::repos::schedules::Schedule,
        scheduled_for: &str,
    ) -> crate::error::AppResult<serde_json::Value>;
}
