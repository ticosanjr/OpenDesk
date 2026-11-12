//! Lightweight continuous learning: deterministic lessons derived from run
//! outcomes. Nothing is installed automatically; promoting a lesson creates a
//! normal Studio proposal that still needs human approval.

use crate::{
    config_sync::file_ops,
    db::{
        proposals::{self, ConfigProposal, ProposalInput},
        runs::{self, TaskRun},
        settings,
    },
    error::{AppError, AppResult},
    queue_scheduler::settlement::classify_run_error,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, SqlitePool};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use uuid::Uuid;

const SETTINGS_PREFIX: &str = "learning:";
const DETAIL_LIMIT: usize = 400;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSettings {
    pub enabled: bool,
    pub capture_successes: bool,
    pub auto_propose: bool,
    pub min_occurrences: i64,
}

impl Default for LearningSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_successes: false,
            auto_propose: false,
            min_occurrences: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LearningLesson {
    pub id: String,
    pub workspace_id: String,
    pub run_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_name: Option<String>,
    pub model_id: Option<String>,
    pub category: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub evidence_json: String,
    pub fingerprint: String,
    pub occurrences: i64,
    pub status: String,
    pub proposal_id: Option<String>,
    pub remembered_at: Option<String>,
    pub last_seen_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningStats {
    pub total: i64,
    pub open: i64,
    pub promoted: i64,
    pub dismissed: i64,
    pub runs_analyzed: i64,
    pub failed_runs: i64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub analyzed: i64,
    pub lessons: i64,
}

struct Candidate {
    category: &'static str,
    severity: &'static str,
    title: String,
    detail: String,
    evidence: serde_json::Value,
    fingerprint: String,
}

const LESSON_COLUMNS: &str = "id, workspace_id, run_id, task_id, agent_name, model_id, category, \
    severity, title, detail, evidence_json, fingerprint, occurrences, status, proposal_id, \
    remembered_at, last_seen_at, created_at, updated_at";

fn settings_key(workspace_id: &str) -> String {
    format!("{SETTINGS_PREFIX}{workspace_id}")
}

pub async fn get_settings(pool: &SqlitePool, workspace_id: &str) -> AppResult<LearningSettings> {
    let raw = settings::get(pool, &settings_key(workspace_id)).await?;
    let Some(value) = raw else {
        return Ok(LearningSettings::default());
    };
    let mut parsed: LearningSettings = serde_json::from_value(value).unwrap_or_default();
    parsed.min_occurrences = parsed.min_occurrences.clamp(1, 20);
    Ok(parsed)
}

pub async fn set_settings(
    pool: &SqlitePool,
    workspace_id: &str,
    mut settings: LearningSettings,
) -> AppResult<LearningSettings> {
    settings.min_occurrences = settings.min_occurrences.clamp(1, 20);
    let value = serde_json::to_value(&settings)
        .map_err(|error| AppError::Internal(format!("invalid learning settings: {error}")))?;
    settings::set(pool, &settings_key(workspace_id), &value).await?;
    Ok(settings)
}

fn truncate(value: &str, limit: usize) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= limit {
        collapsed
    } else {
        let mut out: String = collapsed.chars().take(limit).collect();
        out.push('…');
        out
    }
}

fn error_label(code: &str) -> &'static str {
    match code {
        "engine_offline" => "motor offline",
        "auth_failed" => "autenticação",
        "model_not_found" => "modelo indisponível",
        "agent_not_found" => "agente ausente",
        "rate_limited" => "limite do provedor",
        "timeout" => "timeout",
        "budget_blocked" => "budget bloqueado",
        "binary_missing" => "binário ausente",
        _ => "erro de execução",
    }
}

