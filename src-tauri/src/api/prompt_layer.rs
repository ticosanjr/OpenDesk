use crate::config_sync::{agents, commands, jsonc_editor, paths::StudioPaths, skills};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct LayerAgent {
    pub id: String,
    pub scope: String,
    pub path: String,
    pub system: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerFile {
    pub scope: String,
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerSkill {
    pub id: String,
    pub scope: String,
    pub name: String,
    pub description: Option<String>,
    pub path: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerCommand {
    pub id: String,
    pub scope: String,
    pub description: Option<String>,
    pub agent: Option<String>,
    pub template: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenCodeLayer {
    pub agent: Option<LayerAgent>,
    pub agents_md: Vec<LayerFile>,
    pub skills: Vec<LayerSkill>,
    pub commands: Vec<LayerCommand>,
    pub mcp_servers: Vec<Value>,
    pub note: String,
}

/// Everything OpenDesk can read from the native OpenCode configuration for the
/// workspace/agent. The internal system prompt and tool schemas are server-side
/// and intentionally not fabricated here.
pub fn snapshot(workspace_root: &Path, agent_id: &str) -> OpenCodeLayer {
    let paths = StudioPaths::new(Some(workspace_root.to_path_buf()));
    let mut agent = None;
    let mut skills_list = Vec::new();
    let mut commands_list = Vec::new();
    let mut mcp_servers = Vec::new();
    for scope in ["workspace", "global"] {
        if agent.is_none()
            && let Ok(items) = agents::list(&paths, scope)
            && let Some(found) = items.into_iter().find(|item| item.id == agent_id)
        {
            agent = Some(LayerAgent {
                id: found.id,
                scope: found.scope,
                path: found.path,
                system: found.system,
            });
        }
        if let Ok(items) = skills::list(&paths, scope) {
            skills_list.extend(
                items
                    .into_iter()
                    .filter(|skill| !skill.disabled)
                    .map(|skill| LayerSkill {
                        id: skill.id,
                        scope: skill.scope,
                        name: skill.name,
                        description: skill.description,
                        path: skill.path,
                        body: skill.body,
                    }),
            );
        }
        if let Ok(items) = commands::list(&paths, scope) {
            commands_list.extend(items.into_iter().filter(|command| !command.disabled).map(
                |command| LayerCommand {
                    id: command.id,
                    scope: command.scope,
                    description: command.description,
                    agent: command.agent,
                    template: command.template,
                    path: command.path,
                },
            ));
        }
        if let Ok(document) = jsonc_editor::read_for_scope(&paths, scope) {
            for server in jsonc_editor::list_mcp_servers(&document) {
                mcp_servers.push(json!({ "scope": scope, "server": server }));
            }
        }
    }

    let mut agents_md = Vec::new();
    for (scope, path) in [
        ("workspace", workspace_root.join("AGENTS.md")),
        ("global", paths.global_dir.join("AGENTS.md")),
    ] {
        if let Ok(content) = std::fs::read_to_string(&path) {
            agents_md.push(LayerFile {
                scope: scope.into(),
                path: path.to_string_lossy().to_string(),
                content,
            });
        }
    }

    OpenCodeLayer {
        agent,
        agents_md,
        skills: skills_list,
        commands: commands_list,
        mcp_servers,
        note: "O OpenCode monta o payload do provedor internamente. Esta visão reúne o prompt exato do OpenDesk e tudo o que o OpenCode lê da configuração nativa; o system prompt interno e os schemas de ferramentas não são expostos pela API.".into(),
    }
}

/// Concatenates the OpenDesk prompt with the readable OpenCode layer. It is a
/// reconstruction, not the provider wire payload.
pub fn compose(base: &str, layer: &OpenCodeLayer) -> String {
    let mut composed = base.to_string();
    if let Some(agent) = &layer.agent
        && !agent.system.trim().is_empty()
    {
        composed.push_str(&format!(
            "\n\n## OpenCode — prompt do agente @{}\n\n{}",
            agent.id,
            agent.system.trim()
        ));
    }
    for file in &layer.agents_md {
        if !file.content.trim().is_empty() {
            composed.push_str(&format!(
                "\n\n## OpenCode — AGENTS.md ({})\n\n{}",
                file.scope,
                file.content.trim()
            ));
        }
    }
    for skill in &layer.skills {
        let body = skill.body.trim().chars().take(8_000).collect::<String>();
        composed.push_str(&format!(
            "\n\n## OpenCode — skill {} ({})\n\n{}",
            skill.name, skill.scope, body
        ));
    }
    for command in &layer.commands {
        composed.push_str(&format!(
            "\n\n## OpenCode — comando /{} ({})\n\n{}",
            command.id,
            command.scope,
            command.template.trim()
        ));
    }
    composed
}
