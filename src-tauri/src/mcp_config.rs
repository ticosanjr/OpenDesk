use crate::{
    db::{self, proposals::ProposalInput},
    mcp_server,
    paths::AppPaths,
};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn run_stdio() -> Result<()> {
    let workspace_id = std::env::var("OPENDESK_WORKSPACE_ID")
        .context("OPENDESK_WORKSPACE_ID é obrigatório para o MCP de configuração")?;
    let paths = AppPaths::discover()?;
    let pool = Arc::new(db::init(&paths.db_path).await?);

    mcp_server::serve_stdio("opendesk-config", tools(), |name, arguments| {
        let paths = &paths;
        let pool = &pool;
        let workspace_id = &workspace_id;
        async move {
            propose(paths, pool, workspace_id, &name, arguments)
                .await
                .map(|value| value.to_string())
        }
    })
    .await?;

    let _ = pool.close().await;
    Ok(())
}

async fn propose(
    paths: &AppPaths,
    pool: &sqlx::SqlitePool,
    workspace_id: &str,
    tool: &str,
    args: Value,
) -> Result<Value> {
    let (kind, action, title, payload) = match tool {
        "propose_agent" => {
            let id = required(&args, "id")?;
            let title = args
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("Criar agente @{id}"));
            ("AGENT", "CREATE", title, args.clone())
        }
        "propose_skill" => {
            let id = required(&args, "id")?;
            let title = args
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("Criar skill {id}"));
            ("SKILL", "CREATE", title, args.clone())
        }
        "propose_command" => {
            let id = required(&args, "id")?;
            let title = args
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("Criar comando /{id}"));
            ("COMMAND", "CREATE", title, args.clone())
        }
        "propose_recipe" => {
            let id = required(&args, "id")?;
            (
                "RECIPE",
                "CREATE",
                format!("Criar receita {id}"),
                args.clone(),
            )
        }
        "propose_routine" => {
            let name = required(&args, "name")?;
            (
                "ROUTINE",
                "CREATE",
                format!("Criar rotina {name}"),
                args.clone(),
            )
        }
        "propose_mcp" => {
            let name = required(&args, "name")?;
            (
                "MCP",
                "CREATE",
                format!("Adicionar MCP {name}"),
                args.clone(),
            )
        }
        "propose_config" => {
            let path = required(&args, "path")?;
            (
                "CONFIG",
                "CREATE",
                format!("Alterar configuração {path}"),
                args.clone(),
            )
        }
        "propose_change" => {
            let kind = required(&args, "kind")?.to_uppercase();
            let action = args
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("CREATE")
                .to_uppercase();
            let title = required(&args, "title")?;
            let payload = args.get("payload").cloned().unwrap_or_else(|| json!({}));
            let input = ProposalInput {
                kind,
                action,
                title: title.to_string(),
                description: args
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                payload,
                requested_by: args
                    .get("requested_by")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                source_run_id: None,
                source_task_id: None,
            };
            return submit(paths, pool, workspace_id, input).await;
        }
        other => anyhow::bail!("ferramenta desconhecida: {other}"),
    };
    let input = ProposalInput {
        kind: kind.into(),
        action: action.into(),
        title,
        description: args
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
        payload,
        requested_by: args
            .get("requested_by")
            .and_then(Value::as_str)
            .map(str::to_string),
        source_run_id: args
            .get("source_run_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        source_task_id: args
            .get("source_task_id")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    submit(paths, pool, workspace_id, input).await
}

async fn submit(
    paths: &AppPaths,
    pool: &sqlx::SqlitePool,
    workspace_id: &str,
    input: ProposalInput,
) -> Result<Value> {
    if let Ok(endpoint) = std::fs::read_to_string(&paths.api_lock_path)
        && let Ok(endpoint) = serde_json::from_str::<Value>(&endpoint)
        && let (Some(port), Some(token)) = (
            endpoint.get("port").and_then(Value::as_u64),
            endpoint.get("token").and_then(Value::as_str),
        )
    {
        let response = reqwest::Client::new()
            .post(format!(
                "http://127.0.0.1:{port}/api/proposals?workspace_id={workspace_id}"
            ))
            .bearer_auth(token)
            .json(&input)
            .send()
            .await;
        if let Ok(response) = response
            && response.status().is_success()
        {
            return response.json::<Value>().await.context("proposal response");
        }
    }
    let proposal = crate::db::proposals::create(pool, workspace_id, input).await?;
    Ok(serde_json::to_value(proposal)?)
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("argumento '{key}' é obrigatório"))
}

fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "propose_agent",
            "description": "Propõe criar/atualizar um agente no workspace. Requer aprovação humana.",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "mode": { "type": "string" },
                    "model": { "type": "string" },
                    "color": { "type": "string" },
                    "avatar": { "type": "string" },
                    "system": { "type": "string" },
                    "scope": { "type": "string", "enum": ["workspace", "global"] }
                }
            }
        }),
        json!({
            "name": "propose_skill",
            "description": "Propõe criar/atualizar uma skill no workspace. Requer aprovação humana.",
            "inputSchema": {
                "type": "object",
                "required": ["id", "body"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "body": { "type": "string" },
                    "scope": { "type": "string", "enum": ["workspace", "global"] }
                }
            }
        }),
        json!({
            "name": "propose_command",
            "description": "Propõe criar/atualizar um comando no workspace. Requer aprovação humana."
        }),
        json!({
            "name": "propose_recipe",
            "description": "Propõe criar/atualizar uma receita YAML. Requer aprovação humana."
        }),
        json!({
            "name": "propose_routine",
            "description": "Propõe criar uma rotina/cron no workspace atual. Requer aprovação humana."
        }),
        json!({
            "name": "propose_mcp",
            "description": "Propõe adicionar/remover um servidor MCP. Requer aprovação humana."
        }),
        json!({
            "name": "propose_config",
            "description": "Propõe alterar um arquivo dentro de .opencode. Requer aprovação humana."
        }),
        json!({
            "name": "propose_change",
            "description": "Propõe qualquer mudança de configuração com kind/action/title/payload.",
            "inputSchema": {
                "type": "object",
                "required": ["kind", "title", "payload"],
                "properties": {
                    "kind": { "type": "string" },
                    "action": { "type": "string" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "payload": { "type": "object" }
                }
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;
    use tempfile::TempDir;

    #[test]
    fn exposes_proposal_tools() {
        let names = tools()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>();
        assert!(names.contains(&"propose_agent".to_string()));
        assert!(names.contains(&"propose_recipe".to_string()));
        assert!(!names.contains(&"propose_route".to_string()));
    }

    #[tokio::test]
    async fn propose_agent_falls_back_to_database() {
        let temp = TempDir::new().expect("temp");
        let paths = AppPaths {
            data_dir: temp.path().join("data"),
            config_dir: temp.path().join("config"),
            cache_dir: temp.path().join("cache"),
            log_dir: temp.path().join("logs"),
            db_path: temp.path().join("data/opendesk.db"),
            backups_dir: temp.path().join("backups"),
            artifacts_dir: temp.path().join("artifacts"),
            run_logs_dir: temp.path().join("runs"),
            daemon_lock_path: temp.path().join("daemon.json"),
            api_lock_path: temp.path().join("api.json"),
        };
        for dir in [
            &paths.data_dir,
            &paths.config_dir,
            &paths.cache_dir,
            &paths.log_dir,
            &paths.backups_dir,
            &paths.artifacts_dir,
            &paths.run_logs_dir,
        ] {
            std::fs::create_dir_all(dir).expect("dir");
        }
        let pool = crate::db::init(&paths.db_path).await.expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        let value = propose(
            &paths,
            &pool,
            "ws-1",
            "propose_agent",
            json!({"id": "auto", "name": "Auto", "mode": "subagent"}),
        )
        .await
        .expect("propose");
        assert_eq!(value["kind"], "AGENT");
        assert_eq!(value["status"], "PENDING");
    }
}
