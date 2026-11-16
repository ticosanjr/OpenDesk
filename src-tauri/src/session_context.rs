//! Durable cross-harness context packet.
//!
//! A task may move through Plan (Claude), Build (OpenCode), Audit (Ollama) and
//! follow-ups. Instead of relying on one harness session, OpenDesk rebuilds a
//! compact packet from the task's plan, recent decisions, artifacts/diffs,
//! memory and budget, persists it, and injects it into the next prompt.

use crate::{db, error::AppResult};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const MAX_PACKET_CHARS: usize = 12_000;
const MAX_PLAN_CHARS: usize = 4_000;
const MAX_DECISIONS: usize = 6;
const MAX_DECISION_CHARS: usize = 600;
const MAX_DIFFS: usize = 8;
const MAX_DIFF_CHARS: usize = 1_200;
const MAX_ARTIFACTS: usize = 16;
const MAX_MEMORY_CHARS: usize = 3_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub file_path: String,
    pub action: String,
    pub patch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    pub daily_spent: f64,
    pub daily_limit: f64,
    pub monthly_spent: f64,
    pub monthly_limit: f64,
    pub hard_stop: bool,
    pub hard_stop_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPacket {
    pub task_id: String,
    pub workspace_id: String,
    pub agent: String,
    pub phase: Option<String>,
    pub plan: Option<String>,
    pub decisions: Vec<String>,
    pub diffs: Vec<DiffEntry>,
    pub artifacts: Vec<String>,
    pub memory: String,
    pub budget: Option<BudgetSnapshot>,
    pub generated_at: String,
}

