use super::*;
use crate::db::missed_runs;
use std::collections::{HashMap, HashSet};

fn started_in(value: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> bool {
    parse_bound(value, "started_at")
        .ok()
        .is_some_and(|start| start >= from && start < to)
}

fn same_moment(value: &str, moment: DateTime<Utc>) -> bool {
    parse_bound(value, "scheduled_for").ok() == Some(moment)
}

fn execution_color(status: &str) -> Option<String> {
    match status.to_uppercase().as_str() {
        "SUCCESS" | "DONE" => Some("#10b981".into()),
        "FAILED" | "BLOCKED" => Some("#ef4444".into()),
        "RUNNING" | "ACTIVE" => Some("#3b82f6".into()),
        "QUEUED" | "PAUSED" | "MISSED" | "PENDING" | "WAITING_HITL" => Some("#f59e0b".into()),
        "CANCELLED" | "IGNORED" => Some("#71717a".into()),
        _ => None,
    }
}

fn missed_status(status: &str) -> &'static str {
    match status {
        "PENDING" => "MISSED",
        "QUEUED" => "QUEUED",
        "FAILED" => "FAILED",
        "CANCELLED" => "CANCELLED",
        _ => "MISSED",
    }
}

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
            execution_status: None,
            run_id: None,
            missed_id: None,
            executed: false,
            editable: true,
        });
    }

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

    for schedule in schedules::list(&state.db, workspace_id).await? {
        if !schedule.is_active {
            continue;
        }
        let schedule_href = if schedule.target_kind == "RECIPE" {
            schedule
                .recipe_id
                .as_deref()
                .map(|recipe_id| format!("/recipes?recipe={recipe_id}"))
        } else {
            Some(format!("/routines?schedule={}", schedule.id))
        };
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
                let window_end = cron_engine::next_occurrence(
                    &schedule.cron_expression,
                    &schedule.timezone,
                    next,
                )
                .unwrap_or(next + Duration::days(1));
                let matched_run = all_runs.iter().find(|run| {
                    run.schedule_id.as_deref() == Some(schedule.id.as_str())
                        && started_in(&run.started_at, next, window_end)
                });
                let missed_row = missed.iter().find(|item| {
                    item.schedule_id == schedule.id && same_moment(&item.scheduled_for, next)
                });
                let (execution_status, run_id, missed_id, href, color, executed) =
                    if let Some(run) = matched_run {
                        used_runs.insert(run.id.clone());
                        (
                            Some(run.status.clone()),
                            Some(run.id.clone()),
                            None,
                            Some(format!("/runs/{}", run.id)),
                            execution_color(&run.status),
                            true,
                        )
                    } else if let Some(item) = missed_row.filter(|item| item.status != "IGNORED") {
                        let code = missed_status(&item.status);
                        (
                            Some(code.to_string()),
                            item.run_id.clone(),
                            Some(item.id.clone()),
                            schedule_href.clone(),
                            execution_color(code),
                            false,
                        )
                    } else if next > now {
                        (
                            Some("PENDING".into()),
                            None,
                            None,
                            schedule_href.clone(),
                            workspace_color.clone(),
                            false,
                        )
                    } else {
                        cursor = next;
                        continue;
                    };
                let end = next + Duration::minutes(1);
                items.push(AgendaItem {
                    id: format!("routine:{}:{}", schedule.id, rfc3339(next)),
                    kind: if schedule.target_kind == "RECIPE" {
                        "recipe".into()
                    } else {
                        "routine".into()
                    },
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
                    color,
                    href,
                    execution_status,
                    run_id,
                    missed_id,
                    executed,
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
            let window_end = next + Duration::days(1);
            let matched_run = all_runs.iter().find(|run| {
                run.schedule_id.as_deref() == Some(schedule.id.as_str())
                    && started_in(&run.started_at, next, window_end)
            });
            let missed_row = missed.iter().find(|item| {
                item.schedule_id == schedule.id && same_moment(&item.scheduled_for, next)
            });
            let (execution_status, run_id, missed_id, href, color, executed) =
                if let Some(run) = matched_run {
                    used_runs.insert(run.id.clone());
                    (
                        Some(run.status.clone()),
                        Some(run.id.clone()),
                        None,
                        Some(format!("/runs/{}", run.id)),
                        execution_color(&run.status),
                        true,
                    )
                } else if let Some(item) = missed_row.filter(|item| item.status != "IGNORED") {
                    let code = missed_status(&item.status);
                    (
                        Some(code.to_string()),
                        item.run_id.clone(),
                        Some(item.id.clone()),
                        schedule_href.clone(),
                        execution_color(code),
                        false,
                    )
                } else if next > now {
                    (
                        Some("PENDING".into()),
                        None,
                        None,
                        schedule_href.clone(),
                        workspace_color.clone(),
                        false,
                    )
                } else {
                    continue;
                };
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
                color,
                href,
                execution_status,
                run_id,
                missed_id,
                executed,
                editable: false,
            });
        }
    }

    let recipes = store::list(&state.db, workspace_id).await?;
    let enabled_recipe_ids = recipes
        .iter()
        .filter(|recipe| recipe.is_enabled)
        .map(|recipe| recipe.id.clone())
        .collect::<HashSet<_>>();
    let mut recipe_names = HashMap::new();
    for recipe in &recipes {
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
                let window_end = cron
                    .next_occurrence(next)
                    .unwrap_or(next + Duration::days(1));
                let recipe_run = all_recipe_runs.iter().find(|run| {
                    run.recipe_id == recipe.id && started_in(&run.started_at, next, window_end)
                });
                let task_run =
                    recipe_run
                        .and_then(|run| run.task_id.as_deref())
                        .and_then(|task_id| {
                            all_runs.iter().find(|run| {
                                run.task_id.as_deref() == Some(task_id)
                                    && recipe_run.is_some_and(|recipe_run| {
                                        parse_bound(&run.started_at, "started_at")
                                            .ok()
                                            .zip(
                                                parse_bound(&recipe_run.started_at, "started_at")
                                                    .ok(),
                                            )
                                            .is_some_and(|(run_start, recipe_start)| {
                                                run_start >= recipe_start
                                            })
                                    })
                            })
                        });
                let execution_status = task_run
                    .map(|run| run.status.clone())
                    .or_else(|| recipe_run.map(|run| run.status.clone()));
                if execution_status.is_none() && next <= now {
                    cursor = next;
                    continue;
                }
                if let Some(run) = recipe_run {
                    used_recipe_runs.insert(run.id.clone());
                }
                if let Some(run) = task_run {
                    used_runs.insert(run.id.clone());
                }
                let href = task_run
                    .map(|run| format!("/runs/{}", run.id))
                    .or_else(|| {
                        recipe_run
                            .map(|run| format!("/recipes?recipe={}&run={}", recipe.id, run.id))
                    })
                    .unwrap_or_else(|| format!("/recipes?recipe={}", recipe.id));
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
                    color: execution_status
                        .as_deref()
                        .and_then(execution_color)
                        .or_else(|| workspace_color.clone()),
                    href: Some(href),
                    execution_status: execution_status.or_else(|| Some("PENDING".into())),
                    run_id: task_run
                        .map(|run| run.id.clone())
                        .or_else(|| recipe_run.map(|run| run.id.clone())),
                    missed_id: None,
                    executed: task_run.is_some() || recipe_run.is_some(),
                    editable: false,
                });
                cursor = next;
            }
        }
    }

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
            color: execution_color(&run.status),
            href: Some(format!("/runs/{}", run.id)),
            execution_status: Some(run.status.clone()),
            run_id: Some(run.id.clone()),
            missed_id: None,
            executed: true,
            editable: false,
        });
    }

    for run in &all_recipe_runs {
        if used_recipe_runs.contains(&run.id) || !enabled_recipe_ids.contains(&run.recipe_id) {
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
            color: execution_color(&run.status),
            href: Some(format!("/recipes?recipe={}&run={}", run.recipe_id, run.id)),
            execution_status: Some(run.status.clone()),
            run_id: Some(run.id.clone()),
            missed_id: None,
            executed: true,
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
            color: execution_color(&job.status),
            href: Some("/kanban".into()),
            execution_status: Some(job.status.clone()),
            run_id: None,
            missed_id: None,
            executed: false,
            editable: false,
        });
    }

    Ok(items)
}
