//! A/B runs over isolated worktrees (C2).
//!
//! The endpoint creates two ordinary tasks with the same prompt, tags them with
//! the group and slot, and lets the queue create one worktree per task. The
//! comparison is read from the latest run, and the winner is merged only after
//! an explicit approval flag.

use crate::{
    app_state::AppState,
    db::{self, ab, queue, runs, tasks, worktrees},
    error::{AppError, AppResult},
    worktree,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Deserialize, Default)]
pub struct AbQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAbBody {
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub prompt: String,
    #[serde(default = "default_agent")]
    pub agent_a: String,
    #[serde(default = "default_agent")]
    pub agent_b: String,
    pub model_a: String,
    pub model_b: String,
    #[serde(default)]
    pub effort: Option<String>,
}

fn default_agent() -> String {
    "build".into()
}

#[derive(Debug, Deserialize)]
pub struct ChooseBody {
    pub slot: String,
    #[serde(default)]
    pub approved: bool,
}

#[derive(Debug, Serialize)]
pub struct AbSlot {
    pub slot: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub worktree_id: Option<String>,
    pub status: String,
    pub agent: String,
    pub model: String,
    pub cost_usd: f64,
    pub tokens: i64,
    pub duration_ms: i64,
    pub diff: Option<String>,
    pub diff_stat: worktree::DiffStat,
}

#[derive(Debug, Serialize)]
pub struct AbDetail {
    #[serde(flatten)]
    pub group: ab::AbGroup,
    pub slots: Vec<AbSlot>,
}

fn has_tag(tags_json: &str, needle: &str) -> bool {
    serde_json::from_str::<Vec<String>>(tags_json)
        .map(|tags| tags.iter().any(|tag| tag == needle))
        .unwrap_or(false)
}

fn slot_of(task: &tasks::Task) -> Option<&'static str> {
    has_tag(&task.tags_json, "ab-slot:a")
        .then_some("a")
        .or_else(|| has_tag(&task.tags_json, "ab-slot:b").then_some("b"))
}

async fn slot_tasks(
    state: &AppState,
    group: &ab::AbGroup,
) -> AppResult<(Option<tasks::Task>, Option<tasks::Task>)> {
    let needle = format!("ab:{}", group.id);
    let all = tasks::list(&state.db, &group.workspace_id).await?;
    let mut a = None;
    let mut b = None;
    for task in all {
        if !has_tag(&task.tags_json, &needle) {
            continue;
        }
        match slot_of(&task) {
            Some("a") => a = Some(task),
            Some("b") => b = Some(task),
            _ => {}
        }
    }
    Ok((a, b))
}

async fn load_slot(
    state: &AppState,
    group: &ab::AbGroup,
    slot: &str,
    task: Option<tasks::Task>,
) -> AppResult<AbSlot> {
    let Some(task) = task else {
        return Err(AppError::NotFound(format!("slot A/B {slot}")));
    };
    let run = runs::latest_for_task(&state.db, &task.id).await?;
    let mut worktree_id = None;
    let mut diff = None;
    let mut diff_stat = worktree::DiffStat::default();
    if let Some(run) = &run
        && let Some(found) = worktrees::by_run(&state.db, &run.id).await?
    {
        worktree_id = Some(found.id.clone());
        let workspace = db::workspaces::get(&state.db, &group.workspace_id).await?;
        let root = PathBuf::from(workspace.root_path);
        let found_clone = found.clone();
        if let Ok(Ok(patch)) =
            tokio::task::spawn_blocking(move || worktree::diff(&found_clone, &root)).await
        {
            diff_stat = worktree::stat_from_diff(&patch);
            diff = Some(patch);
        }
    }
    Ok(AbSlot {
        slot: slot.to_string(),
        task_id: task.id.clone(),
        run_id: run.as_ref().map(|run| run.id.clone()),
        worktree_id,
        status: run
            .as_ref()
            .map(|run| run.status.clone())
            .or(task.active_job_status.clone())
            .unwrap_or_else(|| "QUEUED".into()),
        agent: task
            .assigned_agent
            .clone()
            .unwrap_or_else(|| "build".into()),
        model: task.preferred_model.clone().unwrap_or_default(),
        cost_usd: run
            .as_ref()
            .map(|run| run.estimated_cost_usd)
            .unwrap_or(0.0),
        tokens: run
            .as_ref()
            .map(|run| run.tokens_input + run.tokens_output)
            .unwrap_or(0),
        duration_ms: run.as_ref().map(|run| run.duration_ms).unwrap_or(0),
        diff,
        diff_stat,
    })
}

