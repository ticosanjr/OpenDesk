use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

use super::{LESSON_COLUMNS, LearningLesson, LearningStats};

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
