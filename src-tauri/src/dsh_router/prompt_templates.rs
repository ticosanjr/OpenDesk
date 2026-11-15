use crate::{
    db::settings,
    error::{AppError, AppResult},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const GLOBAL_KEY: &str = "prompt_templates";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptTemplates {
    pub permanent: String,
    pub completion: String,
    pub personal_block: String,
    pub memory_block: String,
    pub task_meta: String,
    pub description: String,
    pub output: String,
    pub inputs: String,
    pub plan_phase: String,
    pub build_phase: String,
    pub audit_phase: String,
    pub browse: String,
    pub care_quick: String,
    pub care_standard: String,
    pub care_careful: String,
    pub opendesk_context: String,
    pub chat_channel: String,
    pub follow_up: String,
}

impl Default for PromptTemplates {
    fn default() -> Self {
        Self {
            permanent: "# OpenDesk — instruções permanentes\n\nVocê executa tarefas governadas pelo OpenDesk. Salve todos os arquivos finais, relatórios e entregáveis no diretório de saída informado abaixo. Nunca espalhe arquivos finais na raiz do projeto. Arquivos de configuração nativos do OpenCode (opencode.jsonc, .opencode/) não contam como entregáveis.".into(),
            completion: "## Critérios de conclusão\n\n- O resultado está salvo no diretório de saída.\n- O resumo final cita os caminhos reais dos arquivos.\n- Nenhum arquivo final foi deixado na raiz do projeto.".into(),
            personal_block: "{{personal_context}}".into(),
            memory_block: "## Memória do workspace (regras aprendidas)\n\n{{memory}}".into(),
            task_meta: "# Tarefa OpenDesk\n\n**Título:** {{task_title}}\n**Prioridade:** {{priority}}\n{{tags}}**Fase:** {{phase}}\n**Agente:** @{{agent}}\n**Workspace:** {{workspace}}".into(),
            description: "## Descrição\n\n{{task_description}}".into(),
            output: "## DIRETÓRIO DE SAÍDA OBRIGATÓRIO\n\nTodos os arquivos finais, relatórios e entregáveis desta tarefa DEVEM ser salvos dentro de:\n\n`{{output_directory}}`".into(),
            inputs: "## Arquivos de entrada disponíveis\n\nLeia apenas o que for relevante; não despeje o conteúdo de todos no contexto.\n\n{{input_files}}".into(),
            plan_phase: "## Instruções da fase PLAN\n\n- Não escreva nem edite arquivos nesta fase.\n- Produza um plano de ação em Markdown com: objetivo, etapas ordenadas, arquivos a criar/alterar, riscos, dependências e critérios de aceite verificáveis.\n- Se houver ambiguidade, liste as decisões necessárias em vez de inventar fatos.\n- Termine com a seção `## Critérios de Aceite`.".into(),
            build_phase: "## Instruções da fase BUILD\n\n{{plan_block}}- Execute a tarefa e salve todos os entregáveis no diretório de saída obrigatório.\n- Prefira edições pequenas e verificáveis. Não altere arquivos fora do escopo sem justificar.\n- Não remova dados do usuário. Se precisar de uma decisão destrutiva, pare e explique.\n- Ao terminar, responda com um resumo curto, a lista de arquivos criados/modificados e como verificar o resultado.".into(),
            audit_phase: "## Instruções da fase AUDIT\n\n- Você é somente leitura: não crie, edite nem apague arquivos.\n- Verifique os artefatos em `{{output_directory}}` contra o plano e os critérios de aceite.\n- Liste falhas, riscos e arquivos ausentes com caminho e evidência. Termine com veredito `APROVADO` ou `REPROVADO`.".into(),
            browse: "## Navegação web (OpenCode Browse)\n\nEsta tarefa precisa navegar na internet. Use o servidor MCP `browse`, que controla o navegador real do usuário.\n\n- Comece por `status` e crie uma sessão com `session_new`; reutilize a sessão em todas as chamadas.\n- Use `execute` para navegar, clicar, preencher e extrair conteúdo.\n{{allowlist_line}}- Em login, 2FA, pagamento ou captcha, use o handoff e peça a ação ao usuário; não tente contornar.\n- Não use curl para páginas que dependem de JavaScript, login ou interação.\n- Salve screenshots, HARs e evidências no diretório de saída.\n- Ao terminar, feche a sessão e informe as URLs visitadas.".into(),
            care_quick: "\n\n## Nível de cuidado: rápido\n\nSeja direto. Não faça plano formal nem auditoria. Execute o necessário, faça no máximo uma validação objetiva e entregue o resultado. Evite exploração extensa de arquivos.".into(),
            care_standard: "\n\n## Nível de cuidado: padrão\n\nUse um checklist curto, execute o necessário e faça uma validação. Não transforme tarefas simples em um plano longo.".into(),
            care_careful: "\n\n## Nível de cuidado: cuidadoso\n\nRevise premissas, valide o resultado e registre riscos ou pendências. Prefira precisão a velocidade.".into(),
            opendesk_context: "## Contexto OpenDesk\n\nTask ID: `{{task_id}}`. Se faltar informação essencial para continuar, use a ferramenta MCP `ask_user` com este task_id antes de assumir.".into(),
            follow_up: "## Conversa com o usuário\n\n{{conversation}}\n\nContinue a execução considerando a mensagem acima.".into(),
            chat_channel: "Contexto do canal {{source}} (não repita ao usuário):\nVocê está operando o OpenDesk no workspace `{{workspace_id}}` e nesta conversa `{{thread_id}}`.\nUse as ferramentas MCP `opendesk-control` quando o pedido envolver o OpenDesk: tarefas, execuções, rotinas, receitas, fila, artefatos e decisão humana (`ask_user`).\n{{file_hint}}Nunca ative rotina/receita nem aprove HITL/permissão sem pedido explícito.\nResponda em português, de forma curta, citando IDs e caminhos relevantes.".into(),
        }
    }
}

impl PromptTemplates {
    pub async fn load(pool: &sqlx::SqlitePool, workspace_id: Option<&str>) -> AppResult<Self> {
        load(pool, workspace_id).await
    }

    pub async fn save(
        pool: &sqlx::SqlitePool,
        workspace_id: Option<&str>,
        templates: &Self,
    ) -> AppResult<()> {
        save(pool, workspace_id, templates).await
    }

    pub fn care(&self, level: &str) -> &str {
        match level {
            "quick" => &self.care_quick,
            "careful" => &self.care_careful,
            _ => &self.care_standard,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptTemplateField {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub placeholders: &'static [&'static str],
}

pub const FIELDS: &[PromptTemplateField] = &[
    PromptTemplateField {
        id: "permanent",
        label: "Instruções permanentes",
        description: "Prefixo estável de toda tarefa.",
        placeholders: &[],
    },
    PromptTemplateField {
        id: "completion",
        label: "Critérios de conclusão",
        description: "O que o agente deve garantir antes de terminar.",
        placeholders: &[],
    },
    PromptTemplateField {
        id: "personal_block",
        label: "Contexto pessoal",
        description: "Bloco que envolve as regras pessoais.",
        placeholders: &["personal_context"],
    },
    PromptTemplateField {
        id: "memory_block",
        label: "Memória do workspace",
        description: "Bloco de MEMORY.md.",
        placeholders: &["memory"],
    },
    PromptTemplateField {
        id: "task_meta",
        label: "Metadados da tarefa",
        description: "Título, prioridade, tags, fase, agente e workspace.",
        placeholders: &[
            "task_title",
            "priority",
            "tags",
            "phase",
            "agent",
            "workspace",
        ],
    },
    PromptTemplateField {
        id: "description",
        label: "Descrição da tarefa",
        description: "Onde entra o pedido do usuário.",
        placeholders: &["task_description"],
    },
    PromptTemplateField {
        id: "output",
        label: "Diretório de saída",
        description: "Regra obrigatória de entrega.",
        placeholders: &["output_directory"],
    },
    PromptTemplateField {
        id: "inputs",
        label: "Arquivos de entrada",
        description: "Lista de arquivos disponíveis no workspace.",
        placeholders: &["input_files"],
    },
    PromptTemplateField {
        id: "plan_phase",
        label: "Fase PLAN",
        description: "Instruções quando o roteador escolhe plano.",
        placeholders: &[],
    },
    PromptTemplateField {
        id: "build_phase",
        label: "Fase BUILD",
        description: "Instruções de execução; use plan_block para o plano aprovado.",
        placeholders: &["plan_block"],
    },
    PromptTemplateField {
        id: "audit_phase",
        label: "Fase AUDIT",
        description: "Instruções de auditoria.",
        placeholders: &["output_directory"],
    },
    PromptTemplateField {
        id: "browse",
        label: "Navegação web",
        description: "Instruções do OpenCode Browse; allowlist_line é dinâmica.",
        placeholders: &["allowlist_line"],
    },
    PromptTemplateField {
        id: "care_quick",
        label: "Cuidado: rápido",
        description: "Nível quick.",
        placeholders: &[],
    },
    PromptTemplateField {
        id: "care_standard",
        label: "Cuidado: padrão",
        description: "Nível standard.",
        placeholders: &[],
    },
    PromptTemplateField {
        id: "care_careful",
        label: "Cuidado: cuidadoso",
        description: "Nível careful.",
        placeholders: &[],
    },
    PromptTemplateField {
        id: "opendesk_context",
        label: "Contexto OpenDesk",
        description: "Task ID e instrução do ask_user.",
        placeholders: &["task_id"],
    },
];

pub async fn load(
    pool: &sqlx::SqlitePool,
    workspace_id: Option<&str>,
) -> AppResult<PromptTemplates> {
    if let Some(workspace_id) = workspace_id
        && let Some(value) = settings::get(pool, &workspace_key(workspace_id)).await?
    {
        return parse(value);
    }
    if let Some(value) = settings::get(pool, GLOBAL_KEY).await? {
        return parse(value);
    }
    Ok(PromptTemplates::default())
}

pub async fn save(
    pool: &sqlx::SqlitePool,
    workspace_id: Option<&str>,
    templates: &PromptTemplates,
) -> AppResult<()> {
    let value = serde_json::to_value(templates)
        .map_err(|error| AppError::Internal(format!("invalid prompt templates: {error}")))?;
    let key = workspace_id
        .map(workspace_key)
        .unwrap_or_else(|| GLOBAL_KEY.to_string());
    settings::set(pool, &key, &value).await
}

pub async fn reset(pool: &sqlx::SqlitePool, workspace_id: Option<&str>) -> AppResult<()> {
    match workspace_id {
        Some(id) => settings::delete(pool, &workspace_key(id)).await,
        None => settings::delete(pool, GLOBAL_KEY).await,
    }
}

fn workspace_key(workspace_id: &str) -> String {
    format!("{GLOBAL_KEY}:{workspace_id}")
}

fn parse(value: Value) -> AppResult<PromptTemplates> {
    serde_json::from_value(value)
        .map_err(|error| AppError::BadRequest(format!("prompt templates inválidos: {error}")))
}

/// Replaces `{{name}}` placeholders. Unknown placeholders are left untouched so
/// the user can see the typo in the preview instead of silently losing text.
pub fn render(template: &str, values: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in values {
        result = result.replace(&format!("{{{{{key}}}}}"), value);
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_placeholders_and_keeps_unknown_ones() {
        let output = render("Olá {{name}}, {{missing}}", &[("name", "Rafael")]);
        assert_eq!(output, "Olá Rafael, {{missing}}");
    }

    #[test]
    fn defaults_cover_the_documented_sections() {
        let templates = PromptTemplates::default();
        for field in FIELDS {
            let value = match field.id {
                "permanent" => &templates.permanent,
                "completion" => &templates.completion,
                "personal_block" => &templates.personal_block,
                "memory_block" => &templates.memory_block,
                "task_meta" => &templates.task_meta,
                "description" => &templates.description,
                "output" => &templates.output,
                "inputs" => &templates.inputs,
                "plan_phase" => &templates.plan_phase,
                "build_phase" => &templates.build_phase,
                "audit_phase" => &templates.audit_phase,
                "browse" => &templates.browse,
                "care_quick" => &templates.care_quick,
                "care_standard" => &templates.care_standard,
                "care_careful" => &templates.care_careful,
                "opendesk_context" => &templates.opendesk_context,
                "chat_channel" => &templates.chat_channel,
                "follow_up" => &templates.follow_up,
                _ => panic!("uncovered field {}", field.id),
            };
            assert!(!value.trim().is_empty(), "field {}", field.id);
        }
    }

    #[tokio::test]
    async fn saves_and_resets_workspace_templates() {
        let ctx = crate::test_support::context().await;
        let templates = PromptTemplates {
            output: "saída {{output_directory}}".into(),
            ..Default::default()
        };
        save(&ctx.state.db, Some(&ctx.workspace_id), &templates)
            .await
            .expect("save");
        let loaded = load(&ctx.state.db, Some(&ctx.workspace_id))
            .await
            .expect("load");
        assert!(loaded.output.contains("saída"));
        reset(&ctx.state.db, Some(&ctx.workspace_id))
            .await
            .expect("reset");
        let loaded = load(&ctx.state.db, Some(&ctx.workspace_id))
            .await
            .expect("load");
        assert_eq!(loaded.output, PromptTemplates::default().output);
    }
}
