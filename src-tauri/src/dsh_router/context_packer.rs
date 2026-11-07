use std::path::Path;
use walkdir::WalkDir;

pub struct PackContext<'a> {
    pub workspace_root: &'a Path,
    pub output_directory: &'a str,
    pub task_title: &'a str,
    pub task_description: &'a str,
    pub tags: &'a [String],
    pub priority: i64,
    pub phase: &'a str,
    pub plan: Option<&'a str>,
    pub agent: &'a str,
}

pub fn pack(context: &PackContext<'_>) -> String {
    let input_files = list_input_files(context.workspace_root, 30);
    let mut prompt = String::new();

    // Keep the invariant prefix identical across tasks in the same workspace.
    // Providers cache on the longest stable prefix, so task data comes later.
    prompt.push_str("# OpenDesk — instruções permanentes\n\n");
    prompt.push_str(
        "Você executa tarefas governadas pelo OpenDesk. Salve todos os arquivos finais, relatórios e entregáveis no diretório de saída informado abaixo. Nunca espalhe arquivos finais na raiz do projeto. Arquivos de configuração nativos do OpenCode (opencode.jsonc, .opencode/) não contam como entregáveis.\n\n",
    );
    prompt.push_str("## Critérios de conclusão\n\n");
    prompt.push_str("- O resultado está salvo no diretório de saída.\n");
    prompt.push_str("- O resumo final cita os caminhos reais dos arquivos.\n");
    prompt.push_str("- Nenhum arquivo final foi deixado na raiz do projeto.\n\n");

    if let Ok(memory) =
        std::fs::read_to_string(context.workspace_root.join(".opencode").join("MEMORY.md"))
    {
        let memory = memory.trim();
        if !memory.is_empty() {
            prompt.push_str("## Memória do workspace (regras aprendidas)\n\n");
            prompt.push_str(&memory.chars().take(8_000).collect::<String>());
            prompt.push_str("\n\n");
        }
    }

    prompt.push_str("# Tarefa OpenDesk\n\n");
    prompt.push_str(&format!("**Título:** {}\n", context.task_title));
    prompt.push_str(&format!("**Prioridade:** {}\n", context.priority));
    if !context.tags.is_empty() {
        prompt.push_str(&format!("**Tags:** {}\n", context.tags.join(", ")));
    }
    prompt.push_str(&format!("**Fase:** {}\n", context.phase));
    prompt.push_str(&format!("**Agente:** @{}\n", context.agent));
    prompt.push_str(&format!(
        "**Workspace:** {}\n\n",
        context.workspace_root.display()
    ));

    prompt.push_str("## Descrição\n\n");
    prompt.push_str(context.task_description.trim());
    prompt.push_str("\n\n");

    prompt.push_str("## DIRETÓRIO DE SAÍDA OBRIGATÓRIO\n\n");
    prompt.push_str(&format!(
        "Todos os arquivos finais, relatórios e entregáveis desta tarefa DEVEM ser salvos dentro de:\n\n`{}`\n\n",
        context.output_directory
    ));

    if !input_files.is_empty() {
        prompt.push_str("## Arquivos de entrada disponíveis\n\n");
        prompt.push_str(
            "Leia apenas o que for relevante; não despeje o conteúdo de todos no contexto.\n\n",
        );
        for file in input_files {
            prompt.push_str(&format!("- `{file}`\n"));
        }
        prompt.push('\n');
    }

    if context.phase == "plan" {
        prompt.push_str("## Instruções da fase PLAN\n\n");
        prompt.push_str("- Não escreva nem edite arquivos nesta fase.\n");
        prompt.push_str("- Produza um plano de ação em Markdown com: objetivo, etapas ordenadas, arquivos a criar/alterar, riscos, dependências e critérios de aceite verificáveis.\n");
        prompt.push_str(
            "- Se houver ambiguidade, liste as decisões necessárias em vez de inventar fatos.\n",
        );
        prompt.push_str("- Termine com a seção `## Critérios de Aceite`.\n");
    } else if context.phase == "audit" {
        prompt.push_str("## Instruções da fase AUDIT\n\n");
        prompt.push_str("- Você é somente leitura: não crie, edite nem apague arquivos.\n");
        prompt.push_str("- Verifique os artefatos em ");
        prompt.push_str(&format!("`{}`", context.output_directory));
        prompt.push_str(" contra o plano e os critérios de aceite.\n");
        prompt.push_str("- Liste falhas, riscos e arquivos ausentes com caminho e evidência. Termine com veredito `APROVADO` ou `REPROVADO`.\n");
    } else {
        prompt.push_str("## Instruções da fase BUILD\n\n");
        if let Some(plan) = context.plan.filter(|plan| !plan.trim().is_empty()) {
            prompt.push_str("### Plano aprovado\n\n");
            prompt.push_str(plan.trim());
            prompt.push_str("\n\n");
        }
        prompt.push_str(
            "- Execute a tarefa e salve todos os entregáveis no diretório de saída obrigatório.\n",
        );
        prompt.push_str("- Prefira edições pequenas e verificáveis. Não altere arquivos fora do escopo sem justificar.\n");
        prompt.push_str("- Não remova dados do usuário. Se precisar de uma decisão destrutiva, pare e explique.\n");
        prompt.push_str("- Ao terminar, responda com um resumo curto, a lista de arquivos criados/modificados e como verificar o resultado.\n");
    }

    prompt
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
        let prompt = pack(&PackContext {
            workspace_root: temp.path(),
            output_directory: "/tmp/out/2026-11/task",
            task_title: "Relatório",
            task_description: "Produza o relatório.",
            tags: &["relatorio".into()],
            priority: 2,
            phase: "build",
            plan: Some("1. Ler input"),
            agent: "build",
        });
        assert!(prompt.contains("DIRETÓRIO DE SAÍDA OBRIGATÓRIO"));
        assert!(prompt.contains("/tmp/out/2026-11/task"));
        assert!(prompt.contains("Plano aprovado"));
        assert!(prompt.contains("input/brief.md"));
    }
}