fn failure_candidate(run: &TaskRun) -> Option<Candidate> {
    let message = run
        .error_message
        .as_deref()
        .or(run.summary_response.as_deref())
        .unwrap_or("falha sem mensagem");
    let (code, hint) = classify_run_error(message);
    if code == "cancelled" {
        return None;
    }
    let detail = format!(
        "{hint}\n\nÚltimo erro: {}\nAgente: {} · Modelo: {}",
        truncate(message, DETAIL_LIMIT),
        run.agent_name,
        run.model_id
    );
    Some(Candidate {
        category: "failure_pattern",
        severity: "warning",
        title: format!("Falha recorrente: {}", error_label(code)),
        detail,
        evidence: json!({
            "run_id": run.id,
            "error_code": code,
            "error_message": truncate(message, DETAIL_LIMIT),
            "agent_name": run.agent_name,
            "model_id": run.model_id,
            "duration_ms": run.duration_ms,
            "estimated_cost_usd": run.estimated_cost_usd,
        }),
        fingerprint: format!("failure:{code}:{}:{}", run.agent_name, run.model_id),
    })
}

fn success_candidate(run: &TaskRun) -> Option<Candidate> {
    let summary = run.summary_response.as_deref().unwrap_or("").trim();
    if summary.is_empty() {
        return None;
    }
    Some(Candidate {
        category: "success_pattern",
        severity: "info",
        title: format!("Padrão eficaz: {} + {}", run.agent_name, run.model_id),
        detail: format!(
            "Execuções com este agente e modelo estão concluindo com sucesso.\n\nResumo: {}",
            truncate(summary, DETAIL_LIMIT)
        ),
        evidence: json!({
            "run_id": run.id,
            "agent_name": run.agent_name,
            "model_id": run.model_id,
            "duration_ms": run.duration_ms,
            "estimated_cost_usd": run.estimated_cost_usd,
            "summary": truncate(summary, DETAIL_LIMIT),
        }),
        fingerprint: format!("success:{}:{}", run.agent_name, run.model_id),
    })
}

async fn already_captured(pool: &SqlitePool, run_id: &str) -> AppResult<bool> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM learning_run_captures WHERE run_id = ?")
            .bind(run_id)
            .fetch_optional(pool)
            .await?;
    Ok(found.is_some())
}

async fn upsert_lesson(
    pool: &SqlitePool,
    workspace_id: &str,
    run: &TaskRun,
    candidate: &Candidate,
) -> AppResult<LearningLesson> {
    let id = Uuid::now_v7().to_string();
    let query = format!(
        "INSERT INTO learning_lessons (
            id, workspace_id, run_id, task_id, agent_name, model_id, category, severity,
            title, detail, evidence_json, fingerprint, occurrences, status, last_seen_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 'OPEN',
            strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ON CONFLICT(workspace_id, fingerprint) DO UPDATE SET
            occurrences = learning_lessons.occurrences + 1,
            run_id = excluded.run_id,
            task_id = excluded.task_id,
            agent_name = excluded.agent_name,
            model_id = excluded.model_id,
            title = excluded.title,
            detail = excluded.detail,
            evidence_json = excluded.evidence_json,
            last_seen_at = excluded.last_seen_at,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         RETURNING {LESSON_COLUMNS}"
    );
    Ok(sqlx::query_as::<_, LearningLesson>(&query)
        .bind(&id)
        .bind(workspace_id)
        .bind(&run.id)
        .bind(run.task_id.as_deref())
        .bind(&run.agent_name)
        .bind(&run.model_id)
        .bind(candidate.category)
        .bind(candidate.severity)
        .bind(&candidate.title)
        .bind(&candidate.detail)
        .bind(candidate.evidence.to_string())
        .bind(&candidate.fingerprint)
        .fetch_one(pool)
        .await?)
}