/// Builds the packet from live state. The result is deterministic for the same
/// task snapshot and never includes full prompts or secrets.
pub async fn build(pool: &SqlitePool, task_id: &str) -> AppResult<SessionPacket> {
    let task = db::tasks::get(pool, task_id).await?;
    let agent = task
        .assigned_agent
        .clone()
        .unwrap_or_else(|| "build".into());
    let messages = db::task_messages::list(pool, task_id)
        .await
        .unwrap_or_default();
    let decisions = messages
        .iter()
        .filter(|message| matches!(message.role.as_str(), "user" | "assistant" | "agent"))
        .rev()
        .take(MAX_DECISIONS)
        .map(|message| {
            format!(
                "{}: {}",
                message.role,
                truncate(&message.content.replace('\n', " "), MAX_DECISION_CHARS)
            )
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let artifacts = db::artifacts::list(pool, &task.workspace_id, None, Some(task_id), None, 50)
        .await
        .unwrap_or_default();
    let diffs = artifacts
        .iter()
        .filter_map(|artifact| {
            let patch = artifact.diff_patch.as_deref()?.trim();
            (!patch.is_empty()).then(|| DiffEntry {
                file_path: artifact.file_path.clone(),
                action: artifact.action_type.clone(),
                patch: truncate(patch, MAX_DIFF_CHARS),
            })
        })
        .take(MAX_DIFFS)
        .collect();
    let artifact_paths = artifacts
        .iter()
        .take(MAX_ARTIFACTS)
        .map(|artifact| artifact.file_path.clone())
        .collect();

    let memory = db::memory::render_context(
        pool,
        &task.workspace_id,
        &agent,
        &format!("{} {}", task.title, task.description),
        5,
    )
    .await
    .unwrap_or_default();
    let budget = db::budget::status(pool)
        .await
        .ok()
        .map(|status| BudgetSnapshot {
            daily_spent: status.daily_spent,
            daily_limit: status.daily_limit,
            monthly_spent: status.monthly_spent,
            monthly_limit: status.monthly_limit,
            hard_stop: status.hard_stop,
            hard_stop_enabled: status.hard_stop_enabled,
        });

    Ok(SessionPacket {
        task_id: task.id,
        workspace_id: task.workspace_id,
        agent,
        phase: task.phase,
        plan: task
            .plan_markdown
            .map(|plan| truncate(plan.trim(), MAX_PLAN_CHARS)),
        decisions,
        diffs,
        artifacts: artifact_paths,
        memory: truncate(memory.trim(), MAX_MEMORY_CHARS),
        budget,
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Persists the current packet and returns the prompt block.
pub async fn render_and_persist(pool: &SqlitePool, task_id: &str) -> AppResult<String> {
    let packet = build(pool, task_id).await?;
    sqlx::query(
        "INSERT INTO session_context_packets (task_id, workspace_id, packet_json, generated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(task_id) DO UPDATE SET
            workspace_id = excluded.workspace_id,
            packet_json = excluded.packet_json,
            generated_at = excluded.generated_at",
    )
    .bind(&packet.task_id)
    .bind(&packet.workspace_id)
    .bind(serde_json::to_string(&packet).unwrap_or_else(|_| "{}".into()))
    .bind(&packet.generated_at)
    .execute(pool)
    .await?;
    Ok(render(&packet))
}

pub fn render(packet: &SessionPacket) -> String {
    let mut text = String::from("## Contexto entre harnesses\n\n");
    if let Some(phase) = &packet.phase {
        text.push_str(&format!("**Fase atual:** {phase}\n"));
    }
    if let Some(plan) = &packet.plan {
        text.push_str(&format!("\n### Plano aprovado\n{plan}\n"));
    }
    if !packet.decisions.is_empty() {
        text.push_str("\n### Decisões recentes\n");
        for decision in &packet.decisions {
            text.push_str(&format!("- {decision}\n"));
        }
    }
    if !packet.diffs.is_empty() {
        text.push_str("\n### Mudanças recentes\n");
        for diff in &packet.diffs {
            text.push_str(&format!(
                "- `{}` ({}){}\n",
                diff.file_path,
                diff.action,
                if diff.patch.trim().is_empty() {
                    String::new()
                } else {
                    format!(": {}", diff.patch.lines().next().unwrap_or_default())
                }
            ));
        }
    }
    if !packet.artifacts.is_empty() {
        text.push_str("\n### Artefatos\n");
        for path in &packet.artifacts {
            text.push_str(&format!("- {path}\n"));
        }
    }
    if !packet.memory.trim().is_empty() {
        text.push_str("\n### Memória relevante\n");
        text.push_str(packet.memory.trim());
        text.push('\n');
    }
    if let Some(budget) = &packet.budget {
        text.push_str(&format!(
            "\n### Orçamento\nDiário: ${:.2}/${:.2} · mensal: ${:.2}/${:.2}{}\n",
            budget.daily_spent,
            budget.daily_limit,
            budget.monthly_spent,
            budget.monthly_limit,
            if budget.hard_stop_enabled && budget.hard_stop {
                " · bloqueado"
            } else {
                ""
            }
        ));
    }
    truncate(text.trim_end(), MAX_PACKET_CHARS)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut output = value
        .chars()
        .take(max.saturating_sub(1))
        .collect::<String>();
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builds_and_persists_a_compact_packet() {
        let ctx = crate::test_support::context().await;
        let task = db::tasks::create(
            &ctx.state.db,
            &ctx.workspace_id,
            db::tasks::CreateTaskInput {
                title: "Plano de lançamento".into(),
                description: "Executar o lançamento".into(),
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: Some(2),
                assigned_agent: Some("build".into()),
                preferred_model: None,
                preferred_harness: None,
                effort_level: None,
                care_level: Some("standard".into()),
                notify_email: None,
                export_to_gdocs: false,
                tags: vec![],
                origin: Some("MANUAL".into()),
                parent_task_id: None,
                dedupe: None,
            },
        )
        .await
        .expect("task");
        db::tasks::update(
            &ctx.state.db,
            &task.id,
            db::tasks::UpdateTaskInput {
                plan_markdown: Some("1. revisar\n2. publicar".into()),
                ..Default::default()
            },
        )
        .await
        .expect("plan");
        db::task_messages::create(
            &ctx.state.db,
            &task.id,
            "user",
            "note",
            "Aprovo a publicação na sexta",
            None,
            None,
        )
        .await
        .expect("message");

        let packet = build(&ctx.state.db, &task.id).await.expect("packet");
        assert_eq!(packet.task_id, task.id);
        assert!(packet.plan.unwrap().contains("publicar"));
        assert!(packet.decisions.iter().any(|item| item.contains("sexta")));
        let prompt = render_and_persist(&ctx.state.db, &task.id)
            .await
            .expect("persist");
        assert!(prompt.contains("Contexto entre harnesses"));
        assert!(prompt.contains("Plano aprovado"));

        let stored: String =
            sqlx::query_scalar("SELECT packet_json FROM session_context_packets WHERE task_id = ?")
                .bind(&task.id)
                .fetch_one(&ctx.state.db)
                .await
                .expect("stored");
        assert!(stored.contains("Plano de lançamento") || stored.contains("publicar"));
    }

    #[test]
    fn truncates_long_text_by_characters() {
        let text = "á".repeat(20);
        let truncated = truncate(&text, 5);
        assert_eq!(truncated.chars().count(), 5);
        assert!(truncated.ends_with('…'));
    }
}
