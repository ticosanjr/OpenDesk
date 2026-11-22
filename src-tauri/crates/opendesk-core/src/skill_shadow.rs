//! Shadow replay for skills (G2) and promotion baseline (G3).
//!
//! A replay executes curated cases without tools and without writing to the
//! workspace. The shell injects the model executor; the core owns validation,
//! metrics, persistence and the regression comparison.

use crate::{
    config_sync::{paths::StudioPaths, skills},
    error::{AppError, AppResult},
    skill_cases::{self, SkillEvalCase},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::{path::Path, time::Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowThresholds {
    #[serde(default = "default_rate")]
    pub min_success_rate: f64,
    #[serde(default = "default_regression")]
    pub max_regression: f64,
}

fn default_rate() -> f64 {
    0.8
}
fn default_regression() -> f64 {
    0.1
}

impl Default for ShadowThresholds {
    fn default() -> Self {
        Self {
            min_success_rate: default_rate(),
            max_regression: default_regression(),
        }
    }
}

impl ShadowThresholds {
    fn validate(&self) -> AppResult<()> {
        if !(0.0..=1.0).contains(&self.min_success_rate)
            || !(0.0..=1.0).contains(&self.max_regression)
        {
            return Err(AppError::BadRequest(
                "thresholds do shadow precisam ficar entre 0 e 1".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ShadowOutput {
    pub output: String,
    pub tokens: i64,
    pub cost_usd: f64,
}

#[async_trait]
pub trait ShadowExecutor: Send + Sync {
    async fn execute(
        &self,
        skill: &skills::SkillFile,
        case: &SkillEvalCase,
    ) -> AppResult<ShadowOutput>;
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ShadowRun {
    pub id: String,
    pub workspace_id: String,
    pub skill_id: String,
    pub status: String,
    pub total: i64,
    pub passed: i64,
    pub failed: i64,
    pub success_rate: Option<f64>,
    pub tokens: i64,
    pub cost_usd: f64,
    pub latency_ms: i64,
    pub baseline_run_id: Option<String>,
    pub regression: Option<f64>,
    pub min_success_rate: f64,
    pub max_regression: f64,
    pub details_json: String,
    pub promoted_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ShadowResult {
    pub id: String,
    pub run_id: String,
    pub case_id: Option<String>,
    pub name: String,
    pub status: String,
    pub output: Option<String>,
    pub contains_ok: i64,
    pub schema_ok: i64,
    pub tokens: i64,
    pub latency_ms: i64,
    pub error_message: Option<String>,
    pub created_at: String,
}

const RUN_COLUMNS: &str = "id, workspace_id, skill_id, status, total, passed, failed, \
    success_rate, tokens, cost_usd, latency_ms, baseline_run_id, regression, \
    min_success_rate, max_regression, details_json, promoted_at, created_at";

pub async fn run(
    pool: &SqlitePool,
    workspace_id: &str,
    workspace_root: &Path,
    skill_id: &str,
    case_ids: Option<&[String]>,
    thresholds: ShadowThresholds,
    executor: &dyn ShadowExecutor,
) -> AppResult<ShadowRun> {
    thresholds.validate()?;
    let cases = skill_cases::list(pool, workspace_id, skill_id).await?;
    let cases: Vec<_> = cases
        .into_iter()
        .filter(|case| case_ids.is_none_or(|ids| ids.iter().any(|id| id == &case.id)))
        .collect();
    if cases.is_empty() {
        return Err(AppError::BadRequest(
            "a skill não tem casos curados para o replay sombra".into(),
        ));
    }
    let skill = load_skill(workspace_root, skill_id)?;
    let mut results = Vec::new();
    let mut tokens = 0i64;
    let mut cost = 0.0;
    let mut passed = 0i64;
    for case in &cases {
        let started = Instant::now();
        let outcome = executor.execute(&skill, case).await;
        let latency_ms = started.elapsed().as_millis().min(i64::MAX as u128) as i64;
        let (status, output, contains_ok, schema_ok, case_tokens, error) = match outcome {
            Ok(value) => {
                let contains_ok = case
                    .expected_contains
                    .as_deref()
                    .is_none_or(|needle| value.output.contains(needle));
                let schema_ok = schema_matches(case.expected_schema_json.as_deref(), &value.output);
                let ok = contains_ok && schema_ok;
                tokens += value.tokens;
                cost += value.cost_usd;
                if ok {
                    passed += 1;
                }
                (
                    if ok { "PASS" } else { "FAILED" },
                    Some(value.output),
                    contains_ok,
                    schema_ok,
                    value.tokens,
                    None,
                )
            }
            Err(error) => ("ERROR", None, false, false, 0, Some(format!("{error:#}"))),
        };
        results.push(ShadowResultDraft {
            case_id: Some(case.id.clone()),
            name: case.name.clone(),
            status,
            output,
            contains_ok,
            schema_ok,
            tokens: case_tokens,
            latency_ms,
            error,
        });
    }
    let total = results.len() as i64;
    let failed = total - passed;
    let success_rate = (total > 0).then(|| passed as f64 / total as f64);
    let previous = latest(pool, workspace_id, skill_id).await?;
    let regression = previous
        .as_ref()
        .and_then(|previous| {
            previous
                .success_rate
                .zip(success_rate)
                .map(|(before, after)| before - after)
        })
        .filter(|value| *value > 0.0);
    let blocked = success_rate.unwrap_or(0.0) < thresholds.min_success_rate
        || regression.is_some_and(|value| value > thresholds.max_regression);
    let run_id = Uuid::now_v7().to_string();
    let status = if blocked { "BLOCKED" } else { "PASS" };
    let total_latency = results.iter().map(|result| result.latency_ms).sum::<i64>();
    let details = serde_json::json!({
        "cases": total,
        "thresholds": thresholds,
        "baseline_run_id": previous.as_ref().map(|run| run.id.clone()),
    });
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO skill_shadow_runs
            (id, workspace_id, skill_id, status, total, passed, failed, success_rate,
             tokens, cost_usd, latency_ms, baseline_run_id, regression,
             min_success_rate, max_regression, details_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(workspace_id)
    .bind(skill_id)
    .bind(status)
    .bind(total)
    .bind(passed)
    .bind(failed)
    .bind(success_rate)
    .bind(tokens)
    .bind(cost)
    .bind(total_latency)
    .bind(previous.as_ref().map(|run| run.id.clone()))
    .bind(regression)
    .bind(thresholds.min_success_rate)
    .bind(thresholds.max_regression)
    .bind(details.to_string())
    .execute(&mut *tx)
    .await?;
    for result in results {
        sqlx::query(
            "INSERT INTO skill_shadow_results
                (id, run_id, case_id, name, status, output, contains_ok, schema_ok,
                 tokens, latency_ms, error_message)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(&run_id)
        .bind(result.case_id)
        .bind(result.name)
        .bind(result.status)
        .bind(result.output)
        .bind(result.contains_ok as i64)
        .bind(result.schema_ok as i64)
        .bind(result.tokens)
        .bind(result.latency_ms)
        .bind(result.error)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    get(pool, &run_id).await
}

struct ShadowResultDraft {
    case_id: Option<String>,
    name: String,
    status: &'static str,
    output: Option<String>,
    contains_ok: bool,
    schema_ok: bool,
    tokens: i64,
    latency_ms: i64,
    error: Option<String>,
}

fn load_skill(workspace_root: &Path, skill_id: &str) -> AppResult<skills::SkillFile> {
    let workspace = StudioPaths::new(Some(workspace_root.to_path_buf()));
    match skills::get(&workspace, "workspace", skill_id) {
        Ok(skill) => Ok(skill),
        Err(_) => {
            let global = StudioPaths::new(None);
            skills::get(&global, "global", skill_id).map_err(Into::into)
        }
    }
}

fn schema_matches(schema: Option<&str>, output: &str) -> bool {
    let Some(schema) = schema else { return true };
    let Ok(schema) = serde_json::from_str::<serde_json::Value>(schema) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(output) else {
        return false;
    };
    jsonschema::validator_for(&schema)
        .map(|validator| validator.is_valid(&value))
        .unwrap_or(false)
}

pub async fn latest(
    pool: &SqlitePool,
    workspace_id: &str,
    skill_id: &str,
) -> AppResult<Option<ShadowRun>> {
    Ok(sqlx::query_as::<_, ShadowRun>(&format!(
        "SELECT {RUN_COLUMNS} FROM skill_shadow_runs
         WHERE workspace_id = ? AND skill_id = ?
         ORDER BY created_at DESC, id DESC LIMIT 1"
    ))
    .bind(workspace_id)
    .bind(skill_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn list(
    pool: &SqlitePool,
    workspace_id: &str,
    skill_id: &str,
) -> AppResult<Vec<ShadowRun>> {
    Ok(sqlx::query_as::<_, ShadowRun>(&format!(
        "SELECT {RUN_COLUMNS} FROM skill_shadow_runs
         WHERE workspace_id = ? AND skill_id = ?
         ORDER BY created_at DESC LIMIT 20"
    ))
    .bind(workspace_id)
    .bind(skill_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ShadowRun> {
    sqlx::query_as::<_, ShadowRun>(&format!(
        "SELECT {RUN_COLUMNS} FROM skill_shadow_runs WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("shadow run {id}")))
}

pub async fn results(pool: &SqlitePool, run_id: &str) -> AppResult<Vec<ShadowResult>> {
    Ok(sqlx::query_as::<_, ShadowResult>(
        "SELECT id, run_id, case_id, name, status, output, contains_ok, schema_ok,
                tokens, latency_ms, error_message, created_at
         FROM skill_shadow_results WHERE run_id = ? ORDER BY created_at",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?)
}

pub async fn promote(
    pool: &SqlitePool,
    workspace_id: &str,
    run_id: &str,
    approved: bool,
    _note: Option<&str>,
) -> AppResult<ShadowRun> {
    if !approved {
        return Err(AppError::BadRequest(
            "promoção de skill exige aprovação explícita".into(),
        ));
    }
    let run = get(pool, run_id).await?;
    if run.workspace_id != workspace_id {
        return Err(AppError::NotFound(format!("shadow run {run_id}")));
    }
    if run.status != "PASS" {
        return Err(AppError::Conflict(format!(
            "shadow run {} está {}; corrija a skill ou os thresholds antes de promover",
            run.id, run.status
        )));
    }
    sqlx::query(
        "UPDATE skill_shadow_runs SET promoted_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    get(pool, run_id).await
}

/// Restores the latest backup of a skill's `SKILL.md`.
pub async fn rollback(
    workspace_root: &Path,
    backups_dir: &Path,
    skill_id: &str,
) -> AppResult<skills::SkillFile> {
    let workspace = StudioPaths::new(Some(workspace_root.to_path_buf()));
    let skill = skills::get(&workspace, "workspace", skill_id)
        .or_else(|_| skills::get(&StudioPaths::new(None), "global", skill_id))?;
    let path = std::path::PathBuf::from(&skill.path);
    let backups = crate::config_sync::file_ops::list_backups_for(&path, backups_dir)?;
    let latest = backups
        .first()
        .ok_or_else(|| AppError::NotFound(format!("nenhum backup para a skill {skill_id}")))?;
    crate::config_sync::file_ops::restore_backup(&latest.id, &path, backups_dir)?;
    skills::get(&workspace, "workspace", skill_id)
        .or_else(|_| skills::get(&StudioPaths::new(None), "global", skill_id))
        .map_err(Into::into)
}

/// Small helper for callers that want a plain success/failure list.
pub fn passed(result: &ShadowResult) -> bool {
    result.status == "PASS"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_cases::CreateSkillCase;

    struct FakeExecutor;

    #[async_trait]
    impl ShadowExecutor for FakeExecutor {
        async fn execute(
            &self,
            _skill: &skills::SkillFile,
            case: &SkillEvalCase,
        ) -> AppResult<ShadowOutput> {
            Ok(ShadowOutput {
                output: if case.name == "bad" {
                    "sem contrato".into()
                } else {
                    r#"{"status":"ok"}"#.into()
                },
                tokens: 10,
                cost_usd: 0.01,
            })
        }
    }

    async fn fixture() -> crate::test_support::TestContext {
        let ctx = crate::test_support::context().await;
        let dir = ctx.workspace_root.join(".opencode/skills/demo");
        std::fs::create_dir_all(&dir).expect("skill dir");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: Demo\ndescription: Demo\n---\nUse a skill.\n",
        )
        .expect("skill");
        skill_cases::create(
            &ctx.db,
            &ctx.workspace_id,
            "demo",
            CreateSkillCase {
                name: "ok".into(),
                prompt: "faça".into(),
                expected_contains: Some("ok".into()),
                expected_schema: Some(serde_json::json!({
                    "type": "object",
                    "required": ["status"]
                })),
            },
        )
        .await
        .expect("case");
        skill_cases::create(
            &ctx.db,
            &ctx.workspace_id,
            "demo",
            CreateSkillCase {
                name: "bad".into(),
                prompt: "faça".into(),
                expected_contains: Some("ok".into()),
                expected_schema: None,
            },
        )
        .await
        .expect("case");
        ctx
    }

    #[tokio::test]
    async fn rollback_restores_the_previous_skill_file() {
        let ctx = crate::test_support::context().await;
        let paths = StudioPaths::new(Some(ctx.workspace_root.clone()));
        skills::write(
            &paths,
            "workspace",
            "rollback-demo",
            skills::SkillWriteInput {
                name: Some("Rollback".into()),
                description: Some("v1".into()),
                body: "primeira versão".into(),
                files: Vec::new(),
                expected_sha256: None,
            },
            &ctx.paths.backups_dir,
        )
        .expect("first");
        skills::write(
            &paths,
            "workspace",
            "rollback-demo",
            skills::SkillWriteInput {
                name: Some("Rollback".into()),
                description: Some("v2".into()),
                body: "segunda versão".into(),
                files: Vec::new(),
                expected_sha256: None,
            },
            &ctx.paths.backups_dir,
        )
        .expect("second");
        let restored = rollback(&ctx.workspace_root, &ctx.paths.backups_dir, "rollback-demo")
            .await
            .expect("rollback");
        assert!(restored.body.contains("primeira versão"));
    }

    #[tokio::test]
    async fn replays_cases_and_blocks_regression() {
        let ctx = fixture().await;
        let sentinel = ctx.workspace_root.join("sentinel.txt");
        std::fs::write(&sentinel, "keep").expect("sentinel");
        let first = run(
            &ctx.db,
            &ctx.workspace_id,
            &ctx.workspace_root,
            "demo",
            None,
            ShadowThresholds {
                min_success_rate: 0.5,
                max_regression: 0.1,
            },
            &FakeExecutor,
        )
        .await
        .expect("first");
        assert_eq!(first.status, "PASS");
        assert_eq!(first.total, 2);
        assert_eq!(first.passed, 1);
        let second = run(
            &ctx.db,
            &ctx.workspace_id,
            &ctx.workspace_root,
            "demo",
            None,
            ShadowThresholds {
                min_success_rate: 0.9,
                max_regression: 0.1,
            },
            &FakeExecutor,
        )
        .await
        .expect("second");
        assert_eq!(second.status, "BLOCKED");
        assert!(
            promote(&ctx.db, &ctx.workspace_id, &second.id, true, None)
                .await
                .is_err()
        );
        assert!(
            promote(&ctx.db, &ctx.workspace_id, &first.id, false, None)
                .await
                .is_err()
        );
        assert!(
            !results(&ctx.db, &first.id)
                .await
                .expect("results")
                .is_empty()
        );
    }
}
