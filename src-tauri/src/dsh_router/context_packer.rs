use crate::dsh_router::prompt_templates::{PromptTemplates, render};
use serde::Serialize;
use std::path::Path;
use walkdir::WalkDir;

pub struct PackContext<'a> {
    pub workspace_root: &'a Path,
    pub output_directory: &'a str,
    pub task_id: &'a str,
    pub task_title: &'a str,
    pub task_description: &'a str,
    pub tags: &'a [String],
    pub priority: i64,
    pub phase: &'a str,
    pub plan: Option<&'a str>,
    pub agent: &'a str,
    pub care_level: &'a str,
    pub needs_browser: bool,
    pub personal_context: &'a str,
    /// Rendered notes from the structured memory store, already filtered by the
    /// agent's ACL.
    pub structured_memory: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptSection {
    pub id: String,
    pub label: String,
    pub source: String,
    pub content: String,
}

pub fn pack(context: &PackContext<'_>, templates: &PromptTemplates) -> String {
    pack_sections(context, templates)
        .into_iter()
        .map(|section| section.content)
        .filter(|content| !content.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn pack_sections(context: &PackContext<'_>, templates: &PromptTemplates) -> Vec<PromptSection> {
    let input_files = list_input_files(context.workspace_root, 30);
    let mut sections = Vec::new();
    let mut push = |id: &str, label: &str, source: &str, content: String| {
        if !content.trim().is_empty() {
            sections.push(PromptSection {
                id: id.to_string(),
                label: label.to_string(),
                source: source.to_string(),
                content: content.trim().to_string(),
            });
        }
    };

    push(
        "permanent",
        "Instruções permanentes",
        "OpenDesk",
        render(&templates.permanent, &[]),
    );
    push(
        "completion",
        "Critérios de conclusão",
        "OpenDesk",
        render(&templates.completion, &[]),
    );

    if !context.personal_context.trim().is_empty() {
        push(
            "personal",
            "Contexto pessoal",
            "Contexto pessoal",
            render(
                &templates.personal_block,
                &[("personal_context", context.personal_context.trim())],
            ),
        );
    }

    if let Ok(memory) =
        std::fs::read_to_string(context.workspace_root.join(".opencode").join("MEMORY.md"))
    {
        let memory = memory.trim();
        if !memory.is_empty() {
            let memory = memory.chars().take(8_000).collect::<String>();
            push(
                "memory",
                "Memória do workspace",
                "Aprendizado/MEMORY.md",
                render(&templates.memory_block, &[("memory", &memory)]),
            );
        }
    }

    if !context.structured_memory.trim().is_empty() {
        push(
            "structured_memory",
            "Memória estruturada",
            "Memória/ACL",
            render(
                &templates.structured_memory_block,
                &[("structured_memory", context.structured_memory.trim())],
            ),
        );
    }

    let tags_line = if context.tags.is_empty() {
        String::new()
    } else {
        format!("**Tags:** {}\n", context.tags.join(", "))
    };
    push(
        "task",
        "Tarefa",
        "Tarefa",
        render(
            &templates.task_meta,
            &[
                ("task_title", context.task_title),
                ("priority", &context.priority.to_string()),
                ("tags", &tags_line),
                ("phase", context.phase),
                ("agent", context.agent),
                ("workspace", &context.workspace_root.display().to_string()),
            ],
        ),
    );
    push(
        "description",
        "Descrição",
        "Tarefa",
        render(
            &templates.description,
            &[("task_description", context.task_description.trim())],
        ),
    );

    if context.needs_browser {
        let allowlist = crate::browse::allowlist_from_tags(context.tags);
        let allow_line = if allowlist.is_empty() {
            "- Nenhuma allowlist configurada; prefira sites públicos e pare em login, 2FA ou pagamento.\n".to_string()
        } else {
            format!(
                "- Use `session_allowlist` com exatamente estes domínios: {}.\n",
                allowlist.join(", ")
            )
        };
        push(
            "browser",
            "Navegação web",
            "OpenCode Browse",
            render(&templates.browse, &[("allowlist_line", &allow_line)]),
        );
    }

    push(
        "output",
        "Diretório de saída",
        "Workspace",
        render(
            &templates.output,
            &[("output_directory", context.output_directory)],
        ),
    );

    if !input_files.is_empty() {
        let list = input_files
            .iter()
            .map(|file| format!("- `{file}`"))
            .collect::<Vec<_>>()
            .join("\n");
        push(
            "inputs",
            "Arquivos de entrada",
            "Workspace",
            render(&templates.inputs, &[("input_files", &list)]),
        );
    }

    match context.phase {
        "plan" => push(
            "plan",
            "Fase PLAN",
            "Roteador",
            render(&templates.plan_phase, &[]),
        ),
        "audit" => push(
            "audit",
            "Fase AUDIT",
            "Roteador",
            render(
                &templates.audit_phase,
                &[("output_directory", context.output_directory)],
            ),
        ),
        _ => {
            let plan_block = context
                .plan
                .filter(|plan| !plan.trim().is_empty())
                .map(|plan| format!("### Plano aprovado\n\n{}\n\n", plan.trim()))
                .unwrap_or_default();
            push(
                "build",
                "Fase BUILD",
                "Roteador/Plano",
                render(&templates.build_phase, &[("plan_block", &plan_block)]),
            );
        }
    }

    push(
        "care",
        "Nível de cuidado",
        "Nível de cuidado",
        render(templates.care(context.care_level), &[]),
    );
    push(
        "context",
        "Contexto OpenDesk",
        "OpenDesk HITL",
        render(&templates.opendesk_context, &[("task_id", context.task_id)]),
    );
    push(
        "memory_policy",
        "Política de memória",
        "Aprendizado/MCP",
        render(&templates.memory_policy, &[]),
    );

    sections
}

fn list_input_files(root: &Path, limit: usize) -> Vec<String> {
    let input = root.join("input");
    if !input.exists() {
        return Vec::new();
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&input).max_depth(4).follow_links(false) {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if let Ok(relative) = entry.path().strip_prefix(root) {
            files.push(relative.to_string_lossy().replace('\\', "/"));
            if files.len() >= limit {
                break;
            }
        }
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn context_pack_contains_output_rule_and_plan() {
        let temp = TempDir::new().expect("temp");
        std::fs::create_dir_all(temp.path().join("input")).expect("input");
        std::fs::write(temp.path().join("input/brief.md"), "brief").expect("brief");
        let prompt = pack(
            &PackContext {
                workspace_root: temp.path(),
                output_directory: "/tmp/out/2026-11/task",
                task_id: "task-1",
                care_level: "standard",
                task_title: "Relatório",
                task_description: "Produza o relatório.",
                tags: &["relatorio".into()],
                priority: 2,
                phase: "build",
                plan: Some("1. Ler input"),
                agent: "build",
                needs_browser: false,
                personal_context: "",
                structured_memory: "",
            },
            &PromptTemplates::default(),
        );
        assert!(prompt.contains("DIRETÓRIO DE SAÍDA OBRIGATÓRIO"));
        assert!(prompt.contains("/tmp/out/2026-11/task"));
        assert!(prompt.contains("Plano aprovado"));
        assert!(prompt.contains("input/brief.md"));
        assert!(prompt.contains("Contexto OpenDesk"));
        assert!(prompt.contains("Nível de cuidado: padrão"));

        let browser_prompt = pack(
            &PackContext {
                workspace_root: temp.path(),
                output_directory: "/tmp/out",
                task_id: "task-2",
                care_level: "quick",
                task_title: "Pesquisa",
                task_description: "Abra https://example.com",
                tags: &["needs-browser".into()],
                priority: 2,
                phase: "build",
                plan: None,
                agent: "build",
                needs_browser: true,
                personal_context: "",
                structured_memory: "",
            },
            &PromptTemplates::default(),
        );
        assert!(browser_prompt.contains("OpenCode Browse"));
        assert!(browser_prompt.contains("session_new"));
    }
}