async fn detail(state: &AppState, group_id: &str) -> AppResult<AbDetail> {
    let group = ab::get(&state.db, group_id).await?;
    let (a, b) = slot_tasks(state, &group).await?;
    let slots = vec![
        load_slot(state, &group, "a", a).await?,
        load_slot(state, &group, "b", b).await?,
    ];
    Ok(AbDetail { group, slots })
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AbQuery>,
) -> AppResult<Json<Vec<ab::AbGroup>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(ab::list(&state.db, &workspace_id).await?))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateAbBody>,
) -> AppResult<Json<AbDetail>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    db::workspaces::get(&state.db, &workspace_id).await?;
    crate::api::tasks::validate_model_ref(Some(&body.model_a))?;
    crate::api::tasks::validate_model_ref(Some(&body.model_b))?;
    let effort = body.effort.as_deref().unwrap_or("medium");
    let title = body
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("A/B de worktrees");
    let group = ab::create(&state.db, &workspace_id, title, &body.prompt).await?;
    for (slot, agent, model) in [
        ("a", body.agent_a.as_str(), body.model_a.as_str()),
        ("b", body.agent_b.as_str(), body.model_b.as_str()),
    ] {
        let task = tasks::create(
            &state.db,
            &workspace_id,
            tasks::CreateTaskInput {
                title: format!("{title} · {}", slot.to_uppercase()),
                description: body.prompt.clone(),
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: Some(2),
                assigned_agent: Some(agent.to_string()),
                preferred_model: Some(model.to_string()),
                preferred_harness: Some("opencode".into()),
                effort_level: Some(effort.to_string()),
                care_level: Some("standard".into()),
                notify_email: None,
                export_to_gdocs: false,
                tags: vec![format!("ab:{}", group.id), format!("ab-slot:{slot}")],
                origin: Some("AB_TEST".into()),
                parent_task_id: None,
                dedupe: Some(false),
            },
        )
        .await?;
        state
            .queue
            .enqueue(queue::EnqueueInput {
                workspace_id: workspace_id.clone(),
                task_id: Some(task.id.clone()),
                schedule_id: None,
                session_id: None,
                concurrency_group: Some("light_api".into()),
                prompt_payload: body.prompt.clone(),
                resolved_mode: "BUILD_DIRECT".into(),
                resolved_agent: agent.to_string(),
                resolved_model: model.to_string(),
                resolved_effort: effort.to_string(),
                resolved_harness: "opencode".into(),
                job_kind: "task".into(),
                scheduled_for: None,
                output_dir: None,
                priority: 2,
                max_retries: 0,
            })
            .await?;
    }
    Ok(Json(detail(&state, &group.id).await?))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<AbDetail>> {
    Ok(Json(detail(&state, &id).await?))
}

