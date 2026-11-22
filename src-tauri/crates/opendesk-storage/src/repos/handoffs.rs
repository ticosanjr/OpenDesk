//! Agent-to-agent handoffs.
//!
//! A handoff is a small, durable contract: objective, input artifacts, output
//! schema, acceptance criteria and an optional sub-budget. It starts PENDING and
//! needs an explicit accept/reject. Accepting creates a task for the target
//! agent; finishing that task completes the handoff.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

pub const HANDOFF_STATUSES: &[&str] = &[
    "PENDING",
    "ACCEPTED",
    "REJECTED",
    "RUNNING",
    "DONE",
    "FAILED",
    "CANCELLED",
];

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Handoff {
    pub id: String,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub parent_handoff_id: Option<String>,
    pub source_agent: String,
    pub target_agent: String,
    pub objective: String,
    pub input_artifacts_json: String,
    pub output_schema_json: String,
    pub acceptance_json: String,
    pub sub_budget_usd: Option<f64>,
    pub status: String,
    pub result_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Handoff {
    pub fn input_artifacts(&self) -> Vec<String> {
        serde_json::from_str(&self.input_artifacts_json).unwrap_or_default()
    }

    pub fn acceptance(&self) -> Vec<String> {
        serde_json::from_str(&self.acceptance_json).unwrap_or_default()
    }

    pub fn result(&self) -> Value {
        self.result_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or(Value::Null)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateHandoffInput {
    pub source_agent: String,
    pub target_agent: String,
    pub objective: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub parent_handoff_id: Option<String>,
    #[serde(default)]
    pub input_artifacts: Vec<String>,
    #[serde(default)]
    pub output_schema: Value,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub sub_budget_usd: Option<f64>,
}

const COLUMNS: &str = "id, workspace_id, task_id, parent_handoff_id, source_agent, target_agent, \
    objective, input_artifacts_json, output_schema_json, acceptance_json, sub_budget_usd, status, \
    result_json, created_at, updated_at";

pub async fn create(
    pool: &SqlitePool,
    workspace_id: &str,
    input: CreateHandoffInput,
) -> AppResult<Handoff> {
    let source = input.source_agent.trim();
    let target = input.target_agent.trim();
    if source.is_empty() || target.is_empty() {
        return Err(AppError::BadRequest(
            "handoff precisa de source_agent e target_agent".into(),
        ));
    }
    if source.eq_ignore_ascii_case(target) {
        return Err(AppError::BadRequest(
            "um agente não pode delegar para si mesmo".into(),
        ));
    }
    if input.objective.trim().is_empty() {
        return Err(AppError::BadRequest("handoff precisa de objetivo".into()));
    }
    if let Some(budget) = input.sub_budget_usd
        && budget < 0.0
    {
        return Err(AppError::BadRequest(
            "sub_budget_usd não pode ser negativo".into(),
        ));
    }
    if let Some(parent) = input.parent_handoff_id.as_deref() {
        get(pool, parent).await?;
    }

    let output_schema = if input.output_schema.is_null() {
        Value::Object(Default::default())
    } else {
        input.output_schema.clone()
    };
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO handoffs (
            id, workspace_id, task_id, parent_handoff_id, source_agent, target_agent,
            objective, input_artifacts_json, output_schema_json, acceptance_json,
            sub_budget_usd, status
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'PENDING')",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(input.task_id.as_deref())
    .bind(input.parent_handoff_id.as_deref())
    .bind(source)
    .bind(target)
    .bind(input.objective.trim())
    .bind(serde_json::to_string(&input.input_artifacts).unwrap_or_else(|_| "[]".into()))
    .bind(serde_json::to_string(&output_schema).unwrap_or_else(|_| "{}".into()))
    .bind(serde_json::to_string(&input.acceptance).unwrap_or_else(|_| "[]".into()))
    .bind(input.sub_budget_usd)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn list(
    pool: &SqlitePool,
    workspace_id: &str,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<Handoff>> {
    let query = format!(
        "SELECT {COLUMNS} FROM handoffs
         WHERE workspace_id = ? AND (? IS NULL OR status = ?)
         ORDER BY created_at DESC LIMIT ?"
    );
    Ok(sqlx::query_as::<_, Handoff>(&query)
        .bind(workspace_id)
        .bind(status)
        .bind(status)
        .bind(limit.clamp(1, 500))
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Handoff> {
    let query = format!("SELECT {COLUMNS} FROM handoffs WHERE id = ?");
    sqlx::query_as::<_, Handoff>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("handoff {id}")))
}

/// Applies an explicit transition and rejects invalid ones.
pub async fn transition(
    pool: &SqlitePool,
    id: &str,
    to: &str,
    result: Option<Value>,
) -> AppResult<Handoff> {
    let current = get(pool, id).await?;
    let to = to.trim().to_uppercase();
    if !HANDOFF_STATUSES.contains(&to.as_str()) {
        return Err(AppError::BadRequest(format!(
            "status de handoff inválido '{to}'"
        )));
    }
    let allowed = match current.status.as_str() {
        "PENDING" => matches!(to.as_str(), "ACCEPTED" | "REJECTED" | "CANCELLED"),
        "ACCEPTED" => matches!(to.as_str(), "RUNNING" | "DONE" | "FAILED" | "CANCELLED"),
        "RUNNING" => matches!(to.as_str(), "DONE" | "FAILED" | "CANCELLED"),
        _ => false,
    };
    if !allowed {
        return Err(AppError::Conflict(format!(
            "transição inválida {} → {to}",
            current.status
        )));
    }
    let result_json = result
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    sqlx::query(
        "UPDATE handoffs SET status = ?,
                result_json = COALESCE(?, result_json),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&to)
    .bind(result_json.as_deref())
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

/// Merges metadata into `result_json` (used to remember the created task).
pub async fn merge_result(pool: &SqlitePool, id: &str, patch: Value) -> AppResult<Handoff> {
    let current = get(pool, id).await?;
    let mut result = current.result();
    if !result.is_object() {
        result = Value::Object(Default::default());
    }
    if let (Some(target), Some(source)) = (result.as_object_mut(), patch.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    let encoded =
        serde_json::to_string(&result).map_err(|error| AppError::Internal(error.to_string()))?;
    sqlx::query(
        "UPDATE handoffs SET result_json = ?,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&encoded)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

/// Called after a run settles: the delegated task closes its handoff.
pub async fn complete_for_task(
    pool: &SqlitePool,
    task_id: &str,
    success: bool,
) -> AppResult<Option<Handoff>> {
    let id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM handoffs
         WHERE status IN ('ACCEPTED', 'RUNNING')
           AND json_extract(result_json, '$.task_id') = ?
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;
    let Some(id) = id else {
        return Ok(None);
    };
    let status = if success { "DONE" } else { "FAILED" };
    Ok(Some(transition(pool, &id, status, None).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(target: &str) -> CreateHandoffInput {
        CreateHandoffInput {
            source_agent: "build".into(),
            target_agent: target.into(),
            objective: "Revisar o relatório".into(),
            task_id: None,
            parent_handoff_id: None,
            input_artifacts: vec!["output/report.md".into()],
            output_schema: serde_json::json!({ "type": "object" }),
            acceptance: vec!["sem erros".into()],
            sub_budget_usd: Some(1.5),
        }
    }

    #[tokio::test]
    async fn enforces_status_transitions() {
        let ctx = crate::test_support::context().await;
        let handoff = create(&ctx.db, &ctx.workspace_id, input("review"))
            .await
            .expect("create");
        assert_eq!(handoff.status, "PENDING");

        let error = transition(&ctx.db, &handoff.id, "DONE", None)
            .await
            .expect_err("cannot skip");
        assert!(matches!(error, AppError::Conflict(_)));

        let accepted = transition(&ctx.db, &handoff.id, "accepted", None)
            .await
            .expect("accept");
        assert_eq!(accepted.status, "ACCEPTED");
        let merged = merge_result(
            &ctx.db,
            &handoff.id,
            serde_json::json!({ "task_id": "task-1" }),
        )
        .await
        .expect("merge");
        assert_eq!(merged.result()["task_id"], "task-1");
        let done = complete_for_task(&ctx.db, "task-1", true)
            .await
            .expect("complete")
            .expect("handoff");
        assert_eq!(done.status, "DONE");
    }

    #[tokio::test]
    async fn rejects_self_delegation_and_unknown_parent() {
        let ctx = crate::test_support::context().await;
        assert!(
            create(&ctx.db, &ctx.workspace_id, input("build"))
                .await
                .is_err()
        );
        let mut orphan = input("review");
        orphan.parent_handoff_id = Some("missing".into());
        assert!(matches!(
            create(&ctx.db, &ctx.workspace_id, orphan)
                .await
                .expect_err("orphan"),
            AppError::NotFound(_)
        ));
    }
}
