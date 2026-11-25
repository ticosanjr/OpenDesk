//! MCP stdio server that exposes enabled `script_tools` as `script_<slug>`.
//!
//! Unlike `opendesk-control`, this server executes locally: it opens the same
//! SQLite database and runs the validated script with the workspace root as
//! `cwd`. Stdout stays reserved for JSON-RPC frames.

use crate::{
    db::script_tools::{self, ScriptTool},
    error::{AppError, AppResult},
    paths::AppPaths,
    script_tools as executor,
};
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

pub async fn run_stdio() -> Result<()> {
    let workspace_id = std::env::var("OPENDESK_WORKSPACE_ID")
        .context("OPENDESK_WORKSPACE_ID é obrigatório para o MCP de scripts")?;
    let paths = AppPaths::discover().map_err(|error| anyhow!(error.to_string()))?;
    let pool = crate::db::init(&paths.db_path)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let tools = script_tools::list_enabled(&pool, Some(&workspace_id))
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let (definitions, by_name) = definitions(tools)?;
    crate::mcp_server::serve_stdio("opendesk-scripts", definitions, |name, arguments| {
        let pool = pool.clone();
        let paths = paths.clone();
        let workspace_id = workspace_id.clone();
        let tool = by_name.get(&name).cloned();
        async move {
            let tool = tool.ok_or_else(|| anyhow!("ferramenta desconhecida: {name}"))?;
            call_tool(&pool, &paths, &workspace_id, &tool, &arguments).await
        }
    })
    .await
}

pub fn definitions(
    tools: Vec<ScriptTool>,
) -> AppResult<(Vec<Value>, BTreeMap<String, ScriptTool>)> {
    let mut by_name = BTreeMap::new();
    for tool in tools {
        // Workspace tools override globals with the same slug.
        by_name.insert(format!("script_{}", tool.slug), tool);
    }
    let mut definitions = Vec::new();
    for (name, tool) in &by_name {
        let schema: Value = serde_json::from_str(&tool.input_schema_json).map_err(|error| {
            AppError::BadRequest(format!("input_schema inválido em {}: {error}", tool.slug))
        })?;
        let description = [tool.description.trim(), tool.agent_prompt.trim()]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        definitions.push(json!({
            "name": name,
            "description": description,
            "inputSchema": schema,
        }));
    }
    Ok((definitions, by_name))
}

pub async fn call_tool(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    tool: &ScriptTool,
    arguments: &Value,
) -> Result<String> {
    if tool.require_approval {
        return Err(anyhow!(
            "a ferramenta {} exige aprovação HITL; aprove-a em Integrações antes de executar",
            tool.slug
        ));
    }
    let result = executor::execute(pool, paths, workspace_id, tool, arguments)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    serde_json::to_string(&result).map_err(|error| anyhow!(error.to_string()))
}
