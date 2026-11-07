use crate::{
    app_state::AppState,
    db::{self, tasks},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    recipes::{
        hitl::{self, HitlOutcome, HitlResolution},
        parser::{self, FlowItem, Recipe, Step},
        sinks, store,
    },
};
use chrono::Utc;
use serde_json::json;
use std::{sync::Arc, time::Duration};

pub async fn start(
    state: Arc<AppState>,
    recipe: Recipe,
    trigger: serde_json::Value,
    dry_run: bool,
) -> AppResult<store::RecipeRun> {
    let index = store::get(&state.db, &recipe.id).await?;
    if !index.is_enabled || !recipe.enabled {
        return Err(AppError::BadRequest("receita desabilitada".into()));
    }
    let workspace = db::workspaces::get(&state.db, &index.workspace_id).await?;
    let output = crate::dsh_router::output_directory(
        std::path::Path::new(&workspace.root_path),
        &format!(
            "{} {}",
            recipe.name,
            uuid::Uuid::now_v7()
                .simple()
                .to_string()
                .get(..8)
                .unwrap_or("run")
        ),
    );
    std::fs::create_dir_all(&output)?;
    let output = output.to_string_lossy().to_string();

    if dry_run {
        let run = store::create_run(
            &state.db,
            &recipe,
            &index.workspace_id,
            None,
            &trigger,
            &output,
            true,
        )
        .await?;
        let state_clone = state.clone();
        let recipe_clone = recipe.clone();
        let run_clone = run.clone();
        tokio::spawn(async move {
            for step in recipe_clone.steps() {
                let _ = store::update_run(
                    &state_clone.db,
                    &run_clone.id,
                    "RUNNING_STEP",
                    Some(&step.id),
                    None,
                    None,
                    false,
                )
                .await;
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            let _ = store::update_run(
                &state_clone.db,
                &run_clone.id,
                "COMPLETED",
                None,
                Some(&json!({"dry_run": true})),
                None,
                true,
            )
            .await;
            state_clone
                .events
                .publish(OpenDeskWsEvent::RecipeRunUpdated {
                    run_id: run_clone.id,
                    recipe_id: recipe_clone.id,
                    workspace_id: index.workspace_id,
                    status: "COMPLETED".into(),
                });
        });
        return Ok(run);
    }

    let task = tasks::create(
        &state.db,
        &index.workspace_id,
        tasks::CreateTaskInput {
            title: format!("{} · receita", recipe.name),
            description: recipe
                .description
                .clone()
                .unwrap_or_else(|| format!("Receita {}", recipe.id)),
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: None,
            preferred_model: None,
            effort_level: None,
            notify_email: None,
            export_to_gdocs: false,
            tags: vec![format!("recipe:{}", recipe.id)],
        },
    )
    .await?;
    tasks::set_output_dir(&state.db, &task.id, &output).await?;
    let run = store::create_run(
        &state.db,
        &recipe,
        &index.workspace_id,
        Some(&task.id),
        &trigger,
        &output,
        false,
    )
    .await?;
    let state_clone = state.clone();
    let run_clone = run.clone();
    let recipe_clone = recipe.clone();
    tokio::spawn(async move {
        if let Err(error) = execute(&state_clone, &recipe_clone, &run_clone, &trigger).await {
            tracing::warn!(run = %run_clone.id, error = %error, "recipe run failed");
            let _ = store::update_run(
                &state_clone.db,
                &run_clone.id,
                "FAILED",
                None,
                None,
                Some(&format!("{error:#}")),
                true,
            )
            .await;
            if let Some(task_id) = &run_clone.task_id {
                let _ = tasks::set_phase_status(
                    &state_clone.db,
                    task_id,
                    Some("failed"),
                    Some("BLOCKED"),
                )
                .await;
                let _ = tasks::set_blocked_reason(
                    &state_clone.db,
                    task_id,
                    Some(&format!("{error:#}")),
                )
                .await;
            }
            mark_routine(&state_clone, &trigger, "FAILED").await;
            state_clone
                .events
                .publish(OpenDeskWsEvent::RecipeRunUpdated {
                    run_id: run_clone.id,
                    recipe_id: recipe_clone.id,
                    workspace_id: run_clone.workspace_id,
                    status: "FAILED".into(),
                });
        }
    });
    Ok(run)
}

async fn execute(
    state: &Arc<AppState>,
    recipe: &Recipe,
    run: &store::RecipeRun,
    trigger: &serde_json::Value,
) -> AppResult<()> {
    let task_id = run
        .task_id
        .clone()
        .ok_or_else(|| AppError::BadRequest("recipe run sem task".into()))?;
    let run_vars = json!({
        "id": run.id,
        "output_dir": run.output_dir_path,
        "date": Utc::now().format("%Y-%m-%d").to_string(),
    });
    let mut outputs = serde_json::json!({});
    let mut session_id: Option<String> = None;
    let routine_id = trigger
        .get("routine_id")
        .or_else(|| trigger.get("schedule_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mut feedback_by_step: std::collections::HashMap<String, String> = Default::default();
    let mut review_rounds = 0usize;
    let flow = recipe.flow.clone();
    let mut index = 0usize;

    while index < flow.len() {
        match &flow[index] {
            FlowItem::Step(step) => {
                let feedback = feedback_by_step.remove(&step.id);
                execute_step(
                    state,
                    run,
                    &task_id,
                    step,
                    trigger,
                    &run_vars,
                    &mut outputs,
                    &mut session_id,
                    feedback,
                    routine_id.as_deref(),
                )
                .await?;
                index += 1;
            }
            FlowItem::Hitl(checkpoint) => {
                let mut checkpoint = checkpoint.clone();
                checkpoint.id = parser::checkpoint_id(&checkpoint);
                let current_step = flow[..index].iter().rev().find_map(|item| match item {
                    FlowItem::Step(step) => Some(step.id.clone()),
                    _ => None,
                });
                store::update_run(
                    &state.db,
                    &run.id,
                    "WAITING_HITL",
                    current_step.as_deref(),
                    Some(&outputs),
                    None,
                    false,
                )
                .await?;
                let _ = tasks::move_to(&state.db, &task_id, "IN_REVIEW", None).await;
                let _ = tasks::set_phase_status(
                    &state.db,
                    &task_id,
                    Some("waiting_hitl"),
                    Some("IN_REVIEW"),
                )
                .await;
                let message =
                    parser::render(&checkpoint.prompt_template, trigger, &run_vars, &outputs)?;
                let attachments = checkpoint
                    .attach_files
                    .iter()
                    .map(|path| parser::render(path, trigger, &run_vars, &outputs))
                    .collect::<AppResult<Vec<_>>>()?;
                let token = hitl::request(
                    state,
                    run,
                    &checkpoint,
                    &recipe.name,
                    &message,
                    &attachments,
                )
                .await?;
                state.events.publish(OpenDeskWsEvent::HitlRequested {
                    token: token.clone(),
                    run_id: run.id.clone(),
                    workspace_id: run.workspace_id.clone(),
                    checkpoint_id: checkpoint.id.clone(),
                    interaction_type: checkpoint.interaction_type.clone(),
                });
                let outcome = hitl::wait(state, &token, checkpoint.timeout_minutes).await?;
                match outcome {
                    HitlOutcome::Resolved(HitlResolution::Approved) => {
                        state.events.publish(OpenDeskWsEvent::HitlResolved {
                            token,
                            status: "APPROVED".into(),
                        });
                        index += 1;
                    }
                    HitlOutcome::Resolved(HitlResolution::Rejected { reason }) => {
                        finish(
                            state,
                            run,
                            "REJECTED",
                            reason.as_deref().unwrap_or("rejeitado pelo usuário"),
                            routine_id.as_deref(),
                        )
                        .await?;
                        return Ok(());
                    }
                    HitlOutcome::Resolved(HitlResolution::FeedbackProvided {
                        feedback_text,
                        attached_files,
                    }) => {
                        if let Some(path) = attached_files.first() {
                            replace_attachment(path, &attachments)?;
                            index += 1;
                            continue;
                        }
                        review_rounds += 1;
                        if review_rounds > 5 {
                            finish(
                                state,
                                run,
                                "REJECTED",
                                "limite de revisões atingido",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        }
                        let Some((previous_index, previous_step)) = previous_step(&flow, index)
                        else {
                            finish(
                                state,
                                run,
                                "REJECTED",
                                "checkpoint sem step anterior para refazer",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        };
                        feedback_by_step.insert(previous_step.id.clone(), feedback_text);
                        index = previous_index;
                    }
                    HitlOutcome::Resolved(HitlResolution::ChoiceSelected {
                        option_value, ..
                    }) => {
                        review_rounds += 1;
                        if review_rounds > 5 {
                            finish(
                                state,
                                run,
                                "REJECTED",
                                "limite de revisões atingido",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        }
                        let Some((previous_index, previous_step)) = previous_step(&flow, index)
                        else {
                            finish(
                                state,
                                run,
                                "REJECTED",
                                "checkpoint sem step anterior para refazer",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        };
                        feedback_by_step.insert(
                            previous_step.id.clone(),
                            format!("Opção escolhida: {option_value}"),
                        );
                        index = previous_index;
                    }
                    HitlOutcome::Resolved(HitlResolution::ArtifactEdited { path }) => {
                        replace_attachment(&path, &attachments)?;
                        index += 1;
                    }
                    HitlOutcome::Expired => {
                        if checkpoint.on_timeout == "AUTO_APPROVE" {
                            index += 1;
                        } else {
                            finish(
                                state,
                                run,
                                "TIMED_OUT",
                                "timeout do checkpoint HITL",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        }
                    }
                }
            }
            FlowItem::Sink(sink) => {
                store::update_run(
                    &state.db,
                    &run.id,
                    "RUNNING_SINKS",
                    None,
                    Some(&outputs),
                    None,
                    false,
                )
                .await?;
                sinks::execute_one(state, sink, run, trigger, &outputs).await?;
                index += 1;
            }
        }
    }

    let _ = tasks::move_to(&state.db, &task_id, "DONE", None).await;
    let _ = tasks::set_phase_status(&state.db, &task_id, Some("done"), Some("DONE")).await;
    store::update_run(
        &state.db,
        &run.id,
        "COMPLETED",
        None,
        Some(&outputs),
        None,
        true,
    )
    .await?;
    mark_routine(state, trigger, "SUCCESS").await;
    state.events.publish(OpenDeskWsEvent::RecipeRunUpdated {
        run_id: run.id.clone(),
        recipe_id: recipe.id.clone(),
        workspace_id: run.workspace_id.clone(),
        status: "COMPLETED".into(),
    });
    Ok(())
}

fn previous_step(flow: &[FlowItem], index: usize) -> Option<(usize, &Step)> {
    flow[..index]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(position, item)| match item {
            FlowItem::Step(step) => Some((position, step)),
            _ => None,
        })
}

#[allow(clippy::too_many_arguments)]
async fn execute_step(
    state: &Arc<AppState>,
    run: &store::RecipeRun,
    task_id: &str,
    step: &Step,
    trigger: &serde_json::Value,
    run_vars: &serde_json::Value,
    outputs: &mut serde_json::Value,
    session_id: &mut Option<String>,
    feedback: Option<String>,
    routine_id: Option<&str>,
) -> AppResult<()> {
    store::update_run(
        &state.db,
        &run.id,
        "RUNNING_STEP",
        Some(&step.id),
        Some(outputs),
        None,
        false,
    )
    .await?;
    let prompt = parser::render(&step.prompt, trigger, run_vars, outputs)?;
    let prompt = match &feedback {
        Some(feedback) => format!(
            "{prompt}\n\n## Correção do usuário\n\n{feedback}\n\nRefaça o step considerando a correção."
        ),
        None => prompt,
    };
    tasks::update(
        &state.db,
        task_id,
        tasks::UpdateTaskInput {
            description: Some(prompt.clone()),
            ..Default::default()
        },
    )
    .await?;
    let started = Utc::now().to_rfc3339();
    let body = crate::api::tasks::DispatchBody {
        policy: None,
        manual_agent: step
            .agent
            .clone()
            .map(|value| value.trim_start_matches('@').to_string()),
        manual_model: step.model.clone(),
        manual_effort: step.effort.clone(),
        execution_mode: Some(mode_for_step(step)),
    };
    crate::api::tasks::dispatch_task_with_session(
        state,
        task_id,
        body,
        None,
        session_id.as_deref(),
    )
    .await?;
    let run_result = wait_for_run(state, &run.workspace_id, task_id, &started).await?;
    if let Some(routine_id) = routine_id {
        let _ = db::runs::set_schedule_id(&state.db, &run_result.id, routine_id).await;
    }
    *session_id = run_result.session_id.clone();
    let summary = run_result.summary_response.clone().unwrap_or_default();
    let artifacts = db::artifacts::list_by_run(&state.db, &run_result.id, 200)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|artifact| artifact.file_path)
        .collect::<Vec<_>>();
    outputs[&step.id] = json!({
        "run_id": run_result.id,
        "summary": summary,
        "artifacts": artifacts,
    });
    store::update_run(
        &state.db,
        &run.id,
        "RUNNING_STEP",
        Some(&step.id),
        Some(outputs),
        None,
        false,
    )
    .await?;
    Ok(())
}

async fn wait_for_run(
    state: &Arc<AppState>,
    workspace_id: &str,
    task_id: &str,
    started_after: &str,
) -> AppResult<db::runs::TaskRun> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2 * 60 * 60);
    loop {
        if let Ok(runs) = db::runs::list_with_limit(&state.db, workspace_id, Some(task_id), 5).await
            && let Some(run) = runs
                .into_iter()
                .find(|run| run.started_at.as_str() >= started_after)
            && matches!(run.status.as_str(), "SUCCESS" | "FAILED" | "CANCELLED")
        {
            return Ok(run);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::Internal(
                "timeout aguardando step da receita".into(),
            ));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn replace_attachment(source: &str, targets: &[String]) -> AppResult<()> {
    let target = targets
        .first()
        .ok_or_else(|| AppError::BadRequest("checkpoint sem arquivo de destino".into()))?;
    let bytes = std::fs::read(source)?;
    if let Some(parent) = std::path::Path::new(target).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(target, bytes)?;
    Ok(())
}

async fn finish(
    state: &Arc<AppState>,
    run: &store::RecipeRun,
    status: &str,
    message: &str,
    routine_id: Option<&str>,
) -> AppResult<()> {
    store::update_run(&state.db, &run.id, status, None, None, Some(message), true).await?;
    if let Some(task_id) = &run.task_id {
        let _ = tasks::set_phase_status(&state.db, task_id, Some("failed"), Some("BLOCKED")).await;
        let _ = tasks::set_blocked_reason(&state.db, task_id, Some(message)).await;
    }
    if let Some(routine_id) = routine_id {
        mark_routine_by_id(state, routine_id, status).await;
    }
    state.events.publish(OpenDeskWsEvent::RecipeRunUpdated {
        run_id: run.id.clone(),
        recipe_id: run.recipe_id.clone(),
        workspace_id: run.workspace_id.clone(),
        status: status.to_string(),
    });
    Ok(())
}

async fn mark_routine(state: &Arc<AppState>, trigger: &serde_json::Value, status: &str) {
    if let Some(routine_id) = trigger
        .get("routine_id")
        .or_else(|| trigger.get("schedule_id"))
        .and_then(serde_json::Value::as_str)
    {
        mark_routine_by_id(state, routine_id, status).await;
    }
}

async fn mark_routine_by_id(state: &Arc<AppState>, routine_id: &str, status: &str) {
    if let Ok(schedule) = crate::db::schedules::get(&state.db, routine_id).await {
        let _ = crate::db::schedules::mark_run(
            &state.db,
            &schedule.id,
            status,
            &Utc::now().to_rfc3339(),
        )
        .await;
        state.events.publish(OpenDeskWsEvent::ScheduleUpdated {
            schedule_id: schedule.id,
            workspace_id: schedule.workspace_id,
        });
    }
}

fn mode_for_step(step: &Step) -> String {
    match step.mode.as_str() {
        "plan" => "PLAN_ONLY".into(),
        _ => "BUILD_DIRECT".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::parser::Step;

    #[test]
    fn maps_step_mode() {
        assert_eq!(
            mode_for_step(&Step {
                id: "s".into(),
                name: "S".into(),
                mode: "plan".into(),
                agent: None,
                model: None,
                effort: None,
                description: None,
                prompt: "x".into(),
            }),
            "PLAN_ONLY"
        );
    }

    #[tokio::test]
    async fn dry_run_completes_without_dispatch() {
        use crate::paths::AppPaths;
        let temp = tempfile::TempDir::new().expect("temp");
        let root = temp.path();
        let paths = AppPaths {
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            db_path: root.join("data").join("opendesk.db"),
            backups_dir: root.join("backups"),
            artifacts_dir: root.join("artifacts"),
            run_logs_dir: root.join("runs"),
            daemon_lock_path: root.join("daemon.json"),
            api_lock_path: root.join("api.json"),
        };
        for dir in [
            &paths.data_dir,
            &paths.config_dir,
            &paths.cache_dir,
            &paths.log_dir,
            &paths.backups_dir,
            &paths.artifacts_dir,
            &paths.run_logs_dir,
        ] {
            std::fs::create_dir_all(dir).expect("dir");
        }
        let state = Arc::new(
            crate::app_state::AppState::initialize(paths)
                .await
                .expect("state"),
        );
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(root.to_string_lossy().to_string())
            .execute(&state.db)
            .await
            .expect("ws");
        let yaml = "id: dry
name: Dry\ntrigger:\n  type: MANUAL\n  config: {}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n";
        store::save_yaml(&state.db, "ws-1", "dry", yaml)
            .await
            .expect("save");
        let recipe = crate::recipes::parser::parse(yaml).expect("parse");
        let run = start(state.clone(), recipe, serde_json::json!({}), true)
            .await
            .expect("run");
        let mut status = run.status;
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            status = store::get_run(&state.db, &run.id)
                .await
                .expect("get")
                .status;
            if status == "COMPLETED" {
                break;
            }
        }
        assert_eq!(status, "COMPLETED");
    }

    #[test]
    fn replaces_attachment_safely_in_temp_dir() {
        let temp = tempfile::TempDir::new().expect("temp");
        let source = temp.path().join("new.md");
        let target = temp.path().join("out/final.md");
        std::fs::write(&source, "novo").expect("source");
        replace_attachment(
            &source.to_string_lossy(),
            &[target.to_string_lossy().to_string()],
        )
        .expect("replace");
        assert_eq!(std::fs::read_to_string(target).expect("target"), "novo");
    }
}