async fn mark_captured(
    pool: &SqlitePool,
    workspace_id: &str,
    run_id: &str,
    lesson_id: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO learning_run_captures (run_id, workspace_id, lesson_id)
         VALUES (?, ?, ?)",
    )
    .bind(run_id)
    .bind(workspace_id)
    .bind(lesson_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Observes a settled run. Returns the affected lesson when one was created or
/// reinforced. Already-observed runs are skipped, so manual scans are safe.
pub async fn capture_run(pool: &SqlitePool, run_id: &str) -> AppResult<Option<LearningLesson>> {
    let run = runs::get(pool, run_id).await?;
    let settings = get_settings(pool, &run.workspace_id).await?;
    if !settings.enabled || already_captured(pool, run_id).await? {
        return Ok(None);
    }
    let candidate = match run.status.as_str() {
        "FAILED" => failure_candidate(&run),
        "SUCCESS" if settings.capture_successes => success_candidate(&run),
        _ => None,
    };
    let Some(candidate) = candidate else {
        return Ok(None);
    };
    let mut lesson = upsert_lesson(pool, &run.workspace_id, &run, &candidate).await?;
    mark_captured(pool, &run.workspace_id, &run.id, &lesson.id).await?;

    if settings.auto_propose
        && lesson.status == "OPEN"
        && lesson.proposal_id.is_none()
        && lesson.occurrences >= settings.min_occurrences
        && let Ok(proposal) =
            promote(pool, &lesson.workspace_id, &lesson.id, Some("learning")).await
    {
        lesson = get_lesson(pool, &lesson.workspace_id, &lesson.id).await?;
        tracing::info!(
            lesson_id = %lesson.id,
            proposal_id = %proposal.id,
            "learning lesson promoted automatically"
        );
    }
    Ok(Some(lesson))
}

pub async fn scan(pool: &SqlitePool, workspace_id: &str, limit: i64) -> AppResult<ScanResult> {
    let runs = runs::list_with_limit(pool, workspace_id, None, limit.clamp(1, 200)).await?;
    let mut analyzed = 0;
    let mut lessons = 0;
    for run in runs {
        analyzed += 1;
        if capture_run(pool, &run.id).await?.is_some() {
            lessons += 1;
        }
    }
    Ok(ScanResult { analyzed, lessons })
}

pub async fn list_lessons(
    pool: &SqlitePool,
    workspace_id: &str,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<LearningLesson>> {
    let status = status.map(str::trim).filter(|value| !value.is_empty());
    if let Some(value) = status
        && !matches!(value, "OPEN" | "DISMISSED" | "PROMOTED")
    {
        return Err(AppError::BadRequest(format!(
            "invalid lesson status '{value}'"
        )));
    }
    let query = format!(
        "SELECT {LESSON_COLUMNS} FROM learning_lessons
         WHERE workspace_id = ? AND (? IS NULL OR status = ?)
         ORDER BY CASE status WHEN 'OPEN' THEN 0 WHEN 'PROMOTED' THEN 1 ELSE 2 END,
                  last_seen_at DESC
         LIMIT ?"
    );
    Ok(sqlx::query_as::<_, LearningLesson>(&query)
        .bind(workspace_id)
        .bind(status)
        .bind(status)
        .bind(limit.clamp(1, 500))
        .fetch_all(pool)
        .await?)
}

pub async fn get_lesson(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
) -> AppResult<LearningLesson> {
    let query =
        format!("SELECT {LESSON_COLUMNS} FROM learning_lessons WHERE id = ? AND workspace_id = ?");
    sqlx::query_as::<_, LearningLesson>(&query)
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("lesson {id}")))
}

pub async fn dismiss(pool: &SqlitePool, workspace_id: &str, id: &str) -> AppResult<LearningLesson> {
    let lesson = get_lesson(pool, workspace_id, id).await?;
    if lesson.status != "OPEN" {
        return Err(AppError::Conflict("lição já resolvida".into()));
    }
    sqlx::query(
        "UPDATE learning_lessons SET status = 'DISMISSED',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND workspace_id = ?",
    )
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    get_lesson(pool, workspace_id, id).await
}