pub async fn choose(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ChooseBody>,
) -> AppResult<Json<AbDetail>> {
    if !body.approved {
        return Err(AppError::BadRequest(
            "escolher o vencedor do A/B exige aprovação explícita".into(),
        ));
    }
    let group = ab::get(&state.db, &id).await?;
    if group.status != "OPEN" {
        return Err(AppError::Conflict("grupo A/B já decidido".into()));
    }
    let slot = body.slot.trim().to_lowercase();
    if slot != "a" && slot != "b" {
        return Err(AppError::BadRequest("slot A/B inválido".into()));
    }
    let (a, b) = slot_tasks(&state, &group).await?;
    let (winner, loser) = if slot == "a" { (a, b) } else { (b, a) };
    let winner = winner.ok_or_else(|| AppError::NotFound(format!("slot A/B {slot}")))?;
    let run = runs::latest_for_task(&state.db, &winner.id)
        .await?
        .ok_or_else(|| AppError::BadRequest("slot vencedor ainda não executou".into()))?;
    let found = worktrees::by_run(&state.db, &run.id)
        .await?
        .ok_or_else(|| AppError::BadRequest("slot vencedor sem worktree isolado".into()))?;
    let workspace = db::workspaces::get(&state.db, &group.workspace_id).await?;
    let root = PathBuf::from(workspace.root_path);
    let merge_worktree = found.clone();
    let merge_root = root.clone();
    let report =
        tokio::task::spawn_blocking(move || worktree::merge(&merge_worktree, &merge_root, true))
            .await
            .map_err(|error| AppError::Internal(format!("worktree task: {error}")))??;
    worktrees::set_status(
        &state.db,
        &report.id,
        if report.status == "MERGED" {
            "MERGED"
        } else {
            "ACTIVE"
        },
    )
    .await?;
    if let Some(loser) = loser
        && let Some(loser_run) = runs::latest_for_task(&state.db, &loser.id).await?
        && let Some(loser_tree) = worktrees::by_run(&state.db, &loser_run.id).await?
    {
        let loser_tree_clone = loser_tree.clone();
        let loser_root = root.clone();
        let _ =
            tokio::task::spawn_blocking(move || worktree::cleanup(&loser_tree_clone, &loser_root))
                .await;
        let _ = worktrees::set_status(&state.db, &loser_tree.id, "REMOVED").await;
    }
    ab::decide(&state.db, &group.id, &slot).await?;
    Ok(Json(detail(&state, &group.id).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[tokio::test]
    async fn creates_two_tagged_slots_and_rejects_unapproved_choice() {
        let ctx = test_support::context().await;
        let created = create(
            State(ctx.state.clone()),
            Json(CreateAbBody {
                workspace_id: Some(ctx.workspace_id.clone()),
                title: Some("A/B teste".into()),
                prompt: "implemente x".into(),
                agent_a: "build".into(),
                agent_b: "build".into(),
                model_a: "opencode-go/glm-5.3".into(),
                model_b: "opencode-go/glm-5.3".into(),
                effort: Some("medium".into()),
            }),
        )
        .await
        .expect("create");
        assert_eq!(created.0.slots.len(), 2);
        assert_eq!(created.0.slots[0].slot, "a");
        assert_eq!(created.0.slots[1].slot, "b");
        let error = choose(
            State(ctx.state.clone()),
            Path(created.0.group.id.clone()),
            Json(ChooseBody {
                slot: "a".into(),
                approved: false,
            }),
        )
        .await
        .expect_err("approval required");
        assert!(format!("{error}").contains("aprovação"));
    }

    fn git(root: &std::path::Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .expect("git");
        assert!(
            output.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn chooses_a_slot_and_merges_only_the_winner() {
        if std::process::Command::new("git")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let ctx = test_support::context().await;
        let root = ctx.workspace_root.clone();
        git(&root, &["init"]);
        git(&root, &["config", "user.email", "test@opendesk.local"]);
        git(&root, &["config", "user.name", "Test"]);
        std::fs::write(root.join("base.txt"), "base\n").expect("base");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-m", "base"]);

        let created = create(
            State(ctx.state.clone()),
            Json(CreateAbBody {
                workspace_id: Some(ctx.workspace_id.clone()),
                title: Some("A/B merge".into()),
                prompt: "implemente x".into(),
                agent_a: "build".into(),
                agent_b: "build".into(),
                model_a: "opencode-go/glm-5.3".into(),
                model_b: "opencode-go/glm-5.3".into(),
                effort: Some("medium".into()),
            }),
        )
        .await
        .expect("create");
        let group = created.0.group.clone();
        let (a, b) = slot_tasks(&ctx.state, &group).await.expect("tasks");
        let (a, b) = (a.expect("slot a"), b.expect("slot b"));

        for (task, name, content) in [(&a, "winner.txt", "winner\n"), (&b, "loser.txt", "loser\n")]
        {
            let run_id = format!("run-{}-{}", task.id, name);
            runs::create(
                &ctx.state.db,
                runs::CreateRunInput {
                    id: run_id.clone(),
                    queue_job_id: None,
                    workspace_id: ctx.workspace_id.clone(),
                    task_id: Some(task.id.clone()),
                    schedule_id: None,
                    parent_run_id: None,
                    phase: Some("build".into()),
                    agent_name: "build".into(),
                    execution_mode: "BUILD_DIRECT".into(),
                    provider_id: Some("opencode-go".into()),
                    model_id: "glm-5.3".into(),
                    effort_level: "medium".into(),
                    pricing_snapshot_json: None,
                    log_path: None,
                },
            )
            .await
            .expect("run");
            let prepared = worktree::prepare(
                &ctx.workspace_id,
                &root,
                &ctx.state.paths.data_dir,
                Some(&run_id),
            )
            .expect("worktree");
            worktrees::insert(&ctx.state.db, &prepared)
                .await
                .expect("insert");
            std::fs::write(std::path::Path::new(&prepared.path).join(name), content)
                .expect("change");
        }

        let chosen = choose(
            State(ctx.state.clone()),
            Path(group.id.clone()),
            Json(ChooseBody {
                slot: "a".into(),
                approved: true,
            }),
        )
        .await
        .expect("choose");
        assert_eq!(chosen.0.group.status, "DECIDED");
        assert_eq!(chosen.0.group.winner_slot.as_deref(), Some("a"));
        assert!(root.join("winner.txt").exists());
        assert!(!root.join("loser.txt").exists());
    }
}
