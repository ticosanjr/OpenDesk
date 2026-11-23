use super::super::{AgendaItem, parse_bound, rfc3339, status_for};
use super::items::{execution_color, missed_status};
use crate::{
    db::{calendar, missed_runs::MissedRun, runs::TaskRun, schedules},
    error::AppResult,
    queue_scheduler::cron_engine,
    recipes::{
        cron::CronTrigger,
        parser,
        store::{self, RecipeRun},
    },
};
use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

pub(super) struct RecipeIndex {
    pub(super) names: HashMap<String, String>,
    pub(super) enabled: HashSet<String>,
}

fn started_in(value: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> bool {
    parse_bound(value, "started_at")
        .ok()
        .is_some_and(|start| start >= from && start < to)
}

fn same_moment(value: &str, moment: DateTime<Utc>) -> bool {
    parse_bound(value, "scheduled_for").ok() == Some(moment)
}

pub(super) async fn collect_events(
    pool: &SqlitePool,
    workspace_id: &str,
    sources: &[calendar::CalendarSource],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    now: DateTime<Utc>,
    items: &mut Vec<AgendaItem>,
) -> AppResult<()> {
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
        calendar::list_events_between(pool, workspace_id, &rfc3339(from), &rfc3339(to), 1000)
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
    Ok(())
}

pub(super) struct AgendaQuery<'a> {
    pub pool: &'a SqlitePool,
    pub workspace_id: &'a str,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub now: DateTime<Utc>,
    pub workspace_color: Option<String>,
    pub all_runs: &'a [TaskRun],
}

pub(super) async fn collect_routines(
    query: &AgendaQuery<'_>,
    missed: &[MissedRun],
    used_runs: &mut HashSet<String>,
    items: &mut Vec<AgendaItem>,
) -> AppResult<()> {
    let AgendaQuery {
        pool,
        workspace_id,
        from,
        to,
        now,
        workspace_color,
        all_runs,
    } = query;
    let (from, to, now) = (*from, *to, *now);
    for schedule in schedules::list(pool, workspace_id).await? {
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
    Ok(())
}

pub(super) async fn collect_recipes(
    query: &AgendaQuery<'_>,
    all_recipe_runs: &[RecipeRun],
    used_runs: &mut HashSet<String>,
    used_recipe_runs: &mut HashSet<String>,
    items: &mut Vec<AgendaItem>,
) -> AppResult<RecipeIndex> {
    let AgendaQuery {
        pool,
        workspace_id,
        from,
        to,
        now,
        workspace_color,
        all_runs,
    } = query;
    let (from, to, now) = (*from, *to, *now);
    let recipes = store::list(pool, workspace_id).await?;
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
        let Ok(yaml) = store::yaml_for(pool, workspace_id, &recipe.id).await else {
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
    Ok(RecipeIndex {
        names: recipe_names,
        enabled: enabled_recipe_ids,
    })
}
