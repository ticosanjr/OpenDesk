use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArtifactProvenance {
    pub id: String,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub run_id: String,
    pub file_path: String,
    pub absolute_path: String,
    pub file_type: String,
    pub file_size_bytes: i64,
    pub content_sha256: Option<String>,
    pub previous_sha256: Option<String>,
    pub action_type: String,
    pub created_by_agent: String,
    pub creation_reason: String,
    pub diff_patch: Option<String>,
    pub google_doc_url: Option<String>,
    pub google_drive_url: Option<String>,
    pub emailed_to: Option<String>,
    pub created_at: String,
    #[sqlx(default)]
    pub pinned: bool,
}

#[derive(Debug, Clone)]
pub struct CreateArtifactInput {
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub run_id: String,
    pub file_path: String,
    pub absolute_path: String,
    pub file_type: String,
    pub file_size_bytes: i64,
    pub content_sha256: Option<String>,
    pub previous_sha256: Option<String>,
    pub action_type: String,
    pub created_by_agent: String,
    pub creation_reason: String,
    pub diff_patch: Option<String>,
}

const COLUMNS: &str = "id, workspace_id, task_id, run_id, file_path, absolute_path, file_type, \
    file_size_bytes, content_sha256, previous_sha256, action_type, created_by_agent, \
    creation_reason, diff_patch, google_doc_url, google_drive_url, emailed_to, created_at, pinned";

pub async fn create(
    pool: &SqlitePool,
    input: CreateArtifactInput,
) -> AppResult<ArtifactProvenance> {
    if !matches!(
        input.action_type.as_str(),
        "CREATED" | "MODIFIED" | "DELETED"
    ) {
        return Err(AppError::BadRequest(format!(
            "invalid artifact action_type '{}'",
            input.action_type
        )));
    }

    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO artifacts_provenance (
            id, workspace_id, task_id, run_id, file_path, absolute_path, file_type,
            file_size_bytes, content_sha256, previous_sha256, action_type,
            created_by_agent, creation_reason, diff_patch
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(input.task_id.as_deref())
    .bind(&input.run_id)
    .bind(&input.file_path)
    .bind(&input.absolute_path)
    .bind(&input.file_type)
    .bind(input.file_size_bytes)
    .bind(input.content_sha256.as_deref())
    .bind(input.previous_sha256.as_deref())
    .bind(&input.action_type)
    .bind(&input.created_by_agent)
    .bind(&input.creation_reason)
    .bind(input.diff_patch.as_deref())
    .execute(pool)
    .await?;

    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ArtifactProvenance> {
    let query = format!("SELECT {COLUMNS} FROM artifacts_provenance WHERE id = ?");
    sqlx::query_as::<_, ArtifactProvenance>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("artifact {id}")))
}

pub async fn list(
    pool: &SqlitePool,
    workspace_id: &str,
    agent: Option<&str>,
    task_id: Option<&str>,
    action: Option<&str>,
    limit: i64,
) -> AppResult<Vec<ArtifactProvenance>> {
    let query = format!(
        "SELECT {COLUMNS} FROM artifacts_provenance
         WHERE workspace_id = ?
           AND (? IS NULL OR created_by_agent = ?)
           AND (? IS NULL OR task_id = ?)
           AND (? IS NULL OR action_type = ?)
         ORDER BY created_at DESC
         LIMIT ?"
    );
    Ok(sqlx::query_as::<_, ArtifactProvenance>(&query)
        .bind(workspace_id)
        .bind(agent)
        .bind(agent)
        .bind(task_id)
        .bind(task_id)
        .bind(action)
        .bind(action)
        .bind(limit.clamp(1, 1000))
        .fetch_all(pool)
        .await?)
}

pub async fn list_by_run(
    pool: &SqlitePool,
    run_id: &str,
    limit: i64,
) -> AppResult<Vec<ArtifactProvenance>> {
    let query = format!(
        "SELECT {COLUMNS} FROM artifacts_provenance
         WHERE run_id = ? ORDER BY created_at ASC LIMIT ?"
    );
    Ok(sqlx::query_as::<_, ArtifactProvenance>(&query)
        .bind(run_id)
        .bind(limit.clamp(1, 500))
        .fetch_all(pool)
        .await?)
}

pub async fn list_pinned(
    pool: &SqlitePool,
    workspace_id: &str,
    limit: i64,
) -> AppResult<Vec<ArtifactProvenance>> {
    let query = format!(
        "SELECT {COLUMNS} FROM artifacts_provenance
         WHERE workspace_id = ? AND pinned = 1
         ORDER BY created_at DESC LIMIT ?"
    );
    Ok(sqlx::query_as::<_, ArtifactProvenance>(&query)
        .bind(workspace_id)
        .bind(limit.clamp(1, 200))
        .fetch_all(pool)
        .await?)
}

pub async fn set_pinned(
    pool: &SqlitePool,
    id: &str,
    pinned: bool,
) -> AppResult<ArtifactProvenance> {
    sqlx::query("UPDATE artifacts_provenance SET pinned = ? WHERE id = ?")
        .bind(pinned)
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}

