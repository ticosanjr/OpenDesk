use crate::{
    app_state::AppState,
    db,
    error::AppResult,
    recipes::{engine, parser, store},
};
use std::sync::Arc;

pub(super) async fn scan_calendar_triggers(state: &Arc<AppState>) -> AppResult<()> {
    let now = chrono::Utc::now();
    let mut candidates = Vec::new();
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
        let triggers = recipe
            .triggers()
            .into_iter()
            .filter(|trigger| trigger.trigger_type == "CALENDAR_EVENT")
            .cloned()
            .collect::<Vec<_>>();
        if !triggers.is_empty() {
            candidates.push((index, recipe, triggers));
        }
    }
    if candidates.is_empty() {
        return Ok(());
    }

    let events = db::calendar::list_events_starting_between(
        &state.db,
        &(now - chrono::Duration::days(1)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        &(now + chrono::Duration::days(8)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    )
    .await?;
    if events.is_empty() {
        return Ok(());
    }

    for (index, recipe, triggers) in candidates {
        let sources = db::calendar::list_sources(&state.db, &index.workspace_id)
            .await
            .unwrap_or_default();
        for trigger_config in triggers {
            let lead = trigger_config
                .config
                .get("lead_minutes")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0)
                .clamp(0, 10_080);
            let tolerance = trigger_config
                .config
                .get("lookahead_minutes")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(10)
                .clamp(1, 1_440);
            let calendar_id = trigger_config
                .config
                .get("calendar_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .trim();
            let title_filter = trigger_config
                .config
                .get("match")
                .or_else(|| trigger_config.config.get("title_contains"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .trim()
                .to_lowercase();

            for event in &events {
                if event.workspace_id != index.workspace_id {
                    continue;
                }
                let belongs = event.source_id.as_deref() == Some(calendar_id)
                    || sources.iter().any(|source| {
                        source.external_id.as_deref() == Some(calendar_id)
                            && event.source_id.as_deref() == Some(source.id.as_str())
                    });
                if !calendar_id.is_empty() && !belongs {
                    continue;
                }
                if !title_filter.is_empty() && !event.title.to_lowercase().contains(&title_filter) {
                    continue;
                }
                let Ok(start) = chrono::DateTime::parse_from_rfc3339(&event.start_at) else {
                    continue;
                };
                let trigger_at =
                    start.with_timezone(&chrono::Utc) - chrono::Duration::minutes(lead);
                let window_end = trigger_at + chrono::Duration::minutes(tolerance);
                if now < trigger_at || now > window_end {
                    continue;
                }
                // Claim before starting so a restart cannot fire the same occurrence twice.
                if !db::calendar::claim_trigger_fire(
                    &state.db,
                    &recipe.id,
                    &event.id,
                    &event.start_at,
                )
                .await?
                {
                    continue;
                }
                let trigger = serde_json::json!({
                    "calendar_event": {
                        "id": event.id,
                        "external_id": event.external_id,
                        "title": event.title,
                        "description": event.description,
                        "location": event.location,
                        "start": event.start_at,
                        "end": event.end_at,
                        "all_day": event.all_day,
                        "source_id": event.source_id,
                    },
                    "event_id": event.id,
                    "event_title": event.title,
                    "event_start": event.start_at,
                    "recipe_id": recipe.id,
                });
                if let Err(error) = engine::start(
                    crate::app_state::recipe_context(state),
                    &index.workspace_id,
                    recipe.clone(),
                    trigger,
                    false,
                )
                .await
                {
                    // Release the claim so a transient failure is retried.
                    let _ = db::calendar::release_trigger_fire(
                        &state.db,
                        &recipe.id,
                        &event.id,
                        &event.start_at,
                    )
                    .await;
                    tracing::warn!(recipe = %recipe.id, error = %error, "calendar recipe trigger failed; will retry");
                }
            }
        }
    }
    Ok(())
}
