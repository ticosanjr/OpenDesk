use crate::{
    app_state::AppState,
    error::AppResult,
    recipes::{cron::CronTrigger, engine, parser, store},
};
use std::sync::Arc;

pub(super) async fn scan_cron_triggers(state: &Arc<AppState>) -> AppResult<()> {
    let now = chrono::Utc::now();
    for index in store::list_all(&state.db).await? {
        if !index.is_enabled {
            continue;
        }
        let Ok(yaml) = store::yaml_for(&state.db, &index.workspace_id, &index.id).await else {
            continue;
        };
        let Ok(recipe) = parser::parse(&yaml) else {
            continue;
        };
        let Some(cron) = recipe
            .triggers()
            .into_iter()
            .find_map(CronTrigger::from_trigger)
        else {
            continue;
        };
        let after = index
            .last_run_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc))
            .unwrap_or_else(|| now - chrono::Duration::minutes(1));
        if let Ok(next) = cron.next_occurrence(after)
            && next <= now
        {
            let trigger = serde_json::json!({
                "cron": true,
                "scheduled_for": next.to_rfc3339(),
                "recipe_id": recipe.id,
            });
            let _ = engine::start(
                crate::app_state::recipe_context(state),
                &index.workspace_id,
                recipe.clone(),
                trigger,
                false,
            )
            .await;
        }
    }
    Ok(())
}
