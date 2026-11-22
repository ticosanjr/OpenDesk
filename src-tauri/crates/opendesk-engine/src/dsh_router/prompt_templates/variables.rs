use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PromptVariable {
    pub id: &'static str,
    pub description: &'static str,
    pub available: bool,
}

/// Variables accepted by the renderer. Future control-plane variables are
/// listed now so templates can be authored before the feature lands; unknown
/// placeholders stay visible in the preview.
pub const VARIABLES: &[PromptVariable] = &[
    PromptVariable {
        id: "task_title",
        description: "Título da tarefa.",
        available: true,
    },
    PromptVariable {
        id: "task_description",
        description: "Descrição da tarefa.",
        available: true,
    },
    PromptVariable {
        id: "priority",
        description: "Prioridade da tarefa.",
        available: true,
    },
    PromptVariable {
        id: "tags",
        description: "Linha de tags.",
        available: true,
    },
    PromptVariable {
        id: "phase",
        description: "Fase do roteador.",
        available: true,
    },
    PromptVariable {
        id: "agent",
        description: "Agente selecionado.",
        available: true,
    },
    PromptVariable {
        id: "workspace",
        description: "Caminho do workspace.",
        available: true,
    },
    PromptVariable {
        id: "output_directory",
        description: "Diretório de saída.",
        available: true,
    },
    PromptVariable {
        id: "input_files",
        description: "Lista de entradas.",
        available: true,
    },
    PromptVariable {
        id: "plan_block",
        description: "Bloco do plano aprovado.",
        available: true,
    },
    PromptVariable {
        id: "personal_context",
        description: "Regras pessoais.",
        available: true,
    },
    PromptVariable {
        id: "memory",
        description: "MEMORY.md do workspace.",
        available: true,
    },
    PromptVariable {
        id: "allowlist_line",
        description: "Linha de allowlist do Browse.",
        available: true,
    },
    PromptVariable {
        id: "task_id",
        description: "ID da tarefa.",
        available: true,
    },
    PromptVariable {
        id: "conversation",
        description: "Mensagem de continuação.",
        available: true,
    },
    PromptVariable {
        id: "instruction",
        description: "Instrução avulsa.",
        available: true,
    },
    PromptVariable {
        id: "source",
        description: "Canal do chat.",
        available: true,
    },
    PromptVariable {
        id: "workspace_id",
        description: "ID do workspace do chat.",
        available: true,
    },
    PromptVariable {
        id: "thread_id",
        description: "ID da conversa.",
        available: true,
    },
    PromptVariable {
        id: "file_hint",
        description: "Instrução de envio de arquivo.",
        available: true,
    },
    PromptVariable {
        id: "feedback",
        description: "Correção de step de receita.",
        available: true,
    },
    PromptVariable {
        id: "memory.core",
        description: "Core Memory compacta (v1.0).",
        available: false,
    },
    PromptVariable {
        id: "memory.context",
        description: "Contexto recuperado do vault (v1.0).",
        available: false,
    },
    PromptVariable {
        id: "handoff.contract",
        description: "Contrato do handoff atual (v1.0).",
        available: false,
    },
    PromptVariable {
        id: "channel.trust_level",
        description: "Trust level da entrada (v1.0).",
        available: false,
    },
    PromptVariable {
        id: "harness",
        description: "Adapter/harness selecionado (v1.0).",
        available: false,
    },
];
