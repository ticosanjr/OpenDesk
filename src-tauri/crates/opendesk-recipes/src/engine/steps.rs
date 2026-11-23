use crate::{
    context::RecipeContext,
    error::{AppError, AppResult},
    hitl::{self, HitlOutcome, HitlResolution},
    parser::{self, FlowItem, Step},
    store,
};
use opendesk_storage::repos::tasks;

use chrono::Utc;
use std::time::Duration;

pub(super) fn ensure_step_finished(step_id: &str, status: &str) -> AppResult<()> {
    match status {
        "SUCCESS" => Ok(()),
        "FAILED" => Err(AppError::BadRequest(format!(
            "step {step_id} falhou; a receita foi interrompida"
        ))),
        "CANCELLED" => Err(AppError::BadRequest(format!(
            "step {step_id} foi cancelado; a receita foi interrompida"
        ))),
        other => Err(AppError::BadRequest(format!(
            "step {step_id} terminou com status inesperado '{other}'"
        ))),
    }
}

pub(super) fn previous_step(flow: &[FlowItem], index: usize) -> Option<(usize, &Step)> {
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
pub(crate) async fn execute_step(
    context: &RecipeContext,
    dispatcher: &dyn opendesk_engine::recipe_dispatch::RecipeStepDispatcher,
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
        &context.pool,
        &run.id,
        "RUNNING_STEP",
        Some(&step.id),
        Some(outputs),
        None,
        false,
    )
    .await?;
    if let Some(schema) = step.input_schema.as_ref() {
        let input = serde_json::json!({
            "trigger": trigger,
            "run": run_vars,
            "steps": outputs,
        });
        let validator = jsonschema::validator_for(schema)
            .map_err(|error| AppError::BadRequest(format!("input_schema inválido: {error}")))?;
        validator.validate(&input).map_err(|error| {
            AppError::BadRequest(format!("input do step {} fora do schema: {error}", step.id))
        })?;
    }

    let templates = opendesk_engine::dsh_router::prompt_templates::PromptTemplates::load(
        &context.pool,
        Some(&run.workspace_id),
    )
    .await
    .unwrap_or_default();
    let base_prompt = parser::render(&step.prompt, trigger, run_vars, outputs)?;
    let schema_block = step.output_schema.as_ref().map(|schema| {
        format!(
            "\n\n## Contrato de saída (JSON Schema)\nResponda APENAS com um objeto JSON válido para este schema, sem texto fora do JSON:\n```json\n{}\n```",
            serde_json::to_string_pretty(schema).unwrap_or_default()
        )
    });
    let mut attempt = 0usize;
    let mut failure_retries = 0usize;
    let max_failure_retries = step.retry_max.unwrap_or(0);
    let mut correction = feedback;
    loop {
        attempt += 1;
        let mut prompt = base_prompt.clone();
        if let Some(feedback) = correction.as_deref() {
            let block = opendesk_engine::dsh_router::prompt_templates::render(
                &templates.recipe_feedback,
                &[("feedback", feedback)],
            );
            prompt.push_str("\n\n");
            prompt.push_str(&block);
        }
        if let Some(block) = schema_block.as_deref() {
            prompt.push_str(block);
        }
        tasks::update(
            &context.pool,
            task_id,
            tasks::UpdateTaskInput {
                description: Some(prompt.clone()),
                ..Default::default()
            },
        )
        .await?;
        let started = Utc::now().to_rfc3339();
        opendesk_engine::recipe_dispatch::RecipeStepDispatcher::dispatch(
            dispatcher,
            task_id,
            opendesk_engine::recipe_dispatch::RecipeStepRequest {
                agent: step
                    .agent
                    .clone()
                    .map(|value| value.trim_start_matches('@').to_string()),
                model: step.model.clone(),
                effort: step.effort.clone(),
                execution_mode: Some(mode_for_step(step)),
                prompt_append: None,
            },
            session_id.as_deref(),
        )
        .await?;
        let run_result = wait_for_run(
            context,
            &run.workspace_id,
            task_id,
            &started,
            &run.id,
            step.timeout_secs.unwrap_or(2 * 60 * 60),
        )
        .await?;
        if let Some(routine_id) = routine_id {
            let _ = opendesk_storage::repos::runs::set_schedule_id(
                &context.pool,
                &run_result.id,
                routine_id,
            )
            .await;
        }
        if run_result.status == "FAILED" && failure_retries < max_failure_retries {
            failure_retries += 1;
            correction = Some(
                "A execução anterior falhou. Tente novamente, corrija o erro e preserve o contrato."
                    .to_string(),
            );
            continue;
        }
        ensure_step_finished(&step.id, &run_result.status)?;
        *session_id = run_result.session_id.clone();
        let summary = run_result.summary_response.clone().unwrap_or_default();
        let mut structured = None;
        if let Some(schema) = step.output_schema.as_ref() {
            match parser::validate_output(schema, &summary) {
                Ok(value) => structured = Some(value),
                Err(error) => {
                    let policy = step.on_schema_error.as_deref().unwrap_or("retry_format");
                    if policy == "retry_format" && attempt < 2 {
                        correction = Some(format!(
                            "A resposta anterior não validou o contrato: {error}. Responda novamente apenas com JSON válido."
                        ));
                        continue;
                    }
                    if policy == "hitl" {
                        let (approved, feedback) =
                            schema_hitl(context, run, step, &error.to_string()).await?;
                        if approved {
                            structured = None;
                        } else if let Some(feedback) = feedback {
                            correction = Some(feedback);
                            continue;
                        } else {
                            return Err(error);
                        }
                    } else {
                        return Err(error);
                    }
                }
            }
        }
        let artifacts =
            opendesk_storage::repos::artifacts::list_by_run(&context.pool, &run_result.id, 200)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|artifact| artifact.file_path)
                .collect::<Vec<_>>();
        outputs[&step.id] = serde_json::json!({
            "run_id": run_result.id,
            "summary": summary,
            "artifacts": artifacts,
            "structured": structured,
            "attempts": attempt,
        });
        store::update_run(
            &context.pool,
            &run.id,
            "RUNNING_STEP",
            Some(&step.id),
            Some(outputs),
            None,
            false,
        )
        .await?;
        return Ok(());
    }
}

