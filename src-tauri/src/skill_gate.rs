//! Historical regression gate for skills.
//!
//! The gate reads the skill's recorded usage (mentions, successes, failures) and
//! blocks promotion when the sample is too small or the success rate falls
//! below the configured threshold. It is deliberately deterministic and does
//! not call a model; shadow-case replay will plug into the same records later.

use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
    skill_eval,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct GateThresholds {
    #[serde(default = "default_days")]
    pub days: i64,
    #[serde(default = "default_samples")]
    pub min_samples: i64,
    #[serde(default = "default_rate")]
    pub min_success_rate: f64,
}

fn default_days() -> i64 {
    30
}
fn default_samples() -> i64 {
    5
}
fn default_rate() -> f64 {
    0.8
}

impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            days: default_days(),
            min_samples: default_samples(),
            min_success_rate: default_rate(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SkillGate {
    pub id: String,
    pub workspace_id: String,
    pub skill_id: String,
    pub status: String,
    pub mentions: i64,
    pub successes: i64,
    pub failures: i64,
    pub success_rate: Option<f64>,
    pub min_samples: i64,
    pub min_success_rate: f64,
    pub days: i64,
    pub details_json: String,
    pub decision: String,
    pub decision_note: Option<String>,
    pub decided_at: Option<String>,
    pub created_at: String,
}

const COLUMNS: &str = "id, workspace_id, skill_id, status, mentions, successes, failures, \
    success_rate, min_samples, min_success_rate, days, details_json, decision, decision_note, \
    decided_at, created_at";

pub async fn run(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    workspace_root: &Path,
    skill_id: &str,
    thresholds: GateThresholds,
) -> AppResult<SkillGate> {
    if thresholds.days < 1 || thresholds.min_samples < 1 {
        return Err(AppError::BadRequest(
            "days e min_samples do gate precisam ser positivos".into(),
        ));
    }
    if !(0.0..=1.0).contains(&thresholds.min_success_rate) {
        return Err(AppError::BadRequest(
            "min_success_rate precisa ficar entre 0 e 1".into(),
        ));
    }
    let usages = skill_eval::evaluate(pool, paths, workspace_root, thresholds.days).await?;
    let usage = usages
        .into_iter()
        .find(|usage| usage.id == skill_id)
        .ok_or_else(|| AppError::NotFound(format!("skill {skill_id}")))?;
    let samples = (usage.successes + usage.failures) as i64;
    let success_rate = usage.success_rate;
    let status = if samples < thresholds.min_samples {
        "INSUFFICIENT"
    } else if success_rate.unwrap_or(0.0) >= thresholds.min_success_rate {
        "PASS"
    } else {
        "BLOCKED"
    };
    let details = serde_json::json!({
        "mentions": usage.mentions,
        "successes": usage.successes,
        "failures": usage.failures,
        "samples": samples,
        "success_rate": success_rate,
        "last_used_at": usage.last_used_at,
    });
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO skill_eval_gates
            (id, workspace_id, skill_id, status, mentions, successes, failures,
             success_rate, min_samples, min_success_rate, days, details_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(&usage.id)
    .bind(status)
    .bind(usage.mentions as i64)
    .bind(usage.successes as i64)
    .bind(usage.failures as i64)
    .bind(success_rate)
    .bind(thresholds.min_samples)
    .bind(thresholds.min_success_rate)
    .bind(thresholds.days)
    .bind(details.to_string())
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<SkillGate> {
    sqlx::query_as::<_, SkillGate>(&format!(
        "SELECT {COLUMNS} FROM skill_eval_gates WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("gate {id}")))
}

pub async fn latest(
    pool: &SqlitePool,
    workspace_id: &str,
    skill_id: &str,
) -> AppResult<Option<SkillGate>> {
    Ok(sqlx::query_as::<_, SkillGate>(&format!(
        "SELECT {COLUMNS} FROM skill_eval_gates
         WHERE workspace_id = ? AND skill_id = ?
         ORDER BY created_at DESC, id DESC LIMIT 1"
    ))
    .bind(workspace_id)
    .bind(skill_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn decide(
    pool: &SqlitePool,
    workspace_id: &str,
    gate_id: &str,
    approved: bool,
    note: Option<&str>,
) -> AppResult<SkillGate> {
    let gate = get(pool, gate_id).await?;
    if gate.workspace_id != workspace_id {
        return Err(AppError::NotFound(format!("gate {gate_id}")));
    }
    if approved && gate.status != "PASS" {
        return Err(AppError::Conflict(format!(
            "skill {} está {}; corrija a skill ou ajuste os thresholds antes de aprovar",
            gate.skill_id, gate.status
        )));
    }
    sqlx::query(
        "UPDATE skill_eval_gates SET decision = ?, decision_note = ?,
            decided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(if approved { "APPROVED" } else { "REJECTED" })
    .bind(note)
    .bind(gate_id)
    .execute(pool)
    .await?;
    get(pool, gate_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[tokio::test]
    async fn blocks_a_skill_with_regression_and_allows_a_pass() {
        let ctx = crate::test_support::context().await;
        let skill_dir =
            std::path::Path::new(&ctx.workspace_root).join(".opencode/skills/gate-skill");
        std::fs::create_dir_all(&skill_dir).expect("skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Gate Skill\ndescription: Gate\n---\nUse a skill.\n",
        )
        .expect("skill");
        for index in 0..5 {
            let id = format!("gate-run-{index}");
            let log = ctx.state.paths.run_logs_dir.join(format!("{id}.jsonl"));
            std::fs::write(&log, "usando Gate Skill\n").expect("log");
            db::runs::create(
                &ctx.state.db,
                db::runs::CreateRunInput {
                    id: id.clone(),
                    queue_job_id: None,
                    workspace_id: ctx.workspace_id.clone(),
                    task_id: None,
                    schedule_id: None,
                    parent_run_id: None,
                    phase: Some("build".into()),
                    agent_name: "build".into(),
                    execution_mode: "BUILD_DIRECT".into(),
                    provider_id: None,
                    model_id: "provider/model".into(),
                    effort_level: "medium".into(),
                    pricing_snapshot_json: None,
                    log_path: Some(log.to_string_lossy().to_string()),
                },
            )
            .await
            .expect("run");
            db::runs::finish(
                &ctx.state.db,
                &id,
                if index < 3 { "SUCCESS" } else { "FAILED" },
                &db::runs::RunUsage::default(),
                100,
                None,
                None,
            )
            .await
            .expect("finish");
        }

        let gate = run(
            &ctx.state.db,
            &ctx.state.paths,
            &ctx.workspace_id,
            &ctx.workspace_root,
            "gate-skill",
            GateThresholds {
                days: 30,
                min_samples: 5,
                min_success_rate: 0.8,
            },
        )
        .await
        .expect("gate");
        assert_eq!(gate.status, "BLOCKED");
        assert_eq!(gate.successes, 3);
        assert_eq!(gate.failures, 2);

        assert!(
            decide(&ctx.state.db, &ctx.workspace_id, &gate.id, true, None)
                .await
                .is_err()
        );
        let rejected = decide(
            &ctx.state.db,
            &ctx.workspace_id,
            &gate.id,
            false,
            Some("regressão"),
        )
        .await
        .expect("reject");
        assert_eq!(rejected.decision, "REJECTED");

        let passing = run(
            &ctx.state.db,
            &ctx.state.paths,
            &ctx.workspace_id,
            &ctx.workspace_root,
            "gate-skill",
            GateThresholds {
                days: 30,
                min_samples: 5,
                min_success_rate: 0.5,
            },
        )
        .await
        .expect("passing");
        assert_eq!(passing.status, "PASS");
        let approved = decide(&ctx.state.db, &ctx.workspace_id, &passing.id, true, None)
            .await
            .expect("approve");
        assert_eq!(approved.decision, "APPROVED");
        assert!(
            latest(&ctx.state.db, &ctx.workspace_id, "gate-skill")
                .await
                .expect("latest")
                .is_some()
        );
    }
}