fn skill_slug(lesson: &LearningLesson) -> String {
    let mut hasher = DefaultHasher::new();
    lesson.fingerprint.hash(&mut hasher);
    let hash = format!("{:08x}", hasher.finish());
    format!(
        "learned-{}-{}",
        lesson.category.replace('_', "-"),
        &hash[..8]
    )
}

fn skill_payload(lesson: &LearningLesson) -> serde_json::Value {
    let evidence: serde_json::Value =
        serde_json::from_str(&lesson.evidence_json).unwrap_or_else(|_| json!({}));
    let run = evidence
        .get("run_id")
        .and_then(|value| value.as_str())
        .unwrap_or("desconhecido");
    let agent = lesson.agent_name.as_deref().unwrap_or("router");
    let model = lesson.model_id.as_deref().unwrap_or("modelo padrão");
    let mut steps = vec![
        "Reproduza o cenário do último run citado na evidência.".to_string(),
        format!("Confirme o agente ({agent}) e o modelo ({model}) usados."),
        "Aplique a verificação descrita abaixo antes de reenfileirar a tarefa.".to_string(),
        "Se o padrão não se repetir, descarte esta skill no Studio.".to_string(),
    ];
    if lesson.category == "failure_pattern" {
        steps.insert(
            1,
            "Leia a mensagem de erro e o diagnóstico da execução.".to_string(),
        );
    } else {
        steps.insert(
            1,
            "Reaproveite a estrutura do resumo bem-sucedido como referência.".to_string(),
        );
    }
    let body = format!(
        "## Quando usar\n\n{} (observado {} vez(es)).\n\n{}\n\n## Passos\n\n{}\n\n## Evidência\n\n- Run: `{run}`\n- Categoria: `{}`\n- Fingerprint: `{}`\n",
        lesson.title,
        lesson.occurrences,
        lesson.detail,
        steps
            .iter()
            .enumerate()
            .map(|(index, step)| format!("{}. {step}", index + 1))
            .collect::<Vec<_>>()
            .join("\n"),
        lesson.category,
        lesson.fingerprint
    );
    json!({
        "id": skill_slug(lesson),
        "scope": "workspace",
        "name": lesson.title,
        "description": format!(
            "Lição aprendida a partir de execuções: {}.",
            truncate(&lesson.title, 120)
        ),
        "body": body,
        "files": [],
    })
}

/// Promotes a lesson into the existing Studio proposal flow. The file is only
/// written when a human approves the proposal.
pub async fn promote(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
    requested_by: Option<&str>,
) -> AppResult<ConfigProposal> {
    let lesson = get_lesson(pool, workspace_id, id).await?;
    if let Some(proposal_id) = &lesson.proposal_id {
        return proposals::get(pool, proposal_id).await;
    }
    if lesson.status == "DISMISSED" {
        return Err(AppError::Conflict(
            "lição descartada não pode ser promovida".into(),
        ));
    }
    let payload = skill_payload(&lesson);
    let proposal = proposals::create(
        pool,
        workspace_id,
        ProposalInput {
            kind: "SKILL".into(),
            action: "CREATE".into(),
            title: format!("Skill aprendida: {}", lesson.title),
            description: Some(
                "Gerada pelo loop de aprendizado a partir de execuções reais. Revise antes de aplicar."
                    .into(),
            ),
            payload,
            requested_by: requested_by.map(str::to_string),
            source_run_id: lesson.run_id.clone(),
            source_task_id: lesson.task_id.clone(),
        },
    )
    .await?;
    sqlx::query(
        "UPDATE learning_lessons SET status = 'PROMOTED', proposal_id = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND workspace_id = ?",
    )
    .bind(&proposal.id)
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    Ok(proposal)
}