async fn schema_hitl(
    context: &RecipeContext,
    run: &store::RecipeRun,
    step: &Step,
    error: &str,
) -> AppResult<(bool, Option<String>)> {
    let checkpoint = parser::HitlCheckpoint {
        id: format!(
            "schema-{}-{}",
            step.id,
            chrono::Utc::now().timestamp_millis()
        ),
        after_step: String::new(),
        name: Some(format!("Schema do step {}", step.name)),
        description: None,
        interaction_type: "APPROVAL_WITH_FEEDBACK".into(),
        channels: vec!["DESKTOP_UI".into()],
        timeout_minutes: 60,
        on_timeout: "ABORT".into(),
        prompt_template: format!(
            "A saída do step {} não validou o schema: {error}. Aprovar mesmo assim ou enviar uma correção?",
            step.name
        ),
        attach_files: Vec::new(),
        options: Vec::new(),
    };
    let token = hitl::request(
        context,
        run,
        &checkpoint,
        &step.name,
        &checkpoint.prompt_template,
        None,
        &[],
    )
    .await?;
    match hitl::wait(context, &token, checkpoint.timeout_minutes).await? {
        HitlOutcome::Resolved(HitlResolution::Approved) => Ok((true, None)),
        HitlOutcome::Resolved(HitlResolution::FeedbackProvided { feedback_text, .. }) => {
            Ok((false, Some(feedback_text)))
        }
        _ => Ok((false, None)),
    }
}

async fn wait_for_run(
    context: &RecipeContext,
    workspace_id: &str,
    task_id: &str,
    started_after: &str,
    recipe_run_id: &str,
    timeout_secs: u64,
) -> AppResult<opendesk_storage::repos::runs::TaskRun> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs.clamp(1, 7200));
    loop {
        if store::is_cancel_requested(&context.pool, recipe_run_id).await? {
            return Err(AppError::BadRequest("cancelado pelo usuário".into()));
        }
        if let Ok(runs) = opendesk_storage::repos::runs::list_with_limit(
            &context.pool,
            workspace_id,
            Some(task_id),
            5,
        )
        .await
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

pub(super) fn mode_for_step(step: &Step) -> String {
    match step.mode.as_str() {
        "plan" => "PLAN_ONLY".into(),
        _ => "BUILD_DIRECT".into(),
    }
}
