use crate::error::{AppError, AppResult};
use serde_json::json;
use sqlx::SqlitePool;
use std::path::Path;
use uuid::Uuid;

use super::memory::remember;
use super::{LESSON_COLUMNS, LearningLesson, ManualLessonInput};

/// Saves an explicit learning from the agent or UI. This is the default
/// destination for "memorize this" requests; personal context has its own,
/// explicitly gated path.
pub async fn save_manual(
    pool: &SqlitePool,
    workspace_root: &Path,
    backups_dir: &Path,
    workspace_id: &str,
    input: ManualLessonInput,
) -> AppResult<LearningLesson> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest(
            "título do aprendizado é obrigatório".into(),
        ));
    }
    let detail = input.detail.trim();
    if detail.is_empty() {
        return Err(AppError::BadRequest(
            "detalhe do aprendizado é obrigatório".into(),
        ));
    }
    if title.chars().count() > 200 || detail.chars().count() > 8_000 {
        return Err(AppError::BadRequest(
            "aprendizado muito longo (título 200, detalhe 8000)".into(),
        ));
    }
    let fingerprint = manual_fingerprint(title, detail);
    let id = Uuid::now_v7().to_string();
    let category = input
        .category
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("manual");
    let severity = input
        .severity
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("info");
    let agent_name = input
        .agent_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("agent");
    let evidence = json!({
        "source": "mcp",
        "requested_by": agent_name,
    })
    .to_string();
    sqlx::query(
        "INSERT INTO learning_lessons (
            id, workspace_id, agent_name, category, severity, title, detail,
            evidence_json, fingerprint, occurrences, status, last_seen_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 'OPEN',
            strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ON CONFLICT(workspace_id, fingerprint) DO UPDATE SET
            title = excluded.title,
            detail = excluded.detail,
            category = excluded.category,
            severity = excluded.severity,
            evidence_json = excluded.evidence_json,
            occurrences = learning_lessons.occurrences + 1,
            last_seen_at = excluded.last_seen_at,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(agent_name)
    .bind(category)
    .bind(severity)
    .bind(title)
    .bind(detail)
    .bind(&evidence)
    .bind(&fingerprint)
    .execute(pool)
    .await?;
    let lesson = sqlx::query_as::<_, LearningLesson>(&format!(
        "SELECT {LESSON_COLUMNS} FROM learning_lessons
         WHERE workspace_id = ? AND fingerprint = ?"
    ))
    .bind(workspace_id)
    .bind(&fingerprint)
    .fetch_one(pool)
    .await?;
    if input.remember {
        return remember(pool, workspace_root, backups_dir, workspace_id, &lesson.id).await;
    }
    Ok(lesson)
}

fn manual_fingerprint(title: &str, detail: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    title.to_lowercase().hash(&mut hasher);
    detail.to_lowercase().hash(&mut hasher);
    format!("manual:{:x}", hasher.finish())
}
