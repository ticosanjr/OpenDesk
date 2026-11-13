use crate::{
    app_state::AppState,
    db,
    error::AppResult,
    events::OpenDeskWsEvent,
    recipes::{cron::CronTrigger, engine, parser, store},
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex;

pub struct RecipeRuntime {
    seen: Mutex<HashMap<String, HashSet<PathBuf>>>,
    pending: Mutex<HashMap<PathBuf, (u64, i64, Instant)>>,
    retry_at: Mutex<HashMap<PathBuf, Instant>>,
    run_checks: Mutex<HashMap<PathBuf, (String, String, u64)>>,
    baselines: Mutex<HashMap<String, SystemTime>>,
    started_at: SystemTime,
}

impl Default for RecipeRuntime {
    fn default() -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            retry_at: Mutex::new(HashMap::new()),
            run_checks: Mutex::new(HashMap::new()),
            baselines: Mutex::new(HashMap::new()),
            started_at: SystemTime::now(),
        }
    }
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

#[derive(Debug, Clone)]
struct FolderTriggerOptions {
    settle_secs: u64,
    process_empty: bool,
    process_existing: bool,
    retry_minutes: u64,
    ignore_patterns: Vec<String>,
}

impl FolderTriggerOptions {
    fn from_config(config: &serde_json::Value) -> Self {
        let settle_secs = config
            .get("settle_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(2)
            .clamp(0, 300);
        let process_empty = config
            .get("process_empty")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let process_existing = config
            .get("process_existing")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let retry_minutes = config
            .get("retry_minutes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            .clamp(1, 1440);
        let ignore_patterns = config
            .get("ignore_patterns")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            settle_secs,
            process_empty,
            process_existing,
            retry_minutes,
            ignore_patterns,
        }
    }

    fn ignores(&self, name: &str) -> bool {
        name.starts_with('.')
            || name.ends_with(".tmp")
            || name.ends_with(".part")
            || name.ends_with(".crdownload")
            || self.ignore_patterns.iter().any(|pattern| {
                !pattern.trim().is_empty()
                    && name.to_lowercase().contains(&pattern.trim().to_lowercase())
            })
    }
}

async fn release_failed_runs(state: &Arc<AppState>, runtime: &Arc<RecipeRuntime>) {
    let checks = runtime
        .run_checks
        .lock()
        .await
        .iter()
        .map(|(path, value)| (path.clone(), value.clone()))
        .collect::<Vec<_>>();
    for (path, (run_id, seen_key, retry_minutes)) in checks {
        let status = sqlx::query_scalar::<_, String>("SELECT status FROM recipe_runs WHERE id = ?")
            .bind(&run_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();
        let Some(status) = status else {
            runtime.run_checks.lock().await.remove(&path);
            continue;
        };
        if matches!(status.as_str(), "FAILED" | "REJECTED" | "TIMED_OUT") {
            runtime.run_checks.lock().await.remove(&path);
            if let Some(set) = runtime.seen.lock().await.get_mut(&seen_key) {
                set.remove(&path);
            }
            runtime.retry_at.lock().await.insert(
                path,
                Instant::now() + Duration::from_secs(retry_minutes.max(1) * 60),
            );
        } else if status == "COMPLETED" {
            runtime.run_checks.lock().await.remove(&path);
        }
    }
}

async fn scan_folder_triggers(
    state: &Arc<AppState>,
    runtime: &Arc<RecipeRuntime>,
) -> AppResult<()> {
    release_failed_runs(state, runtime).await;
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
            let options = FolderTriggerOptions::from_config(&folder_trigger.config);
            let settle_secs = options.settle_secs;
            let process_empty = options.process_empty;
            let process_existing = options.process_existing;
            let retry_minutes = options.retry_minutes;
            let baseline_key = format!("{}:{path}", recipe.id);
            let baseline = if process_existing {
                runtime.started_at
            } else {
                *runtime
                    .baselines
                    .lock()
                    .await
                    .entry(baseline_key)
                    .or_insert_with(SystemTime::now)
            };

            for entry in entries.flatten() {
                let file = entry.path();
                if !file.is_file() {
                    continue;
                }
                let Ok(metadata) = std::fs::metadata(&file) else {
                    continue;
                };
                let name = file
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("arquivo");
                if options.ignores(name) {
                    continue;
                }
                if metadata.len() == 0 && !process_empty {
                    continue;
                }
                if !process_existing
                    && metadata
                        .modified()
                        .map(|value| value < baseline)
                        .unwrap_or(false)
                {
                    let mut seen = runtime.seen.lock().await;
                    seen.entry(format!("{}:{path}", recipe.id))
                        .or_default()
                        .insert(file.clone());
                    continue;
                }
                {
                    let retry = runtime.retry_at.lock().await;
                    if retry.get(&file).is_some_and(|at| *at > Instant::now()) {
                        continue;
                    }
                }
                if settle_secs > 0 {
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|value| value.as_secs() as i64)
                        .unwrap_or(0);
                    let mut pending = runtime.pending.lock().await;
                    match pending.get(&file) {
                        Some((size, stamp, since))
                            if *size == metadata.len()
                                && *stamp == modified
                                && since.elapsed() >= Duration::from_secs(settle_secs) =>
                        {
                            pending.remove(&file);
                        }
                        Some((size, stamp, _)) if *size == metadata.len() && *stamp == modified => {
                            continue;
                        }
                        _ => {
                            pending
                                .insert(file.clone(), (metadata.len(), modified, Instant::now()));
                            continue;
                        }
                    }
                }
                let mut seen = runtime.seen.lock().await;
                let set = seen.entry(format!("{}:{path}", recipe.id)).or_default();
                if !set.insert(file.clone()) {
                    continue;
                }
                drop(seen);
                let trigger = serde_json::json!({
                    "file_path": file.to_string_lossy(),
                    "file_name": name,
                    "file_size": metadata.len(),
                    "modified_at": metadata.modified().ok().map(|value| chrono::DateTime::<chrono::Utc>::from(value).to_rfc3339()),
                    "trigger_type": folder_trigger.trigger_type,
                    "trigger_path": path,
                    "recipe_id": recipe.id,
                });
                match engine::start(
                    state.clone(),
                    &index.workspace_id,
                    recipe.clone(),
                    trigger,
                    false,
                )
                .await
                {
                    Ok(run) => {
                        runtime.run_checks.lock().await.insert(
                            file.clone(),
                            (run.id, format!("{}:{path}", recipe.id), retry_minutes),
                        );
                    }
                    Err(error) => {
                        tracing::warn!(path = %file.display(), error = %error, "recipe folder trigger failed; will retry");
                        let mut seen = runtime.seen.lock().await;
                        if let Some(set) = seen.get_mut(&format!("{}:{path}", recipe.id)) {
                            set.remove(&file);
                        }
                        let mut retry = runtime.retry_at.lock().await;
                        retry.insert(
                            file.clone(),
                            Instant::now() + Duration::from_secs(retry_minutes * 60),
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

async fn scan_calendar_triggers(state: &Arc<AppState>) -> AppResult<()> {
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
                    state.clone(),
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

async fn scan_cron_triggers(state: &Arc<AppState>) -> AppResult<()> {
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
                state.clone(),
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

#[cfg(test)]
mod folder_option_tests {
    use super::*;

    #[test]
    fn folder_options_default_and_custom_values() {
        let defaults = FolderTriggerOptions::from_config(&serde_json::json!({}));
        assert_eq!(defaults.settle_secs, 2);
        assert!(!defaults.process_empty);
        assert!(defaults.process_existing);
        assert_eq!(defaults.retry_minutes, 1);

        let custom = FolderTriggerOptions::from_config(&serde_json::json!({
            "settle_secs": 500,
            "process_empty": true,
            "process_existing": false,
            "retry_minutes": 0,
            "ignore_patterns": ["backup", ""]
        }));
        assert_eq!(custom.settle_secs, 300);
        assert!(custom.process_empty);
        assert!(!custom.process_existing);
        assert_eq!(custom.retry_minutes, 1);
    }

    #[tokio::test]
    async fn failed_recipe_runs_are_released_for_retry() {
        let ctx = crate::test_support::context().await;
        sqlx::query(
            "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
             VALUES ('retry-recipe', ?, 'Retry', '/tmp/retry.yaml', 'FOLDER_WATCH')",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("index");
        sqlx::query(
            "INSERT INTO recipe_runs (id, recipe_id, workspace_id, status, trigger_context_json, output_dir_path)
             VALUES ('retry-run', 'retry-recipe', ?, 'FAILED', '{}', '/tmp/out')",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("run");

        let runtime = Arc::new(RecipeRuntime::default());
        let path = PathBuf::from("/tmp/input/relatorio.pdf");
        runtime.run_checks.lock().await.insert(
            path.clone(),
            ("retry-run".into(), "retry-recipe:input".into(), 1),
        );
        runtime
            .seen
            .lock()
            .await
            .entry("retry-recipe:input".into())
            .or_default()
            .insert(path.clone());

        release_failed_runs(&ctx.state, &runtime).await;

        assert!(runtime.retry_at.lock().await.contains_key(&path));
        assert!(
            !runtime
                .seen
                .lock()
                .await
                .get("retry-recipe:input")
                .expect("seen")
                .contains(&path)
        );
        assert!(runtime.run_checks.lock().await.is_empty());
    }

    #[test]
    fn folder_options_ignore_patterns_and_temporary_files() {
        let options = FolderTriggerOptions::from_config(&serde_json::json!({
            "ignore_patterns": ["backup", "rascunho"]
        }));
        assert!(options.ignores(".hidden"));
        assert!(options.ignores("a.tmp"));
        assert!(options.ignores("Backup-2026.pdf"));
        assert!(options.ignores("meu-rascunho.pdf"));
        assert!(!options.ignores("relatorio.pdf"));
    }
}
