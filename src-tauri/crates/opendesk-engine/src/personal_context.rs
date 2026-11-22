use crate::error::{AppError, AppResult};
use opendesk_storage::repos::workspaces;

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

const MAX_NAME: usize = 80;
const MAX_INSTRUCTIONS: usize = 4_000;
const MAX_PROMPT_CHARS: usize = 8_000;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PersonalContextGroup {
    pub id: String,
    pub workspace_id: Option<String>,
    pub scope: String,
    pub name: String,
    pub instructions: String,
    pub is_enabled: bool,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
    pub source: String,
    pub source_quote: Option<String>,
}

const COLUMNS: &str = "id, workspace_id, scope, name, instructions, is_enabled, position, \
    created_at, updated_at, source, source_quote";

#[derive(Debug, Clone, Deserialize)]
pub struct PersonalContextInput {
    #[serde(default = "default_scope")]
    pub scope: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    /// Optional: the UI treats instructions as the rule and derives a short
    /// internal label when no name is supplied.
    #[serde(default)]
    pub name: Option<String>,
    pub instructions: String,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
    #[serde(default)]
    pub position: Option<i64>,
    /// Present only when the agent saves on an explicit user request.
    #[serde(default)]
    pub source_quote: Option<String>,
}

fn default_scope() -> String {
    "workspace".into()
}

fn default_true() -> bool {
    true
}

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<PersonalContextGroup>> {
    let query = format!(
        "SELECT {COLUMNS} FROM personal_context_groups
         WHERE scope = 'global' OR workspace_id = ?
         ORDER BY CASE scope WHEN 'workspace' THEN 0 ELSE 1 END, position, created_at"
    );
    Ok(sqlx::query_as::<_, PersonalContextGroup>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

/// Enabled groups only, in prompt order. Workspace-specific instructions come
/// before global defaults so the user's current project can override them.
pub async fn enabled(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<PersonalContextGroup>> {
    let groups = list(pool, workspace_id).await?;
    Ok(groups
        .into_iter()
        .filter(|group| group.is_enabled)
        .collect())
}

pub async fn render_body(pool: &SqlitePool, workspace_id: &str) -> AppResult<String> {
    let groups = enabled(pool, workspace_id).await?;
    if groups.is_empty() {
        return Ok(String::new());
    }
    let mut text = String::new();
    for group in groups {
        if text.chars().count() >= MAX_PROMPT_CHARS {
            break;
        }
        let remaining = MAX_PROMPT_CHARS.saturating_sub(text.chars().count());
        let instructions = group.instructions.trim();
        let block = format!(
            "### {} ({})\n{}\n\n",
            group.name.trim(),
            if group.scope == "global" {
                "global"
            } else {
                "workspace"
            },
            instructions
        );
        text.push_str(&block.chars().take(remaining).collect::<String>());
    }
    Ok(text.trim_end().to_string())
}

#[allow(dead_code)]
pub async fn render(pool: &SqlitePool, workspace_id: &str) -> AppResult<String> {
    let body = render_body(pool, workspace_id).await?;
    if body.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("## Contexto pessoal do usuário\n\n{body}"))
}

pub async fn create(
    pool: &SqlitePool,
    input: PersonalContextInput,
) -> AppResult<PersonalContextGroup> {
    let (scope, workspace_id, name, instructions) = validate(&input, pool, None).await?;
    let id = Uuid::now_v7().to_string();
    let position = match input.position {
        Some(position) => position,
        None => {
            let max: Option<i64> = sqlx::query_scalar(
                "SELECT MAX(position) FROM personal_context_groups
                 WHERE scope = ? AND (? IS NULL OR workspace_id = ?)",
            )
            .bind(&scope)
            .bind(&workspace_id)
            .bind(&workspace_id)
            .fetch_one(pool)
            .await?;
            max.unwrap_or(-1) + 1
        }
    };
    let source_quote = input
        .source_quote
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let source = if source_quote.is_some() {
        "agent"
    } else {
        "manual"
    };
    sqlx::query(
        "INSERT INTO personal_context_groups
            (id, workspace_id, scope, name, instructions, is_enabled, position, source, source_quote)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&workspace_id)
    .bind(&scope)
    .bind(&name)
    .bind(&instructions)
    .bind(input.is_enabled)
    .bind(position)
    .bind(source)
    .bind(source_quote.as_deref())
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    input: PersonalContextInput,
) -> AppResult<PersonalContextGroup> {
    let current = get(pool, id).await?;
    let (scope, workspace_id, name, instructions) = validate(&input, pool, None).await?;
    let source_quote = input
        .source_quote
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| current.source_quote.clone());
    let source = if source_quote.is_some() {
        "agent"
    } else {
        current.source.as_str()
    };
    sqlx::query(
        "UPDATE personal_context_groups SET
            workspace_id = ?, scope = ?, name = ?, instructions = ?,
            is_enabled = ?, position = ?, source = ?, source_quote = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&workspace_id)
    .bind(&scope)
    .bind(&name)
    .bind(&instructions)
    .bind(input.is_enabled)
    .bind(input.position.unwrap_or(current.position))
    .bind(source)
    .bind(source_quote.as_deref())
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM personal_context_groups WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("personal context {id}")));
    }
    Ok(())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<PersonalContextGroup> {
    let query = format!("SELECT {COLUMNS} FROM personal_context_groups WHERE id = ?");
    sqlx::query_as::<_, PersonalContextGroup>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("personal context {id}")))
}