/// Appends a compact, marked block to the workspace `MEMORY.md`. The marker
/// makes the operation idempotent and lets the user remove the lesson later.
pub async fn remember(
    pool: &SqlitePool,
    workspace_root: &Path,
    backups_dir: &Path,
    workspace_id: &str,
    id: &str,
) -> AppResult<LearningLesson> {
    let lesson = get_lesson(pool, workspace_id, id).await?;
    let dir = workspace_root.join(".opencode");
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;
    let path = dir.join("MEMORY.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let marker = format!("<!-- learning:{} -->", lesson.fingerprint);
    let mut content = existing.trim_end().to_string();
    if !content.contains(&marker) {
        content.push_str(&format!(
            "\n\n## Aprendizado: {}\n\n{marker}\n\n- {}\n- Observado {} vez(es) · {} · {}\n<!-- /learning:{} -->\n",
            lesson.title,
            truncate(&lesson.detail, 500),
            lesson.occurrences,
            lesson.agent_name.as_deref().unwrap_or("router"),
            lesson.model_id.as_deref().unwrap_or("modelo padrão"),
            lesson.fingerprint,
        ));
        file_ops::write_text_atomic(&path, &content, None, backups_dir)?;
    }
    sqlx::query(
        "UPDATE learning_lessons SET
            remembered_at = COALESCE(remembered_at, strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND workspace_id = ?",
    )
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    get_lesson(pool, workspace_id, id).await
}

pub async fn restore(pool: &SqlitePool, workspace_id: &str, id: &str) -> AppResult<LearningLesson> {
    let lesson = get_lesson(pool, workspace_id, id).await?;
    if lesson.status != "DISMISSED" {
        return Err(AppError::Conflict(
            "apenas lições descartadas podem ser reabertas".into(),
        ));
    }
    sqlx::query(
        "UPDATE learning_lessons SET status = 'OPEN',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND workspace_id = ?",
    )
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    get_lesson(pool, workspace_id, id).await
}

/// Removes the marked block from `MEMORY.md` and clears `remembered_at`.
/// Supports both the current block format and the older heading-only format.
pub async fn forget(
    pool: &SqlitePool,
    workspace_root: &Path,
    backups_dir: &Path,
    workspace_id: &str,
    id: &str,
) -> AppResult<LearningLesson> {
    let lesson = get_lesson(pool, workspace_id, id).await?;
    let path = workspace_root.join(".opencode").join("MEMORY.md");
    if path.exists() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let marker = format!("<!-- learning:{} -->", lesson.fingerprint);
        let end_marker = format!("<!-- /learning:{} -->", lesson.fingerprint);
        if let Some(marker_idx) = existing.find(&marker) {
            let start = existing[..marker_idx]
                .rfind("## Aprendizado:")
                .unwrap_or(marker_idx);
            let end = existing[marker_idx..]
                .find(&end_marker)
                .map(|offset| marker_idx + offset + end_marker.len())
                .or_else(|| {
                    existing[marker_idx + marker.len()..]
                        .find("\n## ")
                        .map(|offset| marker_idx + marker.len() + offset + 1)
                })
                .unwrap_or(existing.len());
            let cleaned = format!("{}{}", &existing[..start], &existing[end..]);
            crate::config_sync::file_ops::write_text_atomic(&path, &cleaned, None, backups_dir)?;
        }
    }
    sqlx::query(
        "UPDATE learning_lessons SET remembered_at = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND workspace_id = ?",
    )
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    get_lesson(pool, workspace_id, id).await
}

pub async fn stats(pool: &SqlitePool, workspace_id: &str) -> AppResult<LearningStats> {
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM learning_lessons WHERE workspace_id = ?")
            .bind(workspace_id)
            .fetch_one(pool)
            .await?;
    let open: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM learning_lessons WHERE workspace_id = ? AND status = 'OPEN'",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?;
    let promoted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM learning_lessons WHERE workspace_id = ? AND status = 'PROMOTED'",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?;
    let dismissed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM learning_lessons WHERE workspace_id = ? AND status = 'DISMISSED'",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?;
    let (runs_analyzed, failed_runs): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(status = 'FAILED'), 0) FROM task_runs
         WHERE workspace_id = ?
           AND started_at >= strftime('%Y-%m-%dT%H:%M:%fZ','now','-30 days')",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?;
    let completed = runs_analyzed.saturating_sub(failed_runs);
    let success_rate = if runs_analyzed > 0 {
        completed as f64 / runs_analyzed as f64
    } else {
        0.0
    };
    Ok(LearningStats {
        total,
        open,
        promoted,
        dismissed,
        runs_analyzed,
        failed_runs,
        success_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::runs::{CreateRunInput, RunUsage};
    use crate::test_support;

    async fn seed_run(
        ctx: &test_support::TestContext,
        id: &str,
        status: &str,
        error: Option<(&str, &str, &str)>,
        summary: Option<&str>,
    ) {
        runs::create(
            &ctx.state.db,
            CreateRunInput {
                id: id.into(),
                queue_job_id: None,
                workspace_id: ctx.workspace_id.clone(),
                task_id: None,
                schedule_id: None,
                parent_run_id: None,
                phase: None,
                agent_name: "build".into(),
                execution_mode: "build".into(),
                provider_id: Some("opencode-go".into()),
                model_id: "glm-5.3".into(),
                effort_level: "high".into(),
                pricing_snapshot_json: None,
                log_path: None,
            },
        )
        .await
        .expect("run");
        runs::finish(
            &ctx.state.db,
            id,
            status,
            &RunUsage::default(),
            1200,
            summary,
            None,
        )
        .await
        .expect("finish");
        if let Some((code, message, hint)) = error {
            runs::set_error(&ctx.state.db, id, code, message, hint)
                .await
                .expect("error");
        }
    }

    #[tokio::test]
    async fn capture_deduplicates_by_fingerprint_and_is_idempotent() {
        let ctx = test_support::context().await;
        seed_run(
            &ctx,
            "run-1",
            "FAILED",
            Some(("engine_offline", "OpenCode daemon is offline", "hint")),
            None,
        )
        .await;
        let first = capture_run(&ctx.state.db, "run-1")
            .await
            .expect("capture")
            .expect("lesson");
        assert_eq!(first.occurrences, 1);
        assert_eq!(first.category, "failure_pattern");

        // Re-capturing the same run is a no-op.
        assert!(
            capture_run(&ctx.state.db, "run-1")
                .await
                .expect("capture")
                .is_none()
        );

        seed_run(
            &ctx,
            "run-2",
            "FAILED",
            Some((
                "engine_offline",
                "OpenCode daemon is offline (attempt 2)",
                "hint",
            )),
            None,
        )
        .await;
        let second = capture_run(&ctx.state.db, "run-2")
            .await
            .expect("capture")
            .expect("lesson");
        assert_eq!(second.id, first.id);
        assert_eq!(second.occurrences, 2);
    }

    #[tokio::test]
    async fn promote_creates_pending_skill_proposal() {
        let ctx = test_support::context().await;
        seed_run(
            &ctx,
            "run-3",
            "FAILED",
            Some(("timeout", "timed out after 60s", "hint")),
            None,
        )
        .await;
        let lesson = capture_run(&ctx.state.db, "run-3")
            .await
            .expect("capture")
            .expect("lesson");
        let proposal = promote(&ctx.state.db, &ctx.workspace_id, &lesson.id, Some("test"))
            .await
            .expect("promote");
        assert_eq!(proposal.kind, "SKILL");
        assert_eq!(proposal.status, "PENDING");
        assert_eq!(proposal.action, "CREATE");
        let payload: serde_json::Value = serde_json::from_str(&proposal.payload_json).unwrap();
        assert!(
            payload["id"]
                .as_str()
                .unwrap_or_default()
                .starts_with("learned-failure-pattern-")
        );

        let promoted = get_lesson(&ctx.state.db, &ctx.workspace_id, &lesson.id)
            .await
            .expect("lesson");
        assert_eq!(promoted.status, "PROMOTED");
        assert_eq!(promoted.proposal_id.as_deref(), Some(proposal.id.as_str()));

        // Calling promote again returns the same proposal.
        let again = promote(&ctx.state.db, &ctx.workspace_id, &lesson.id, None)
            .await
            .expect("again");
        assert_eq!(again.id, proposal.id);
    }

    #[tokio::test]
    async fn settings_and_dismiss_gate_the_loop() {
        let ctx = test_support::context().await;
        let settings = set_settings(
            &ctx.state.db,
            &ctx.workspace_id,
            LearningSettings {
                enabled: false,
                ..LearningSettings::default()
            },
        )
        .await
        .expect("settings");
        assert!(!settings.enabled);
        seed_run(
            &ctx,
            "run-4",
            "FAILED",
            Some(("timeout", "timeout", "hint")),
            None,
        )
        .await;
        assert!(
            capture_run(&ctx.state.db, "run-4")
                .await
                .expect("capture")
                .is_none()
        );

        set_settings(
            &ctx.state.db,
            &ctx.workspace_id,
            LearningSettings::default(),
        )
        .await
        .expect("settings");
        let lesson = capture_run(&ctx.state.db, "run-4")
            .await
            .expect("capture")
            .expect("lesson");
        let dismissed = dismiss(&ctx.state.db, &ctx.workspace_id, &lesson.id)
            .await
            .expect("dismiss");
        assert_eq!(dismissed.status, "DISMISSED");
        assert!(
            promote(&ctx.state.db, &ctx.workspace_id, &lesson.id, None)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn remember_writes_memory_once() {
        let ctx = test_support::context().await;
        seed_run(
            &ctx,
            "run-memory",
            "FAILED",
            Some(("timeout", "timed out after 60s", "hint")),
            None,
        )
        .await;
        let lesson = capture_run(&ctx.state.db, "run-memory")
            .await
            .expect("capture")
            .expect("lesson");
        let first = remember(
            &ctx.state.db,
            &ctx.workspace_root,
            &ctx.state.paths.backups_dir,
            &ctx.workspace_id,
            &lesson.id,
        )
        .await
        .expect("remember");
        assert!(first.remembered_at.is_some());
        let path = ctx.workspace_root.join(".opencode/MEMORY.md");
        let content = std::fs::read_to_string(&path).expect("memory");
        assert!(
            content.contains("## Aprendizado: Falha recorrente: timeout"),
            "content: {content}"
        );
        assert!(content.contains(&format!("<!-- learning:{} -->", lesson.fingerprint)));

        remember(
            &ctx.state.db,
            &ctx.workspace_root,
            &ctx.state.paths.backups_dir,
            &ctx.workspace_id,
            &lesson.id,
        )
        .await
        .expect("remember again");
        let content = std::fs::read_to_string(&path).expect("memory");
        assert_eq!(
            content
                .matches(&format!("<!-- learning:{} -->", lesson.fingerprint))
                .count(),
            1,
            "memory block must be idempotent"
        );
    }

    #[tokio::test]
    async fn stats_reports_recent_runs_and_lessons() {
        let ctx = test_support::context().await;
        seed_run(&ctx, "run-5", "SUCCESS", None, Some("ok")).await;
        seed_run(
            &ctx,
            "run-6",
            "FAILED",
            Some(("timeout", "timeout", "hint")),
            None,
        )
        .await;
        capture_run(&ctx.state.db, "run-6")
            .await
            .expect("capture")
            .expect("lesson");
        let stats = stats(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("stats");
        assert_eq!(stats.total, 1);
        assert_eq!(stats.open, 1);
        assert_eq!(stats.runs_analyzed, 2);
        assert_eq!(stats.failed_runs, 1);
        assert!((stats.success_rate - 0.5).abs() < f64::EPSILON);
    }
}