pub async fn exists(pool: &SqlitePool, run_id: &str, file_path: &str) -> AppResult<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM artifacts_provenance WHERE run_id = ? AND file_path = ?",
    )
    .bind(run_id)
    .bind(file_path)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

/// Latest record for a workspace-relative path, used by the output-directory
/// scan to decide between CREATED, MODIFIED and no-op.
pub async fn latest_for_path(
    pool: &SqlitePool,
    workspace_id: &str,
    file_path: &str,
) -> AppResult<Option<ArtifactProvenance>> {
    let query = format!(
        "SELECT {COLUMNS} FROM artifacts_provenance
         WHERE workspace_id = ? AND file_path = ?
         ORDER BY created_at DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, ArtifactProvenance>(&query)
        .bind(workspace_id)
        .bind(file_path)
        .fetch_optional(pool)
        .await?)
}

#[allow(dead_code)]
pub async fn count_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM artifacts_provenance WHERE task_id = ?")
            .bind(task_id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn set_destinations(
    pool: &SqlitePool,
    id: &str,
    google_doc_url: Option<&str>,
    google_drive_url: Option<&str>,
    emailed_to: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE artifacts_provenance SET
            google_doc_url = COALESCE(?, google_doc_url),
            google_drive_url = COALESCE(?, google_drive_url),
            emailed_to = COALESCE(?, emailed_to)
         WHERE id = ?",
    )
    .bind(google_doc_url)
    .bind(google_drive_url)
    .bind(emailed_to)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("artifacts.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        sqlx::query(
            "INSERT INTO tasks (id, workspace_id, title, description) VALUES ('task-1', 'ws-1', 'Task', 'Desc')",
        )
        .execute(&pool)
        .await
        .expect("task");
        sqlx::query(
            "INSERT INTO task_runs (id, workspace_id, task_id, agent_name, execution_mode, model_id, effort_level, status)
             VALUES ('run-1', 'ws-1', 'task-1', 'build', 'build', 'glm', 'high', 'RUNNING')",
        )
        .execute(&pool)
        .await
        .expect("run");
        (temp, pool)
    }

    fn input(action: &str) -> CreateArtifactInput {
        CreateArtifactInput {
            workspace_id: "ws-1".into(),
            task_id: Some("task-1".into()),
            run_id: "run-1".into(),
            file_path: "output/report.md".into(),
            absolute_path: "/tmp/report.md".into(),
            file_type: "md".into(),
            file_size_bytes: 120,
            content_sha256: Some("abc".into()),
            previous_sha256: None,
            action_type: action.into(),
            created_by_agent: "build".into(),
            creation_reason: "fase build".into(),
            diff_patch: Some("+oi".into()),
        }
    }

    #[tokio::test]
    async fn artifact_lifecycle_and_filters() {
        let (_temp, pool) = setup().await;
        let artifact = create(&pool, input("CREATED")).await.expect("create");
        assert!(
            exists(&pool, "run-1", "output/report.md")
                .await
                .expect("exists")
        );
        assert!(
            !exists(&pool, "run-2", "output/report.md")
                .await
                .expect("exists")
        );

        set_destinations(
            &pool,
            &artifact.id,
            Some("https://docs.google.com/x"),
            Some("https://drive.google.com/y"),
            Some("a@b.c"),
        )
        .await
        .expect("destinations");
        let updated = get(&pool, &artifact.id).await.expect("get");
        assert_eq!(
            updated.google_doc_url.as_deref(),
            Some("https://docs.google.com/x")
        );
        assert_eq!(updated.emailed_to.as_deref(), Some("a@b.c"));

        assert_eq!(
            list(
                &pool,
                "ws-1",
                Some("build"),
                Some("task-1"),
                Some("CREATED"),
                10
            )
            .await
            .expect("list")
            .len(),
            1
        );
        assert!(
            list(&pool, "ws-1", Some("explore"), None, None, 10)
                .await
                .expect("list")
                .is_empty()
        );
        assert_eq!(count_for_task(&pool, "task-1").await.expect("count"), 1);
        assert!(get(&pool, "missing").await.is_err());
    }

    #[tokio::test]
    async fn pin_roundtrip_is_visible_on_get_and_list() {
        let (_temp, pool) = setup().await;
        let artifact = create(&pool, input("CREATED")).await.expect("create");
        assert!(!artifact.pinned);
        let pinned = set_pinned(&pool, &artifact.id, true).await.expect("pin");
        assert!(pinned.pinned);
        assert!(get(&pool, &artifact.id).await.expect("get").pinned);
        let listed = list(&pool, "ws-1", None, None, None, 10)
            .await
            .expect("list");
        assert!(
            listed
                .iter()
                .any(|item| item.id == artifact.id && item.pinned)
        );
        let pinned_list = list_pinned(&pool, "ws-1", 10).await.expect("pinned list");
        assert_eq!(pinned_list.len(), 1);
        let unpinned = set_pinned(&pool, &artifact.id, false).await.expect("unpin");
        assert!(!unpinned.pinned);
        assert!(
            list_pinned(&pool, "ws-1", 10)
                .await
                .expect("pinned list")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn rejects_invalid_actions() {
        let (_temp, pool) = setup().await;
        assert!(create(&pool, input("TOUCHED")).await.is_err());
    }
}