async fn validate(
    input: &PersonalContextInput,
    pool: &SqlitePool,
    fallback_name: Option<&str>,
) -> AppResult<(String, Option<String>, String, String)> {
    let scope = input.scope.trim().to_lowercase();
    if !matches!(scope.as_str(), "workspace" | "global") {
        return Err(AppError::BadRequest(
            "scope deve ser 'workspace' ou 'global'".into(),
        ));
    }
    let instructions = input.instructions.trim();
    if instructions.is_empty() {
        return Err(AppError::BadRequest(
            "instruções do contexto são obrigatórias".into(),
        ));
    }
    if instructions.chars().count() > MAX_INSTRUCTIONS {
        return Err(AppError::BadRequest(format!(
            "instruções do contexto excedem {MAX_INSTRUCTIONS} caracteres"
        )));
    }
    let name = match input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(name) => name.to_string(),
        None => fallback_name
            .map(str::to_string)
            .unwrap_or_else(|| derive_name(instructions)),
    };
    if name.chars().count() > MAX_NAME {
        return Err(AppError::BadRequest(format!(
            "nome do contexto excede {MAX_NAME} caracteres"
        )));
    }
    let workspace_id = if scope == "global" {
        None
    } else {
        let id = input
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::BadRequest("workspace_id é obrigatório no escopo workspace".into())
            })?;
        workspaces::get(pool, id).await?;
        Some(id.to_string())
    };
    Ok((scope, workspace_id, name, instructions.to_string()))
}

fn derive_name(instructions: &str) -> String {
    let line = instructions
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Regra");
    let name = line.trim_start_matches(['#', '-', '*', ' ']).trim();
    let name = if name.is_empty() { "Regra" } else { name };
    name.chars().take(MAX_NAME).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn renders_enabled_groups_in_order() {
        let ctx = crate::test_support::context().await;
        create(
            &ctx.db,
            PersonalContextInput {
                scope: "workspace".into(),
                workspace_id: Some(ctx.workspace_id.clone()),
                name: Some("Tom".into()),
                instructions: "Responda de forma direta.".into(),
                is_enabled: true,
                position: None,
                source_quote: None,
            },
        )
        .await
        .expect("workspace group");
        let global = create(
            &ctx.db,
            PersonalContextInput {
                scope: "global".into(),
                workspace_id: None,
                name: Some("Global".into()),
                instructions: "Prefira português.".into(),
                is_enabled: true,
                position: Some(10),
                source_quote: None,
            },
        )
        .await
        .expect("global group");
        let disabled = create(
            &ctx.db,
            PersonalContextInput {
                scope: "global".into(),
                workspace_id: None,
                name: Some("Desligado".into()),
                instructions: "Não deve aparecer.".into(),
                is_enabled: false,
                position: Some(20),
                source_quote: None,
            },
        )
        .await
        .expect("disabled group");

        let rendered = render(&ctx.db, &ctx.workspace_id).await.expect("render");
        assert!(rendered.contains("Responda de forma direta."));
        assert!(rendered.contains("Prefira português."));
        assert!(!rendered.contains("Não deve aparecer."));
        assert!(rendered.find("Tom").unwrap() < rendered.find("Global").unwrap());

        delete(&ctx.db, &global.id).await.expect("delete");
        delete(&ctx.db, &disabled.id)
            .await
            .expect("delete disabled");
    }
}
