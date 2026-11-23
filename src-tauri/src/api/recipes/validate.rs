//! Dry validation for recipe YAML, including Vault references.

use super::RecipeQuery;
use crate::{app_state::AppState, db::vault, error::AppResult, recipes::parser};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ValidateBody {
    pub yaml: String,
}

pub async fn validate(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RecipeQuery>,
    Json(body): Json<ValidateBody>,
) -> AppResult<Json<Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let recipe = match parser::parse(&body.yaml) {
        Ok(recipe) => recipe,
        Err(error) => {
            return Ok(Json(json!({
                "valid": false,
                "issues": [{ "path": "yaml", "message": error.to_string() }],
            })));
        }
    };
    let issues = vault_issues(&state.db, &workspace_id, &recipe).await;
    Ok(Json(json!({
        "valid": issues.is_empty(),
        "issues": issues,
        "recipe": recipe,
    })))
}

pub(crate) async fn vault_issues(
    pool: &SqlitePool,
    workspace_id: &str,
    recipe: &parser::Recipe,
) -> Vec<Value> {
    let mut issues = Vec::new();
    for step in recipe.all_steps() {
        for reference in &step.vault_refs {
            match vault::get_metadata(pool, reference).await {
                Ok(entry) => {
                    if let Some(entry_workspace) = entry.workspace_id.as_deref()
                        && entry_workspace != workspace_id
                    {
                        issues.push(json!({
                            "path": step.id,
                            "message": format!("vault_ref {reference} pertence a outro workspace"),
                        }));
                        continue;
                    }
                    match vault::list_acl(pool, reference).await {
                        Ok(acls) if acls.iter().any(|acl| acl.can_inject) => {}
                        Ok(_) => issues.push(json!({
                            "path": step.id,
                            "message": format!("vault_ref {reference} não tem ACL de injeção"),
                        })),
                        Err(error) => issues.push(json!({
                            "path": step.id,
                            "message": format!("vault_ref {reference}: {error}"),
                        })),
                    }
                }
                Err(_) => issues.push(json!({
                    "path": step.id,
                    "message": format!("vault_ref {reference} não existe"),
                })),
            }
        }
    }
    issues
}
