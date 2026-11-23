use super::*;

pub async fn runtime(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let workspace_id = match query.workspace_id {
        Some(id) => id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    let workspace = crate::db::workspaces::get(&state.db, &workspace_id).await?;
    let runtime_root = query
        .location
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| workspace.root_path.clone());
    let directory = Some(runtime_root.as_str());
    let client = state.daemon.client().await;
    let (agents, models, skills, commands, mcps) = match client {
        Some(client) => {
            let mut agents = client.agents(None).await.unwrap_or_default();
            let local_agents = client.agents(directory).await.unwrap_or_default();
            agents.extend(local_agents);
            let mut agents = dedupe_by(agents, |agent| agent.id.clone());

            let models = client
                .models(directory)
                .await
                .unwrap_or_else(|_| Vec::new());
            let mut skills = client.skills(None).await.unwrap_or_default();
            skills.extend(client.skills(directory).await.unwrap_or_default());
            let mut skills = dedupe_by(skills, |skill| {
                skill
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            let mut commands = client.commands(None).await.unwrap_or_default();
            commands.extend(client.commands(directory).await.unwrap_or_default());
            let mut commands = dedupe_by(commands, |command| {
                command
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            let mut mcps = client.mcp_servers(None).await.unwrap_or_default();
            mcps.extend(client.mcp_servers(directory).await.unwrap_or_default());
            let mut mcps = dedupe_by(mcps, |mcp| {
                mcp.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            skills.sort_by_key(|value| {
                value
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            commands.sort_by_key(|value| {
                value
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            mcps.sort_by_key(|value| {
                value
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            agents.sort_by_key(|agent| agent.id.clone());
            (agents, models, skills, commands, mcps)
        }
        None => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
    };
    let status = state.daemon.status().await;
    Ok(Json(json!({
        "engine": status,
        "agents": agents,
        "models": models,
        "skills": skills,
        "commands": commands,
        "mcps": mcps,
    })))
}
