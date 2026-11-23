use crate::{
    context::RecipeContext,
    error::{AppError, AppResult},
    parser::Recipe,
    store,
};
use opendesk_storage::events::OpenDeskWsEvent;
use opendesk_storage::repos::tasks;

use serde_json::json;

use super::execute::execute;
use super::finalize::mark_routine;

pub async fn start(
    context: RecipeContext,
    workspace_id: &str,
    recipe: Recipe,
    trigger: serde_json::Value,
    dry_run: bool,
) -> AppResult<store::RecipeRun> {
    let index = store::get(&context.pool, workspace_id, &recipe.id).await?;
    if !index.is_enabled || !recipe.enabled {
        return Err(AppError::BadRequest("receita desabilitada".into()));
    }
    let workspace =
        opendesk_storage::repos::workspaces::get(&context.pool, &index.workspace_id).await?;
    let output = opendesk_engine::dsh_router::output_directory(
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
            &context.pool,
            &recipe,
            &index.workspace_id,
            None,
            &trigger,
            &output,
            true,
        )
        .await?;
        store::sync_run_nodes(&context.pool, &run.id, &recipe.flow).await?;
        let context_clone = context.clone();
        let recipe_clone = recipe.clone();
        let run_clone = run.clone();
        tokio::spawn(async move {
            let _ = store::mark_pending_skipped(&context_clone.pool, &run_clone.id).await;
            let _ = store::update_run(
                &context_clone.pool,
                &run_clone.id,
                "COMPLETED",
                None,
                Some(&json!({"dry_run": true})),
                None,
                true,
            )
            .await;
            context_clone
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
        &context.pool,
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
            preferred_harness: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags: vec![format!("recipe:{}", recipe.id)],
            origin: Some("RECIPE".into()),
            parent_task_id: None,
            dedupe: None,
        },
    )
    .await?;
    if task.deduplicated
        && let Some(existing) = store::latest_run_for_task(&context.pool, &task.id).await?
    {
        tracing::info!(
            recipe_id = %recipe.id,
            run_id = %existing.id,
            "recipe run deduplicated"
        );
        return Ok(existing);
    }
    tasks::set_output_dir(&context.pool, &task.id, &output).await?;
    let run = store::create_run(
        &context.pool,
        &recipe,
        &index.workspace_id,
        Some(&task.id),
        &trigger,
        &output,
        false,
    )
    .await?;
    store::sync_run_nodes(&context.pool, &run.id, &recipe.flow).await?;
    let context_clone = context.clone();
    let run_clone = run.clone();
    let recipe_clone = recipe.clone();
    tokio::spawn(async move {
        if let Err(error) = execute(
            &context_clone,
            &recipe_clone,
            &run_clone,
            &trigger,
            context_clone.dispatcher(),
        )
        .await
        {
            tracing::warn!(run = %run_clone.id, error = %error, "recipe run failed");
            let message = format!("{error:#}");
            let _ = store::fail_running_nodes(&context_clone.pool, &run_clone.id, &message).await;
            let _ = store::update_run(
                &context_clone.pool,
                &run_clone.id,
                "FAILED",
                None,
                None,
                Some(&message),
                true,
            )
            .await;
            if let Some(task_id) = &run_clone.task_id {
                let _ = tasks::set_phase_status(
                    &context_clone.pool,
                    task_id,
                    Some("failed"),
                    Some("BLOCKED"),
                )
                .await;
                let _ = tasks::set_blocked_reason(
                    &context_clone.pool,
                    task_id,
                    Some(&format!("{error:#}")),
                )
                .await;
            }
            mark_routine(&context_clone, &trigger, "FAILED").await;
            context_clone
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

/// Restarts a durable run after boot/crash/retry. Existing DONE nodes are kept.
pub async fn resume(
    context: RecipeContext,
    recipe: Recipe,
    run: store::RecipeRun,
) -> AppResult<()> {
    let trigger: serde_json::Value =
        serde_json::from_str(&run.trigger_context_json).unwrap_or_else(|_| json!({}));
    store::sync_run_nodes(&context.pool, &run.id, &recipe.flow).await?;
    let _ = store::update_run(
        &context.pool,
        &run.id,
        "RUNNING_STEP",
        None,
        None,
        None,
        false,
    )
    .await;
    let context_clone = context.clone();
    let run_clone = run.clone();
    let recipe_clone = recipe.clone();
    tokio::spawn(async move {
        if let Err(error) = execute(
            &context_clone,
            &recipe_clone,
            &run_clone,
            &trigger,
            context_clone.dispatcher(),
        )
        .await
        {
            let message = format!("{error:#}");
            let _ = store::fail_running_nodes(&context_clone.pool, &run_clone.id, &message).await;
            let _ = store::update_run(
                &context_clone.pool,
                &run_clone.id,
                "FAILED",
                None,
                None,
                Some(&message),
                true,
            )
            .await;
            if let Some(task_id) = &run_clone.task_id {
                let _ = tasks::set_phase_status(
                    &context_clone.pool,
                    task_id,
                    Some("failed"),
                    Some("BLOCKED"),
                )
                .await;
                let _ =
                    tasks::set_blocked_reason(&context_clone.pool, task_id, Some(&message)).await;
            }
            context_clone
                .events
                .publish(OpenDeskWsEvent::RecipeRunUpdated {
                    run_id: run_clone.id,
                    recipe_id: recipe_clone.id,
                    workspace_id: run_clone.workspace_id,
                    status: "FAILED".into(),
                });
        }
    });
    Ok(())
}
