use crate::{
    db::runs::{self, TaskRun},
    error::AppResult,
    queue_scheduler::settlement::classify_run_error,
};
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::lessons::get_lesson;
use super::proposals::promote;
use super::settings::get_settings;
use super::{LESSON_COLUMNS, LearningLesson, ScanResult};

const DETAIL_LIMIT: usize = 400;

pub(super) struct Candidate {
    pub category: &'static str,
    pub severity: &'static str,
    pub title: String,
    pub detail: String,
    pub evidence: serde_json::Value,
    pub fingerprint: String,
}

pub(super) fn truncate(value: &str, limit: usize) -> String {
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
