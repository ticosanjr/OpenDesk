use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub description: String,
    pub column_stage: String,
    pub phase_status: Option<String>,
    pub priority: i64,
    pub execution_mode: String,
    pub assigned_agent: Option<String>,
    pub preferred_model: Option<String>,
    pub effort_level: String,
    pub plan_markdown: Option<String>,
    pub output_dir_path: Option<String>,
    pub notify_email: Option<String>,
    pub export_to_gdocs: bool,
    pub tags_json: String,
    pub position_index: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl Task {
    pub fn tags(&self) -> Vec<String> {
        serde_json::from_str(&self.tags_json).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskInput {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub execution_mode: Option<String>,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub assigned_agent: Option<String>,
    #[serde(default)]
    pub preferred_model: Option<String>,
    #[serde(default)]
    pub effort_level: Option<String>,
    #[serde(default)]
    pub notify_email: Option<String>,
    #[serde(default)]
    pub export_to_gdocs: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateTaskInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub column_stage: Option<String>,
    pub phase_status: Option<String>,
    pub priority: Option<i64>,
    pub execution_mode: Option<String>,
    pub assigned_agent: Option<String>,
    pub preferred_model: Option<String>,
    pub effort_level: Option<String>,
    pub plan_markdown: Option<String>,
    pub output_dir_path: Option<String>,
    pub notify_email: Option<String>,
    pub export_to_gdocs: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub position_index: Option<i64>,
    #[serde(default)]
    pub clear_agent: bool,
    #[serde(default)]
    pub clear_model: bool,
    #[serde(default)]
    pub clear_plan: bool,
}

const COLUMNS: &str = "id, workspace_id, title, description, column_stage, phase_status, \
    priority, execution_mode, assigned_agent, preferred_model, effort_level, plan_markdown, \
    output_dir_path, notify_email, export_to_gdocs, tags_json, position_index, created_at, updated_at";

const STAGES: [&str; 5] = ["BACKLOG", "PLAN", "BUILD", "REVIEW", "DONE"];
const MODES: [&str; 4] = ["AUTO_DSH", "PLAN_ONLY", "BUILD_DIRECT", "PLAN_THEN_BUILD"];
const EFFORTS: [&str; 3] = ["low", "medium", "high"];

fn validate_stage(stage: &str) -> AppResult<String> {
    let stage = stage.trim().to_uppercase();
    if STAGES.contains(&stage.as_str()) {
        Ok(stage)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid column_stage '{stage}'"
        )))
    }
}

fn validate_mode(mode: &str) -> AppResult<String> {
    let mode = mode.trim().to_uppercase();
    if MODES.contains(&mode.as_str()) {
        Ok(mode)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid execution_mode '{mode}'"
        )))
    }
}

fn validate_effort(effort: &str) -> AppResult<String> {
    let effort = effort.trim().to_lowercase();
    if EFFORTS.contains(&effort.as_str()) {
        Ok(effort)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid effort_level '{effort}'"
        )))
    }
}

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<Task>> {
    let query = format!(
        "SELECT {COLUMNS} FROM tasks WHERE workspace_id = ?
         ORDER BY CASE column_stage
            WHEN 'BACKLOG' THEN 0 WHEN 'PLAN' THEN 1 WHEN 'BUILD' THEN 2
            WHEN 'REVIEW' THEN 3 WHEN 'DONE' THEN 4 ELSE 5 END,
            position_index ASC, created_at ASC"
    );
    Ok(sqlx::query_as::<_, Task>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    let query = format!("SELECT {COLUMNS} FROM tasks WHERE id = ?");
    sqlx::query_as::<_, Task>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {id}")))
}

pub async fn create(
    pool: &SqlitePool,
    workspace_id: &str,
    input: CreateTaskInput,
) -> AppResult<Task> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest("task title is required".into()));
    }
    if input.description.trim().is_empty() {
        return Err(AppError::BadRequest("task description is required".into()));
    }

    let mode = validate_mode(input.execution_mode.as_deref().unwrap_or("AUTO_DSH"))?;
    let effort = validate_effort(input.effort_level.as_deref().unwrap_or("medium"))?;
    let priority = input.priority.unwrap_or(2).clamp(1, 4);
    let tags_json = serde_json::to_string(&input.tags).unwrap_or_else(|_| "[]".into());

    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position_index), -1) + 1 FROM tasks
         WHERE workspace_id = ? AND column_stage = 'BACKLOG'",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?;

    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO tasks (
            id, workspace_id, title, description, column_stage, priority,
            execution_mode, assigned_agent, preferred_model, effort_level,
            notify_email, export_to_gdocs, tags_json, position_index
         ) VALUES (?, ?, ?, ?, 'BACKLOG', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(title)
    .bind(input.description.trim())
    .bind(priority)
    .bind(&mode)
    .bind(input.assigned_agent.as_deref())
    .bind(input.preferred_model.as_deref())
    .bind(&effort)
    .bind(input.notify_email.as_deref())
    .bind(input.export_to_gdocs)
    .bind(&tags_json)
    .bind(position)
    .execute(pool)
    .await?;

    get(pool, &id).await
}

