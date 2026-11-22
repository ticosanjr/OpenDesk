use crate::error::AppResult;
use serde::{Deserialize, Serialize};

mod fields;
mod render;
mod store;
mod variables;

#[cfg(test)]
mod tests;

pub use fields::{FIELDS, PromptTemplateField};
pub use render::render;
pub use store::{load, reset, save};
pub use variables::{PromptVariable, VARIABLES};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptTemplates {
    pub permanent: String,
    pub completion: String,
    pub personal_block: String,
    pub memory_block: String,
    pub structured_memory_block: String,
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
    pub execution_instruction: String,
    pub memory_policy: String,
    pub recipe_feedback: String,
    pub model_test: String,
}

impl Default for PromptTemplates {
    fn default() -> Self {
        Self {
            permanent: "# OpenDesk — instruções permanentes\n\nVocê executa tarefas governadas pelo OpenDesk. Salve todos os arquivos finais, relatórios e entregáveis no diretório de saída informado abaixo. Nunca espalhe arquivos finais na raiz do projeto. Arquivos de configuração nativos do OpenCode (opencode.jsonc, .opencode/) não contam como entregáveis.".into(),
            completion: "## Critérios de conclusão\n\n- O resultado está salvo no diretório de saída.\n- O resumo final cita os caminhos reais dos arquivos.\n- Nenhum arquivo final foi deixado na raiz do projeto.".into(),
            personal_block: "## Contexto pessoal do usuário\n\n{{personal_context}}".into(),
            memory_block: "## Memória do workspace (regras aprendidas)\n\n{{memory}}".into(),
            structured_memory_block: "## Memória estruturada relevante\n\n{{structured_memory}}".into(),
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
            execution_instruction: "## Instruções desta execução\n\n{{instruction}}".into(),
            memory_policy: "## Memória e contexto pessoal\n\nPara aprendizados, preferências e pedidos de \"memorize\", use `learning_save` (com remember=true quando o usuário pedir para memorizar).\nSó use `personal_context_save` quando o usuário pedir explicitamente para salvar no contexto pessoal. Caso contrário, salve como aprendizado.".into(),
            recipe_feedback: "## Correção do usuário\n\n{{feedback}}\n\nRefaça o step considerando a correção.".into(),
            model_test: "Output only: OK".into(),
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
