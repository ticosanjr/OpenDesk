use crate::{
    app_state::AppState,
    config_sync::{agents, commands, jsonc_editor, paths::StudioPaths, skills},
    db::{tasks, workspaces},
    dsh_router::{self, PackContext},
    error::AppResult,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct OpenCodeAgent {
    pub id: String,
    pub scope: String,
    pub path: String,
    pub system: String,
}

#[derive(Debug, Serialize)]
pub struct OpenCodeSkill {
    pub id: String,
    pub scope: String,
    pub name: String,
    pub description: Option<String>,
    pub path: String,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct OpenCodeCommand {
    pub id: String,
    pub scope: String,
    pub description: Option<String>,
    pub agent: Option<String>,
    pub template: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct PromptPreview {
    pub task_id: String,
    pub workspace_id: String,
    pub phase: String,
    pub agent: String,
    pub model: String,
    pub output_directory: String,
    pub prompt: String,
    pub sections: Vec<crate::dsh_router::context_packer::PromptSection>,
    pub opencode: Value,
    /// OpenDesk prompt plus everything read from the OpenCode native files. It
    /// is a reconstruction, not the provider wire payload.
    pub composed: String,
}

pub async fn preview(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<PromptPreview>> {
    let task = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let tags = task.tags();
    let body = crate::api::tasks::DispatchBody {
        policy: None,
        manual_agent: task
            .agent_override
            .then(|| task.assigned_agent.clone())
            .flatten(),
        manual_model: task
            .model_override
            .then(|| task.preferred_model.clone())
            .flatten(),
        manual_effort: None,
        execution_mode: Some(task.execution_mode.clone()),
        prompt_append: None,
    };
    let decision = crate::api::tasks::task_dispatch::plan_decision(
        &state,
        &task,
        &workspace,
        &body,
        task.plan_markdown.as_deref(),
    )
    .await?;
    let personal_context = crate::personal_context::render(&state.db, &task.workspace_id).await?;
    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &state.db,
        Some(&task.workspace_id),
    )
    .await?;
    let context = PackContext {
        workspace_root: std::path::Path::new(&workspace.root_path),
        output_directory: &decision.output_directory,
        task_id: &task.id,
        task_title: &task.title,
        task_description: &task.description,
        tags: &tags,
        priority: task.priority,
        phase: &decision.phase,
        plan: task.plan_markdown.as_deref(),
        agent: &decision.selected_agent,
        care_level: &task.care_level,
        needs_browser: crate::browse::needs_browser(&task.title, &task.description, &tags),
        personal_context: &personal_context,
    };
    let sections = dsh_router::context_packer::pack_sections(&context, &templates);
    let prompt = sections
        .iter()
        .map(|section| section.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let paths = StudioPaths::new(Some(std::path::PathBuf::from(&workspace.root_path)));
    let mut open_agent = None;
    let mut open_skills = Vec::new();
    let mut open_commands = Vec::new();
    let mut open_mcp = Vec::new();
    for scope in ["workspace", "global"] {
        if open_agent.is_none()
            && let Ok(items) = agents::list(&paths, scope)
            && let Some(agent) = items
                .into_iter()
                .find(|agent| agent.id == decision.selected_agent)
        {
            open_agent = Some(OpenCodeAgent {
                id: agent.id,
                scope: agent.scope,
                path: agent.path,
                system: agent.system,
            });
        }
        if let Ok(items) = skills::list(&paths, scope) {
            open_skills.extend(
                items
                    .into_iter()
                    .filter(|skill| !skill.disabled)
                    .map(|skill| OpenCodeSkill {
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
            open_commands.extend(items.into_iter().filter(|command| !command.disabled).map(
                |command| OpenCodeCommand {
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
                open_mcp.push(serde_json::json!({
                    "scope": scope,
                    "server": server,
                }));
            }
        }
    }

    let mut agents_md = Vec::new();
    for (scope, path) in [
        (
            "workspace",
            std::path::Path::new(&workspace.root_path).join("AGENTS.md"),
        ),
        ("global", paths.global_dir.join("AGENTS.md")),
    ] {
        if let Ok(content) = std::fs::read_to_string(&path) {
            agents_md.push(serde_json::json!({
                "scope": scope,
                "path": path.to_string_lossy(),
                "content": content,
            }));
        }
    }

    let mut composed = prompt.clone();
    if let Some(agent) = &open_agent
        && !agent.system.trim().is_empty()
    {
        composed.push_str(&format!(
            "\n\n## OpenCode — prompt do agente @{}\n\n{}",
            agent.id,
            agent.system.trim()
        ));
    }
    for file in &agents_md {
        let scope = file.get("scope").and_then(Value::as_str).unwrap_or("");
        let content = file.get("content").and_then(Value::as_str).unwrap_or("");
        if !content.trim().is_empty() {
            composed.push_str(&format!(
                "\n\n## OpenCode — AGENTS.md ({scope})\n\n{}",
                content.trim()
            ));
        }
    }
    for skill in &open_skills {
        let body = skill.body.trim();
        let body = body.chars().take(8_000).collect::<String>();
        composed.push_str(&format!(
            "\n\n## OpenCode — skill {} ({})\n\n{}",
            skill.name, skill.scope, body
        ));
    }
    for command in &open_commands {
        composed.push_str(&format!(
            "\n\n## OpenCode — comando /{} ({})\n\n{}",
            command.id,
            command.scope,
            command.template.trim()
        ));
    }

    Ok(Json(PromptPreview {
        task_id: task.id,
        workspace_id: task.workspace_id,
        phase: decision.phase,
        agent: decision.selected_agent,
        model: decision.selected_model,
        output_directory: decision.output_directory,
        prompt,
        sections,
        opencode: serde_json::json!({
            "agent": open_agent,
            "agents_md": agents_md,
            "skills": open_skills,
            "commands": open_commands,
            "mcp_servers": open_mcp,
            "note": "O OpenCode monta o payload do provedor internamente. Esta visão reúne o prompt exato do OpenDesk e tudo o que o OpenCode lê da configuração nativa; o system prompt interno e os schemas de ferramentas não são expostos pela API."
        }),
        composed,
    }))
}