pub async fn update(pool: &SqlitePool, id: &str, input: UpdateTaskInput) -> AppResult<Task> {
    let current = get(pool, id).await?;

    let title = match input.title {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        Some(_) => return Err(AppError::BadRequest("task title cannot be empty".into())),
        None => current.title,
    };
    let stage = match input.column_stage {
        Some(value) => validate_stage(&value)?,
        None => current.column_stage,
    };
    let mode = match input.execution_mode {
        Some(value) => validate_mode(&value)?,
        None => current.execution_mode,
    };
    let effort = match input.effort_level {
        Some(value) => validate_effort(&value)?,
        None => current.effort_level,
    };
    let tags_json = match input.tags {
        Some(tags) => serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into()),
        None => current.tags_json,
    };

    sqlx::query(
        "UPDATE tasks SET
            title = ?, description = ?, column_stage = ?, phase_status = ?,
            priority = ?, execution_mode = ?, assigned_agent = ?, preferred_model = ?,
            effort_level = ?, plan_markdown = ?, output_dir_path = ?, notify_email = ?,
            export_to_gdocs = ?, tags_json = ?, position_index = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(&title)
    .bind(input.description.unwrap_or(current.description))
    .bind(&stage)
    .bind(input.phase_status.or(current.phase_status))
    .bind(input.priority.unwrap_or(current.priority).clamp(1, 4))
    .bind(&mode)
    .bind(if input.clear_agent {
        None
    } else {
        input.assigned_agent.or(current.assigned_agent)
    })
    .bind(if input.clear_model {
        None
    } else {
        input.preferred_model.or(current.preferred_model)
    })
    .bind(&effort)
    .bind(if input.clear_plan {
        None
    } else {
        input.plan_markdown.or(current.plan_markdown)
    })
    .bind(input.output_dir_path.or(current.output_dir_path))
    .bind(input.notify_email.or(current.notify_email))
    .bind(input.export_to_gdocs.unwrap_or(current.export_to_gdocs))
    .bind(&tags_json)
    .bind(input.position_index.unwrap_or(current.position_index))
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id).await
}

pub async fn move_to(
    pool: &SqlitePool,
    id: &str,
    stage: &str,
    position_index: Option<i64>,
) -> AppResult<Task> {
    let stage = validate_stage(stage)?;
    let position = match position_index {
        Some(position) => position,
        None => {
            sqlx::query_scalar(
                "SELECT COALESCE(MAX(position_index), -1) + 1 FROM tasks
                 WHERE workspace_id = (SELECT workspace_id FROM tasks WHERE id = ?)
                   AND column_stage = ?",
            )
            .bind(id)
            .bind(&stage)
            .fetch_one(pool)
            .await?
        }
    };

    let result = sqlx::query(
        "UPDATE tasks SET column_stage = ?, position_index = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(&stage)
    .bind(position)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("task {id}")));
    }

    get(pool, id).await
}

