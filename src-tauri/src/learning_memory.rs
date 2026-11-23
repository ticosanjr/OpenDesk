use crate::{
    config_sync::file_ops,
    error::{AppError, AppResult},
};
use sqlx::SqlitePool;
use std::path::Path;

use super::LearningLesson;
use super::capture::truncate;
use super::lessons::get_lesson;

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
