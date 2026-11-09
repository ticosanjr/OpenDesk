use super::*;
use std::collections::{HashMap, HashSet};

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
    let mut items = Vec::new();

    let sources = calendar::list_sources(&state.db, workspace_id).await?;
    let visible_sources = sources
        .iter()
        .filter(|source| source.is_visible)
        .map(|source| source.id.as_str())
        .collect::<HashSet<_>>();
    let source_colors = sources
        .iter()
        .map(|source| (source.id.clone(), source.color_hex.clone()))
        .collect::<HashMap<_, _>>();
    for event in
        calendar::list_events_between(&state.db, workspace_id, &rfc3339(from), &rfc3339(to), 1000)
            .await?
    {
        if let Some(source_id) = event.source_id.as_deref()
            && !visible_sources.contains(source_id)
        {
            continue;
        }
        let start = parse_bound(&event.start_at, "start_at")?;
        let end = parse_bound(&event.end_at, "end_at")?;
        items.push(AgendaItem {
            id: event.id.clone(),
            kind: "event".into(),
            title: event.title.clone(),
            subtitle: event.location.clone().or_else(|| event.description.clone()),
            start: rfc3339(start),
            end: Some(rfc3339(end)),
            all_day: event.all_day,
            status: status_for(start, end, now),
            source: sources
                .iter()
                .find(|source| Some(&source.id) == event.source_id.as_ref())
                .map(|source| source.provider.clone())
                .unwrap_or_else(|| "LOCAL".into()),
            color: event.color_hex.clone().or_else(|| {
                event
                    .source_id
                    .as_ref()
                    .and_then(|id| source_colors.get(id).cloned())
            }),
            href: None,
            editable: true,
        });
    }

    for schedule in schedules::list(&state.db, workspace_id).await? {
        if !schedule.is_active {
            continue;
        }
        if schedule.trigger_type == "CRON" {
            let mut cursor = from - Duration::seconds(1);
            for _ in 0..60 {
                let Ok(next) = cron_engine::next_occurrence(
                    &schedule.cron_expression,
                    &schedule.timezone,
                    cursor,
                ) else {
                    break;
                };
                if next > to {
                    break;
                }
                let end = next + Duration::minutes(1);
                items.push(AgendaItem {
                    id: format!("routine:{}:{}", schedule.id, rfc3339(next)),
                    kind: "routine".into(),
                    title: schedule.name.clone(),
                    subtitle: Some(format!(
                        "{} · {}",
                        schedule.trigger_type, schedule.cron_expression
                    )),
                    start: rfc3339(next),
                    end: Some(rfc3339(end)),
                    all_day: false,
                    status: status_for(next, end, now),
                    source: "ROUTINE".into(),
                    color: workspace_color.clone(),
                    href: Some(format!("/routines?schedule={}", schedule.id)),
                    editable: false,
                });
                cursor = next;
            }
        } else if let Some(next) = schedule
            .next_run_at
            .as_deref()
            .and_then(|value| parse_bound(value, "next_run_at").ok())
            && next >= from
            && next <= to
        {
            let end = next + Duration::minutes(1);
            items.push(AgendaItem {
                id: format!("routine:{}:{}", schedule.id, rfc3339(next)),
                kind: "routine".into(),
                title: schedule.name.clone(),
                subtitle: Some(schedule.trigger_type.clone()),
                start: rfc3339(next),
                end: Some(rfc3339(end)),
                all_day: false,
                status: status_for(next, end, now),
                source: "ROUTINE".into(),
                color: workspace_color.clone(),
                href: Some(format!("/routines?schedule={}", schedule.id)),
                editable: false,
            });
        }
    }

    let mut recipe_names = HashMap::new();
    for recipe in store::list(&state.db, workspace_id).await? {
        recipe_names.insert(recipe.id.clone(), recipe.name.clone());
        if !recipe.is_enabled {
            continue;
        }
        let Ok(yaml) = store::yaml_for(&state.db, workspace_id, &recipe.id).await else {
            continue;
        };
        let Ok(parsed) = parser::parse(&yaml) else {
            continue;
        };
        for trigger in parsed.triggers() {
            let Some(cron) = CronTrigger::from_trigger(trigger) else {
                continue;
            };
            let mut cursor = from - Duration::seconds(1);
            for _ in 0..60 {
                let Ok(next) = cron.next_occurrence(cursor) else {
                    break;
                };
                if next > to {
                    break;
                }
                let end = next + Duration::minutes(1);
                items.push(AgendaItem {
                    id: format!("recipe:{}:{}", recipe.id, rfc3339(next)),
                    kind: "recipe".into(),
                    title: recipe.name.clone(),
                    subtitle: Some("gatilho CRON".into()),
                    start: rfc3339(next),
                    end: Some(rfc3339(end)),
                    all_day: false,
                    status: status_for(next, end, now),
                    source: "RECIPE".into(),
                    color: workspace_color.clone(),
                    href: Some(format!("/recipes?recipe={}", recipe.id)),
                    editable: false,
                });
                cursor = next;
            }
        }
    }

    for run in runs::list_with_limit(&state.db, workspace_id, None, 500).await? {
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
        items.push(AgendaItem {
            id: run.id.clone(),
            kind: "run".into(),
            title: run
                .task_title
                .clone()
                .unwrap_or_else(|| format!("Execução {}", &run.id[..8.min(run.id.len())])),
            subtitle: Some(format!("@{} · {}", run.agent_name, run.model_id)),
            start: rfc3339(start),
            end: Some(rfc3339(end)),
            all_day: false,
            status: status_for(start, end, now),
            source: "EXECUTION".into(),
            color: Some("#10b981".into()),
            href: Some(format!("/runs/{}", run.id)),
            editable: false,
        });
    }

    for run in store::list_runs(&state.db, workspace_id, 500).await? {
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
        items.push(AgendaItem {
            id: run.id.clone(),
            kind: "recipe_run".into(),
            title: recipe_names
                .get(&run.recipe_id)
                .cloned()
                .unwrap_or_else(|| run.recipe_id.clone()),
            subtitle: Some(format!("receita · {}", run.status)),
            start: rfc3339(start),
            end: Some(rfc3339(end)),
            all_day: false,
            status: status_for(start, end, now),
            source: "EXECUTION".into(),
            color: Some("#8b5cf6".into()),
            href: Some(format!("/recipes?recipe={}", run.recipe_id)),
            editable: false,
        });
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
        items.push(AgendaItem {
            id: job.id.clone(),
            kind: "queued".into(),
            title: job
                .task_title
                .clone()
                .unwrap_or_else(|| "Execução agendada".into()),
            subtitle: Some(format!("@{} · fila {}", job.resolved_agent, job.status)),
            start: rfc3339(scheduled),
            end: Some(rfc3339(end)),
            all_day: false,
            status: status_for(scheduled, end, now),
            source: "EXECUTION".into(),
            color: Some("#f59e0b".into()),
            href: Some("/kanban".into()),
            editable: false,
        });
    }

    Ok(items)
}
