use crate::{
    engine::{finish, replace_attachment},
    error::AppResult,
    hitl::{self, HitlOutcome, HitlResolution},
    parser::{self, FlowItem, HitlCheckpoint},
    store,
};
use opendesk_storage::events::OpenDeskWsEvent;
use opendesk_storage::repos::tasks;

use super::flow::{next_item_name, previous_step};
use super::paths::{child_path, parent_path};
use super::types::{Ctx, ScopeState};

pub(super) async fn handle_hitl(
    ctx: &Ctx<'_>,
    checkpoint: &HitlCheckpoint,
    path: &str,
    items: &[FlowItem],
    index: usize,
    scope: &mut ScopeState,
) -> AppResult<bool> {
    let mut checkpoint = checkpoint.clone();
    checkpoint.id = parser::checkpoint_id(&checkpoint);
    let current_step = previous_step(items, index).map(|(_, step)| step.id.clone());
    store::update_run(
        &ctx.context.pool,
        &ctx.run.id,
        "WAITING_HITL",
        current_step.as_deref(),
        Some(&scope.outputs),
        None,
        false,
    )
    .await?;
    store::mark_node_path(
        &ctx.context.pool,
        &ctx.run.id,
        path,
        "WAITING_HITL",
        None,
        None,
        None,
    )
    .await?;
    let _ = tasks::move_to(&ctx.context.pool, ctx.task_id, "IN_REVIEW", None).await;
    let _ = tasks::set_phase_status(
        &ctx.context.pool,
        ctx.task_id,
        Some("waiting_hitl"),
        Some("IN_REVIEW"),
    )
    .await;
    let message = parser::render(
        &checkpoint.prompt_template,
        ctx.trigger,
        ctx.run_vars,
        &scope.outputs,
    )?;
    let attachments = checkpoint
        .attach_files
        .iter()
        .map(|path| parser::render(path, ctx.trigger, ctx.run_vars, &scope.outputs))
        .collect::<Result<Vec<_>, _>>()?;
    let next_step = next_item_name(items, index + 1);
    let token =
        match store::pending_hitl_for_checkpoint(&ctx.context.pool, &ctx.run.id, &checkpoint.id)
            .await?
        {
            Some(existing) => existing.token,
            None => {
                hitl::request(
                    ctx.context,
                    ctx.run,
                    &checkpoint,
                    &ctx.recipe.name,
                    &message,
                    next_step,
                    &attachments,
                )
                .await?
            }
        };
    ctx.context.events.publish(OpenDeskWsEvent::HitlRequested {
        token: token.clone(),
        run_id: ctx.run.id.clone(),
        workspace_id: ctx.run.workspace_id.clone(),
        checkpoint_id: checkpoint.id.clone(),
        interaction_type: checkpoint.interaction_type.clone(),
    });
    let outcome = hitl::wait(ctx.context, &token, checkpoint.timeout_minutes).await?;
    match outcome {
        HitlOutcome::Resolved(HitlResolution::Approved) => {
            ctx.context.events.publish(OpenDeskWsEvent::HitlResolved {
                token,
                status: "APPROVED".into(),
            });
            store::mark_node_path(
                &ctx.context.pool,
                &ctx.run.id,
                path,
                "DONE",
                None,
                None,
                None,
            )
            .await?;
            Ok(true)
        }
        HitlOutcome::Resolved(HitlResolution::Rejected { reason }) => {
            finish(
                ctx.context,
                ctx.run,
                "REJECTED",
                reason.as_deref().unwrap_or("rejeitado pelo usuário"),
                ctx.routine_id,
            )
            .await?;
            Ok(false)
        }
        HitlOutcome::Resolved(HitlResolution::FeedbackProvided {
            feedback_text,
            attached_files,
        }) => {
            if let Some(file) = attached_files.first() {
                replace_attachment(file, &attachments)?;
                store::mark_node_path(
                    &ctx.context.pool,
                    &ctx.run.id,
                    path,
                    "DONE",
                    None,
                    None,
                    None,
                )
                .await?;
                return Ok(true);
            }
            replay_previous(ctx, items, index, path, &feedback_text, scope).await
        }
        HitlOutcome::Resolved(HitlResolution::ChoiceSelected { option_value, .. }) => {
            let feedback = format!("Opção escolhida: {option_value}");
            replay_previous(ctx, items, index, path, &feedback, scope).await
        }
        HitlOutcome::Resolved(HitlResolution::ArtifactEdited { path: edited }) => {
            replace_attachment(&edited, &attachments)?;
            store::mark_node_path(
                &ctx.context.pool,
                &ctx.run.id,
                path,
                "DONE",
                None,
                None,
                None,
            )
            .await?;
            Ok(true)
        }
        HitlOutcome::Expired => {
            if checkpoint.on_timeout == "AUTO_APPROVE" {
                store::mark_node_path(
                    &ctx.context.pool,
                    &ctx.run.id,
                    path,
                    "DONE",
                    None,
                    None,
                    None,
                )
                .await?;
                Ok(true)
            } else {
                finish(
                    ctx.context,
                    ctx.run,
                    "TIMED_OUT",
                    "timeout do checkpoint HITL",
                    ctx.routine_id,
                )
                .await?;
                Ok(false)
            }
        }
        HitlOutcome::Cancelled => {
            finish(
                ctx.context,
                ctx.run,
                "REJECTED",
                "cancelado pelo usuário",
                ctx.routine_id,
            )
            .await?;
            Ok(false)
        }
    }
}

/// Records feedback for the previous step and rewinds the scope so it runs
/// again. The caller only advances after the HITL is approved.
async fn replay_previous(
    ctx: &Ctx<'_>,
    items: &[FlowItem],
    index: usize,
    hitl_path: &str,
    feedback: &str,
    scope: &mut ScopeState,
) -> AppResult<bool> {
    scope.review_rounds += 1;
    if scope.review_rounds > 5 {
        finish(
            ctx.context,
            ctx.run,
            "REJECTED",
            "limite de revisões atingido",
            ctx.routine_id,
        )
        .await?;
        return Ok(false);
    }
    let Some((_previous_index, previous_step)) = previous_step(items, index) else {
        finish(
            ctx.context,
            ctx.run,
            "REJECTED",
            "checkpoint sem step anterior para refazer",
            ctx.routine_id,
        )
        .await?;
        return Ok(false);
    };
    let previous_path = child_path(parent_path(hitl_path), &previous_step.id);
    scope
        .feedback
        .insert(previous_step.id.clone(), feedback.to_string());
    store::reset_node_path(&ctx.context.pool, &ctx.run.id, &previous_path).await?;
    store::reset_node_path(&ctx.context.pool, &ctx.run.id, hitl_path).await?;
    Ok(true)
}
