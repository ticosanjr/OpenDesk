//! Curated evaluation cases for skills (G1).

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SkillEvalCase {
    pub id: String,
    pub workspace_id: String,
    pub skill_id: String,
    pub name: String,
    pub prompt: String,
    pub expected_contains: Option<String>,
    pub expected_schema_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSkillCase {
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub expected_contains: Option<String>,
    #[serde(default)]
    pub expected_schema: Option<serde_json::Value>,
}

const COLUMNS: &str = "id, workspace_id, skill_id, name, prompt, expected_contains, \
    expected_schema_json, created_at";

pub async fn list(
    pool: &SqlitePool,
    workspace_id: &str,
    skill_id: &str,
) -> AppResult<Vec<SkillEvalCase>> {
    Ok(sqlx::query_as::<_, SkillEvalCase>(&format!(
        "SELECT {COLUMNS} FROM skill_eval_cases
         WHERE workspace_id = ? AND skill_id = ? ORDER BY created_at"
    ))
    .bind(workspace_id)
    .bind(skill_id)
    .fetch_all(pool)
    .await?)
}

pub async fn create(
    pool: &SqlitePool,
    workspace_id: &str,
    skill_id: &str,
    input: CreateSkillCase,
) -> AppResult<SkillEvalCase> {
    let name = input.name.trim();
    let prompt = input.prompt.trim();
    if name.is_empty() || prompt.is_empty() {
        return Err(AppError::BadRequest(
            "nome e prompt do caso são obrigatórios".into(),
        ));
    }
    if let Some(schema) = &input.expected_schema
        && jsonschema::validator_for(schema).is_err()
    {
        return Err(AppError::BadRequest("expected_schema inválido".into()));
    }
    let id = Uuid::now_v7().to_string();
    let schema = input
        .expected_schema
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    sqlx::query(
        "INSERT INTO skill_eval_cases
            (id, workspace_id, skill_id, name, prompt, expected_contains, expected_schema_json)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(skill_id)
    .bind(name)
    .bind(prompt)
    .bind(
        input
            .expected_contains
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    )
    .bind(schema)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<SkillEvalCase> {
    sqlx::query_as::<_, SkillEvalCase>(&format!(
        "SELECT {COLUMNS} FROM skill_eval_cases WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("caso de eval {id}")))
}

pub async fn delete(
    pool: &SqlitePool,
    workspace_id: &str,
    skill_id: &str,
    id: &str,
) -> AppResult<bool> {
    let result = sqlx::query(
        "DELETE FROM skill_eval_cases
         WHERE id = ? AND workspace_id = ? AND skill_id = ?",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(skill_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn creates_lists_and_deletes_cases() {
        let ctx = crate::test_support::context().await;
        let case = create(
            &ctx.db,
            &ctx.workspace_id,
            "demo",
            CreateSkillCase {
                name: "contrato".into(),
                prompt: "faça x".into(),
                expected_contains: Some("ok".into()),
                expected_schema: Some(serde_json::json!({
                    "type": "object",
                    "required": ["status"]
                })),
            },
        )
        .await
        .expect("case");
        assert_eq!(
            list(&ctx.db, &ctx.workspace_id, "demo")
                .await
                .expect("list")
                .len(),
            1
        );
        assert!(
            delete(&ctx.db, &ctx.workspace_id, "demo", &case.id)
                .await
                .expect("delete")
        );
        assert!(
            list(&ctx.db, &ctx.workspace_id, "demo")
                .await
                .expect("list")
                .is_empty()
        );
        assert!(
            create(
                &ctx.db,
                &ctx.workspace_id,
                "demo",
                CreateSkillCase {
                    name: "bad".into(),
                    prompt: "x".into(),
                    expected_contains: None,
                    expected_schema: Some(serde_json::json!({"type": "nope"})),
                },
            )
            .await
            .is_err()
        );
    }
}
