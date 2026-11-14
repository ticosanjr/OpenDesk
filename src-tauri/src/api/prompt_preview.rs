use crate::{
    app_state::AppState,
    config_sync::{agents, paths::StudioPaths, skills},
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
pub struct PromptSection {
    pub id: String,
    pub label: String,
    pub source: String,
    pub content: String,
}

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
    pub sections: Vec<PromptSection>,
    pub opencode: Value,
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
    let packed = dsh_router::pack(&PackContext {
        workspace_root: std::path::Path::new(&workspace.root_path),
        output_directory: &decision.output_directory,
        task_title: &task.title,
        task_description: &task.description,
        tags: &tags,
        priority: task.priority,
        phase: &decision.phase,
        plan: task.plan_markdown.as_deref(),
        agent: &decision.selected_agent,
        needs_browser: crate::browse::needs_browser(&task.title, &task.description, &tags),
        personal_context: &personal_context,
    });
    let prompt = format!(
        "{packed}{}\n\n## Contexto OpenDesk\n\nTask ID: `{}`. Se faltar informação essencial para continuar, use a ferramenta MCP `ask_user` com este task_id antes de assumir.\n",
        crate::api::tasks::task_dispatch::care_instructions(&task.care_level),
        task.id
    );
    let sections = split_sections(&prompt);
    let paths = StudioPaths::new(Some(std::path::PathBuf::from(&workspace.root_path)));
    let mut open_agent = None;
    let mut open_skills = Vec::new();
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
                    }),
            );
        }
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
            "skills": open_skills,
            "note": "O agente e as skills são carregados pelo OpenCode; o texto acima é o prompt enviado pelo OpenDesk."
        }),
    }))
}

fn split_sections(prompt: &str) -> Vec<PromptSection> {
    let mut sections = Vec::new();
    let mut current: Option<(String, String, String)> = None;
    let mut buffer = String::new();
    for line in prompt.lines() {
        let heading = line.starts_with("# ");
        let subheading = line.starts_with("## ");
        if heading || subheading {
            if let Some((id, label, source)) = current.take() {
                sections.push(PromptSection {
                    id,
                    label,
                    source,
                    content: buffer.trim().to_string(),
                });
            }
            buffer.clear();
            let text = line.trim_start_matches('#').trim().to_string();
            let (id, source) = source_for(&text);
            current = Some((id.to_string(), text, source.to_string()));
        }
        buffer.push_str(line);
        buffer.push('\n');
    }
    if let Some((id, label, source)) = current {
        sections.push(PromptSection {
            id,
            label,
            source,
            content: buffer.trim().to_string(),
        });
    } else if !buffer.trim().is_empty() {
        sections.push(PromptSection {
            id: "base".into(),
            label: "Prompt".into(),
            source: "OpenDesk".into(),
            content: buffer.trim().to_string(),
        });
    }
    sections
}

fn source_for(label: &str) -> (&'static str, &'static str) {
    let lower = label.to_lowercase();
    if lower.contains("instruções permanentes") || lower == "critérios de conclusão" {
        ("permanent", "OpenDesk")
    } else if lower.contains("contexto pessoal") {
        ("personal", "Contexto pessoal")
    } else if lower.contains("memória") {
        ("memory", "Aprendizado/MEMORY.md")
    } else if lower.contains("tarefa") || lower == "descrição" {
        ("task", "Tarefa")
    } else if lower.contains("diretório de saída") {
        ("output", "Workspace")
    } else if lower.contains("arquivos de entrada") {
        ("inputs", "Workspace")
    } else if lower.contains("instruções da fase") || lower.contains("plano aprovado") {
        ("phase", "Roteador/Plano")
    } else if lower.contains("opencode browse") {
        ("browser", "OpenCode Browse")
    } else if lower.contains("nível de cuidado") {
        ("care", "Nível de cuidado")
    } else if lower.contains("contexto opendesk") {
        ("context", "OpenDesk HITL")
    } else {
        ("other", "Outro")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_prompt_by_origin() {
        let sections = split_sections(
            "# OpenDesk — instruções permanentes\nA\n\n## Contexto pessoal do usuário\nB\n\n# Tarefa OpenDesk\nC\n\n## Contexto OpenDesk\nD\n",
        );
        assert!(sections.iter().any(|s| s.id == "permanent"));
        assert!(sections.iter().any(|s| s.id == "personal"));
        assert!(sections.iter().any(|s| s.id == "task"));
        assert!(sections.iter().any(|s| s.id == "context"));
    }
}