pub async fn set_plan(pool: &SqlitePool, id: &str, plan_markdown: &str) -> AppResult<Task> {
    sqlx::query(
        "UPDATE tasks SET plan_markdown = ?, column_stage = 'PLAN',
            phase_status = 'awaiting_approval',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(plan_markdown)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_phase_status(
    pool: &SqlitePool,
    id: &str,
    phase_status: Option<&str>,
    stage: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET
            phase_status = COALESCE(?, phase_status),
            column_stage = COALESCE(?, column_stage),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(phase_status)
    .bind(stage)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Replace (not merge) the phase status. Used by explicit workflow
/// transitions such as moving a card back to Backlog or to Done.
pub async fn replace_phase_status(
    pool: &SqlitePool,
    id: &str,
    phase_status: Option<&str>,
    stage: Option<&str>,
) -> AppResult<Task> {
    sqlx::query(
        "UPDATE tasks SET
            phase_status = ?,
            column_stage = COALESCE(?, column_stage),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(phase_status)
    .bind(stage)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_output_dir(pool: &SqlitePool, id: &str, output_dir: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET output_dir_path = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(output_dir)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("task {id}")));
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn count_artifacts(pool: &SqlitePool, task_id: &str) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM artifacts_provenance WHERE task_id = ?")
            .bind(task_id)
            .fetch_one(pool)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("tasks.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    fn input() -> CreateTaskInput {
        CreateTaskInput {
            title: "Relatório".into(),
            description: "Produza o relatório".into(),
            execution_mode: Some("PLAN_THEN_BUILD".into()),
            priority: Some(9),
            assigned_agent: Some("build".into()),
            preferred_model: None,
            effort_level: Some("high".into()),
            notify_email: Some("ops@example.com".into()),
            export_to_gdocs: true,
            tags: vec!["financeiro".into(), "mensal".into()],
        }
    }

    #[tokio::test]
    async fn task_lifecycle() {
        let (_temp, pool) = setup().await;
        let task = create(&pool, "ws-1", input()).await.expect("create");
        assert_eq!(task.column_stage, "BACKLOG");
        assert_eq!(task.priority, 4);
        assert_eq!(task.execution_mode, "PLAN_THEN_BUILD");
        assert_eq!(task.tags(), vec!["financeiro", "mensal"]);
        assert!(task.export_to_gdocs);

        let updated = update(
            &pool,
            &task.id,
            UpdateTaskInput {
                title: Some("Relatório v2".into()),
                priority: Some(1),
                assigned_agent: Some("explore".into()),
                preferred_model: Some("opencode-go/glm-5.3#high".into()),
                ..Default::default()
            },
        )
        .await
        .expect("update");
        assert_eq!(updated.title, "Relatório v2");
        assert_eq!(updated.priority, 1);
        assert_eq!(updated.assigned_agent.as_deref(), Some("explore"));
        assert_eq!(
            updated.preferred_model.as_deref(),
            Some("opencode-go/glm-5.3#high")
        );

        let moved = move_to(&pool, &task.id, "BUILD", Some(2))
            .await
            .expect("move");
        assert_eq!(moved.column_stage, "BUILD");
        assert_eq!(moved.position_index, 2);

        assert!(
            set_plan(&pool, &task.id, "# Plano")
                .await
                .expect("plan")
                .plan_markdown
                .is_some()
        );
        set_phase_status(&pool, &task.id, Some("running"), Some("BUILD"))
            .await
            .expect("phase");
        assert_eq!(
            get(&pool, &task.id)
                .await
                .expect("get")
                .phase_status
                .as_deref(),
            Some("running")
        );

        set_output_dir(&pool, &task.id, "/tmp/output")
            .await
            .expect("output");
        assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 1);
        delete(&pool, &task.id).await.expect("delete");
        assert!(get(&pool, &task.id).await.is_err());
    }

    #[tokio::test]
    async fn rejects_invalid_task_input() {
        let (_temp, pool) = setup().await;
        let mut value = input();
        value.title = "  ".into();
        assert!(create(&pool, "ws-1", value).await.is_err());

        let mut value = input();
        value.execution_mode = Some("nonsense".into());
        assert!(create(&pool, "ws-1", value).await.is_err());

        let mut value = input();
        value.effort_level = Some("extreme".into());
        assert!(create(&pool, "ws-1", value).await.is_err());
    }
}
