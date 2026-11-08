use crate::{
    app_state::AppState,
    db,
    error::AppResult,
    events::OpenDeskWsEvent,
    recipes::{engine, parser, store},
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct RecipeRuntime {
    seen: Mutex<HashMap<String, HashSet<PathBuf>>>,
}

pub fn start(state: Arc<AppState>) -> Arc<RecipeRuntime> {
    let runtime = Arc::new(RecipeRuntime::default());
    let state_clone = state.clone();
    tokio::spawn(async move {
        let _ = sync_all(&state_clone).await;
        let mut receiver = state_clone.events.subscribe();
        loop {
            if let Ok(envelope) = receiver.recv().await
                && let OpenDeskWsEvent::FsSyncTriggered {
                    workspace_id, path, ..
                } = envelope.event
                && path.contains("/recipes/")
            {
                let _ = sync_workspace(&state_clone, &workspace_id).await;
            }
        }
    });
    let runtime_clone = runtime.clone();
    tokio::spawn(async move {
        loop {
            if let Err(error) = scan_folder_triggers(&state, &runtime_clone).await {
                tracing::warn!(error = %error, "recipe folder scan failed");
            }
            if let Err(error) = scan_cron_triggers(&state).await {
                tracing::warn!(error = %error, "recipe cron scan failed");
            }
            if let Err(error) = scan_calendar_triggers(&state).await {
                tracing::warn!(error = %error, "recipe calendar scan failed");
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
    runtime
}

pub async fn sync_all(state: &Arc<AppState>) -> AppResult<()> {
    for workspace in db::workspaces::list(&state.db).await? {
        sync_workspace(state, &workspace.id).await?;
    }
    Ok(())
}

pub async fn sync_workspace(state: &Arc<AppState>, workspace_id: &str) -> AppResult<()> {
    let workspace = db::workspaces::get(&state.db, workspace_id).await?;
    store::sync_workspace(
        &state.db,
        workspace_id,
        std::path::Path::new(&workspace.root_path),
    )
    .await?;
    crate::api::recipes::sync_all_recipe_trigger_sources(state).await?;
    Ok(())
}

async fn scan_folder_triggers(
    state: &Arc<AppState>,
    runtime: &Arc<RecipeRuntime>,
) -> AppResult<()> {
    for index in store::list_all(&state.db).await? {
        if !index.is_enabled {
            continue;
        }
        let Ok(yaml) = store::yaml_for(&state.db, &index.id).await else {
            continue;
        };
        let Ok(recipe) = parser::parse(&yaml) else {
            continue;
        };
        let folder_triggers = recipe
            .triggers()
            .into_iter()
            .filter(|trigger| {
                trigger.trigger_type == "FOLDER_WATCH" || trigger.trigger_type == "RSYNC_FOLDER"
            })
            .cloned()
            .collect::<Vec<_>>();
        if folder_triggers.is_empty() {
            continue;
        }
        for folder_trigger in folder_triggers {
            let Some(path) = folder_trigger
                .config
                .get("path")
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            let workspace = db::workspaces::get(&state.db, &index.workspace_id).await?;
            let dir = PathBuf::from(&workspace.root_path).join(path);
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let file = entry.path();
                if !file.is_file() {
                    continue;
                }
                let mut seen = runtime.seen.lock().await;
                let set = seen.entry(format!("{}:{path}", recipe.id)).or_default();
                if !set.insert(file.clone()) {
                    continue;
                }
                drop(seen);
                let trigger = serde_json::json!({
                    "file_path": file.to_string_lossy(),
                    "file_name": file.file_name().and_then(|value| value.to_str()).unwrap_or("arquivo"),
                    "recipe_id": recipe.id,
                });
                let _ = engine::start(state.clone(), recipe.clone(), trigger, false).await;
            }
        }
    }
    Ok(())
}

async fn scan_calendar_triggers(state: &Arc<AppState>) -> AppResult<()> {
    let now = chrono::Utc::now();
    let events = db::calendar::list_events_starting_between(
        &state.db,
        &(now - chrono::Duration::days(1)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        &(now + chrono::Duration::days(8)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    )
    .await?;
    if events.is_empty() {
        return Ok(());
    }

    for index in store::list_all(&state.db).await? {
        if !index.is_enabled {
            continue;
        }
        let Ok(yaml) = store::yaml_for(&state.db, &index.id).await else {
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
        if triggers.is_empty() {
            continue;
        }
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
                if !calendar_id.is_empty()
                    && event.source_id.as_deref() != Some(calendar_id)
                    && !sources.iter().any(|source| {
                        source.id == calendar_id
                            && Some(source.external_id.as_deref())
                                == Some(event.external_id.as_deref())
                    })
                    && !sources.iter().any(|source| {
                        source.external_id.as_deref() == Some(calendar_id)
                            && event.source_id.as_deref() == Some(source.id.as_str())
                    })
                {
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
                if let Err(error) =
                    engine::start(state.clone(), recipe.clone(), trigger, false).await
                {
                    tracing::warn!(recipe = %recipe.id, error = %error, "calendar recipe trigger failed");
                }
            }
        }
    }
    Ok(())
}

async fn scan_cron_triggers(state: &Arc<AppState>) -> AppResult<()> {
    let now = chrono::Utc::now();
    for index in store::list_all(&state.db).await? {
        if !index.is_enabled {
            continue;
        }
        let Ok(yaml) = store::yaml_for(&state.db, &index.id).await else {
            continue;
        };
        let Ok(recipe) = parser::parse(&yaml) else {
            continue;
        };
        let Some(cron_trigger) = recipe
            .triggers()
            .into_iter()
            .find(|trigger| trigger.trigger_type == "CRON")
        else {
            continue;
        };
        let expression = cron_trigger
            .config
            .get("cron")
            .or_else(|| cron_trigger.config.get("cron_expression"))
            .or_else(|| cron_trigger.config.get("schedule"))
            .and_then(|value| value.as_str());
        let Some(expression) = expression else {
            continue;
        };
        let timezone = cron_trigger
            .config
            .get("timezone")
            .and_then(|value| value.as_str())
            .unwrap_or("UTC");
        let after = index
            .last_run_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc))
            .unwrap_or_else(|| now - chrono::Duration::minutes(1));
        let next =
            crate::queue_scheduler::cron_engine::next_occurrence(expression, timezone, after);
        if let Ok(next) = next
            && next <= now
        {
            let trigger = serde_json::json!({
                "cron": true,
                "scheduled_for": next.to_rfc3339(),
                "recipe_id": recipe.id,
            });
            let _ = engine::start(state.clone(), recipe.clone(), trigger, false).await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn calendar_trigger_fires_once_per_event() {
        let ctx = crate::test_support::context().await;
        let recipes = ctx.workspace_root.join(".opencode/recipes");
        std::fs::create_dir_all(&recipes).expect("recipes");
        let source = db::calendar::ensure_local_source(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("source");
        std::fs::write(
            recipes.join("receita-calendario.yaml"),
            format!(
                r#"id: receita-calendario
name: Receita Calendário
enabled: true
triggers:
  - type: CALENDAR_EVENT
    config:
      calendar_id: {source_id}
      match: "Reunião"
      lead_minutes: 0
      lookahead_minutes: 10
flow:
  - type: step
    id: passo-1
    name: Passo
    mode: build
    agent: build
    prompt: "Resuma {{{{trigger.calendar_event.title}}}}"
"#,
                source_id = source.id
            ),
        )
        .expect("recipe");
        store::sync_workspace(&ctx.state.db, &ctx.workspace_id, &ctx.workspace_root)
            .await
            .expect("sync");

        let now = chrono::Utc::now();
        db::calendar::create_event(
            &ctx.state.db,
            &ctx.workspace_id,
            &source.id,
            db::calendar::CreateEventInput {
                source_id: None,
                title: "Reunião de planejamento".into(),
                description: None,
                location: None,
                start_at: now.to_rfc3339(),
                end_at: (now + chrono::Duration::hours(1)).to_rfc3339(),
                all_day: false,
                timezone: Some("UTC".into()),
                status: None,
                color_hex: None,
            },
            &now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            &(now + chrono::Duration::hours(1)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "UTC",
            "CONFIRMED",
        )
        .await
        .expect("event");

        scan_calendar_triggers(&ctx.state)
            .await
            .expect("first scan");
        scan_calendar_triggers(&ctx.state)
            .await
            .expect("second scan");

        let runs: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM recipe_runs WHERE recipe_id = 'receita-calendario'",
        )
        .fetch_one(&ctx.state.db)
        .await
        .expect("count");
        assert_eq!(runs, 1);
    }
}
