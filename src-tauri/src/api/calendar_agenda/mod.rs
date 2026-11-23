use super::{AgendaItem, parse_bound, rfc3339};
use crate::{
    app_state::AppState,
    db::{calendar, missed_runs, queue, runs},
    error::AppResult,
    recipes::store,
};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashSet;

mod items;
mod query;

use items::{queued_item, recipe_run_item, run_item};

/// Collects every agenda source for one window. The HTTP handler only owns
/// range validation, ordering and truncation.
pub(super) async fn collect(
    state: &AppState,
    workspace_id: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    now: DateTime<Utc>,
    workspace_color: Option<String>,
) -> AppResult<Vec<AgendaItem>> {
    let mut entries = Vec::new();

    let sources = calendar::list_sources(&state.db, workspace_id).await?;
    query::collect_events(
        &state.db,
        workspace_id,
        &sources,
        from,
        to,
        now,
        &mut entries,
    )
    .await?;

    let all_runs = runs::list_with_limit(&state.db, workspace_id, None, 500).await?;
    let all_recipe_runs = store::list_runs(&state.db, workspace_id, 500).await?;
    let missed =
        missed_runs::list_range(&state.db, workspace_id, &rfc3339(from), &rfc3339(to)).await?;
    let mut used_runs = HashSet::new();
    let mut used_recipe_runs = HashSet::new();
    let recipe_task_ids = all_recipe_runs
        .iter()
        .filter_map(|run| run.task_id.clone())
        .collect::<HashSet<_>>();

    let agenda = query::AgendaQuery {
        pool: &state.db,
        workspace_id,
        from,
        to,
        now,
        workspace_color: workspace_color.clone(),
        all_runs: &all_runs,
    };
    query::collect_routines(&agenda, &missed, &mut used_runs, &mut entries).await?;

    let recipe_index = query::collect_recipes(
        &agenda,
        &all_recipe_runs,
        &mut used_runs,
        &mut used_recipe_runs,
        &mut entries,
    )
    .await?;

    for run in &all_runs {
        if used_runs.contains(&run.id)
            || run.schedule_id.is_some()
            || run
                .task_id
                .as_deref()
                .is_some_and(|task_id| recipe_task_ids.contains(task_id))
        {
            continue;
        }
        let Ok(start) = parse_bound(&run.started_at, "started_at") else {
            continue;
        };
        if start < from || start > to {
            continue;
        }
        let end = run
            .completed_at
            .as_deref()
            .and_then(|value| parse_bound(value, "completed_at").ok())
            .unwrap_or(start + Duration::minutes(1));
        entries.push(run_item(run, start, end, now));
    }

    for run in &all_recipe_runs {
        if used_recipe_runs.contains(&run.id) || !recipe_index.enabled.contains(&run.recipe_id) {
            continue;
        }
        let Ok(start) = parse_bound(&run.started_at, "started_at") else {
            continue;
        };
        if start < from || start > to {
            continue;
        }
        let end = run
            .completed_at
            .as_deref()
            .and_then(|value| parse_bound(value, "completed_at").ok())
            .unwrap_or(start + Duration::minutes(1));
        let title = recipe_index
            .names
            .get(&run.recipe_id)
            .cloned()
            .unwrap_or_else(|| run.recipe_id.clone());
        entries.push(recipe_run_item(run, title, start, end, now));
    }

    for job in queue::list(&state.db, Some(workspace_id)).await? {
        if !matches!(job.status.as_str(), "QUEUED" | "PAUSED") {
            continue;
        }
        let Some(scheduled) = job
            .scheduled_for
            .as_deref()
            .and_then(|value| parse_bound(value, "scheduled_for").ok())
        else {
            continue;
        };
        if scheduled < from || scheduled > to {
            continue;
        }
        let end = scheduled + Duration::minutes(1);
        entries.push(queued_item(&job, scheduled, end, now));
    }

    Ok(entries)
}
