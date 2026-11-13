//! E2E-only deterministic seed endpoint. Disabled unless `OPENDESK_E2E=1`.
//! It creates realistic operational records so Playwright can test the UI
//! without a live OpenCode daemon.

use crate::{
    app_state::AppState,
    db::{
        artifacts::{self, CreateArtifactInput},
        permissions,
        proposals::{self, ProposalInput},
        runs::{self, CreateRunInput, RunUsage},
        tasks, workspaces,
    },
    error::{AppError, AppResult},
    recipes::{parser, store as recipe_store},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct SeedBody {
    pub workspace_id: Option<String>,
    pub run: Option<SeedRun>,
    #[serde(default)]
    pub artifacts: Vec<SeedArtifact>,
    pub hitl: Option<SeedHitl>,
    pub proposal: Option<SeedProposal>,
    pub permission: Option<SeedPermission>,
    pub task: Option<SeedTask>,
}

#[derive(Debug, Deserialize)]
pub struct SeedRun {
    pub id: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_agent")]
    pub agent: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub tokens_input: i64,
    #[serde(default)]
    pub tokens_output: i64,
    #[serde(default)]
    pub cost_usd: f64,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
    #[serde(default)]
    pub events: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SeedArtifact {
    pub file_path: String,
    #[serde(default)]
    pub content: String,
    #[serde(default = "default_action")]
    pub action_type: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct SeedHitl {
    pub checkpoint_id: String,
    #[serde(default = "default_hitl_type")]
    pub interaction_type: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub on_timeout: String,
}

#[derive(Debug, Deserialize)]
pub struct SeedProposal {
    #[serde(default = "default_proposal_kind")]
    pub kind: String,
    #[serde(default = "default_proposal_action")]
    pub action: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
pub struct SeedPermission {
    #[serde(default = "default_permission_action")]
    pub action: String,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default = "default_session")]
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SeedTask {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_task_status")]
    pub status: String,
    #[serde(default)]
    pub phase_status: Option<String>,
}

fn default_status() -> String {
    "SUCCESS".into()
}
fn default_agent() -> String {
    "build".into()
}
fn default_model() -> String {
    "opencode-go/glm-5.3".into()
}
fn default_action() -> String {
    "CREATED".into()
}
fn default_hitl_type() -> String {
    "BINARY_GATE".into()
}
fn default_proposal_kind() -> String {
    "SKILL".into()
}
fn default_proposal_action() -> String {
    "CREATE".into()
}
fn default_permission_action() -> String {
    "bash".into()
}
fn default_session() -> String {
    "e2e-session".into()
}
fn default_task_status() -> String {
    "TODO".into()
}

pub async fn seed(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SeedBody>,
) -> AppResult<Json<Value>> {
    if !cfg!(test) && std::env::var("OPENDESK_E2E").as_deref() != Ok("1") {
        return Err(AppError::NotFound("test seed endpoint disabled".into()));
    }
    let workspace_id = match body.workspace_id {
        Some(id) => id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let mut output = json!({ "workspace_id": workspace_id });

    let mut task_id: Option<String> = None;
    if let Some(task) = body.task {
        let created = tasks::create(
            &state.db,
            &workspace_id,
            tasks::CreateTaskInput {
                title: task.title,
                description: task.description,
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: Some(2),
                assigned_agent: task.agent,
                preferred_model: None,
                effort_level: None,
                care_level: Some("standard".into()),
                notify_email: None,
                export_to_gdocs: false,
                tags: task.tags,
                origin: Some("MANUAL".into()),
                parent_task_id: None,
                dedupe: None,
            },
        )
        .await?;
        if task.status != "TODO" {
            let _ = tasks::move_to(&state.db, &created.id, &task.status, None).await;
        }
        if let Some(phase_status) = task.phase_status.as_deref() {
            tasks::set_phase_status(&state.db, &created.id, Some(phase_status), None).await?;
        }
        task_id = Some(created.id.clone());
        output["task_id"] = json!(created.id);
    }

    let mut run_id: Option<String> = None;
    if let Some(run) = body.run {
        let id = run.id.unwrap_or_else(|| Uuid::now_v7().to_string());
        let log_path = state.paths.run_logs_dir.join(format!("{id}.jsonl"));
        if !run.events.is_empty() {
            let mut content = String::new();
            for event in &run.events {
                content.push_str(&serde_json::to_string(event)?);
                content.push('\n');
            }
            std::fs::write(&log_path, content)?;
        }
        runs::create(
            &state.db,
            CreateRunInput {
                id: id.clone(),
                queue_job_id: None,
                workspace_id: workspace_id.clone(),
                task_id: task_id.clone(),
                schedule_id: None,
                parent_run_id: None,
                phase: Some("build".into()),
                agent_name: run.agent.clone(),
                execution_mode: "build".into(),
                provider_id: Some("opencode-go".into()),
                model_id: run.model.clone(),
                effort_level: "high".into(),
                pricing_snapshot_json: None,
                log_path: Some(log_path.to_string_lossy().to_string()),
            },
        )
        .await?;
        runs::finish(
            &state.db,
            &id,
            &run.status,
            &RunUsage {
                tokens_input: run.tokens_input,
                tokens_output: run.tokens_output,
                estimated_cost_usd: run.cost_usd,
                ..Default::default()
            },
            run.duration_ms,
            run.summary.as_deref(),
            None,
        )
        .await?;
        if let Some(error) = run.error.as_deref() {
            let code = run.error_code.as_deref().unwrap_or("run_failed");
            runs::set_error(&state.db, &id, code, error, "hint de teste").await?;
        }
        output["run_id"] = json!(id);
        run_id = Some(id);
    }

    if !body.artifacts.is_empty() {
        let run = run_id
            .clone()
            .ok_or_else(|| AppError::BadRequest("artifacts exigem run".into()))?;
        let output_dir = state.paths.artifacts_dir.join(&workspace_id);
        std::fs::create_dir_all(&output_dir)?;
        let mut ids = Vec::new();
        for artifact in body.artifacts {
            let absolute = output_dir.join(&artifact.file_path);
            if let Some(parent) = absolute.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&absolute, &artifact.content)?;
            let created = artifacts::create(
                &state.db,
                CreateArtifactInput {
                    workspace_id: workspace_id.clone(),
                    task_id: task_id.clone(),
                    run_id: run.clone(),
                    file_path: artifact.file_path,
                    absolute_path: absolute.to_string_lossy().to_string(),
                    file_type: "markdown".into(),
                    file_size_bytes: artifact.content.len() as i64,
                    content_sha256: None,
                    previous_sha256: None,
                    action_type: artifact.action_type,
                    created_by_agent: if artifact.agent.is_empty() {
                        "build".into()
                    } else {
                        artifact.agent
                    },
                    creation_reason: if artifact.reason.is_empty() {
                        "seed E2E".into()
                    } else {
                        artifact.reason
                    },
                    diff_patch: None,
                },
            )
            .await?;
            ids.push(created.id);
        }
        output["artifact_ids"] = json!(ids);
    }

    if let Some(hitl) = body.hitl {
        let yaml = "id: e2e-seed\nname: Receita E2E\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo\n    prompt: oi\n";
        let recipe = parser::parse(yaml)?;
        // Write the recipe file too: the workspace watcher prunes index rows
        // that have no source file, which would cascade-delete the HITL rows.
        let recipe_path =
            std::path::Path::new(&workspace.root_path).join(".opencode/recipes/e2e-seed.yaml");
        if let Some(parent) = recipe_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&recipe_path, yaml)?;
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
             VALUES ('e2e-seed', ?, 'Receita E2E', ?, 'MANUAL')",
        )
        .bind(&workspace_id)
        .bind(recipe_path.to_string_lossy().to_string())
        .execute(&state.db)
        .await;
        let recipe_run = recipe_store::create_run(
            &state.db,
            &recipe,
            &workspace_id,
            task_id.as_deref(),
            &json!({}),
            &state.paths.artifacts_dir.to_string_lossy(),
            false,
        )
        .await?;
        let token = format!("E2E-{}", &Uuid::now_v7().to_string()[..8]);
        let request = recipe_store::create_hitl(
            &state.db,
            &token,
            &recipe_run,
            &hitl.checkpoint_id,
            &hitl.interaction_type,
            if hitl.title.is_empty() {
                "Receita E2E"
            } else {
                &hitl.title
            },
            if hitl.question.is_empty() {
                "Aprovar esta etapa?"
            } else {
                &hitl.question
            },
            &json!({ "next_step": "Publicar", "recipe_name": "Receita E2E" }),
            &hitl.options,
            if hitl.on_timeout.is_empty() {
                "PAUSE_AND_ALERT"
            } else {
                &hitl.on_timeout
            },
            "2999-01-01T00:00:00Z",
        )
        .await?;
        output["hitl_token"] = json!(request.token);
    }

    if let Some(proposal) = body.proposal {
        let created = proposals::create(
            &state.db,
            &workspace_id,
            ProposalInput {
                kind: proposal.kind,
                action: proposal.action,
                title: proposal.title,
                description: proposal.description,
                payload: proposal.payload,
                requested_by: Some("e2e".into()),
                source_run_id: run_id.clone(),
                source_task_id: task_id.clone(),
            },
        )
        .await?;
        output["proposal_id"] = json!(created.id);
    }

    if let Some(permission) = body.permission {
        let created = permissions::create(
            &state.db,
            run_id.as_deref(),
            task_id.as_deref(),
            &workspace_id,
            &permission.session_id,
            &format!("req-{}", Uuid::now_v7()),
            &permission.action,
            &permission.resources,
        )
        .await?;
        output["permission_id"] = json!(created.request_id);
    }

    Ok(Json(output))
}
