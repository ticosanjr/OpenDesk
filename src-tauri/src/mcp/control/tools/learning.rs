use super::tool;
use serde_json::{Value, json};

pub(super) fn learning_save() -> Value {
    tool(
        "learning_save",
        "Salva um aprendizado do agente no workspace. Use para preferências, lições e pedidos de memorizar. remember=true grava também no MEMORY.md. Nunca use esta ferramenta para contexto pessoal; para isso existe personal_context_save, somente com pedido explícito do usuário.",
        json!({
            "type": "object",
            "required": ["title", "detail"],
            "properties": {
                "title": { "type": "string", "description": "Título curto do aprendizado." },
                "detail": { "type": "string", "description": "O que deve ser lembrado, com contexto suficiente." },
                "category": { "type": "string", "description": "Categoria livre; padrão manual." },
                "severity": { "type": "string", "enum": ["info", "warning", "critical"] },
                "remember": { "type": "boolean", "description": "true quando o usuário pedir para memorizar; grava no MEMORY.md." },
                "agent_name": { "type": "string" },
                "workspace_id": { "type": "string", "description": "Workspace alvo; vazio usa o ativo." }
            }
        }),
    )
}

pub(super) fn personal_context_save() -> Value {
    tool(
        "personal_context_save",
        "Salva uma regra no contexto pessoal. SÓ use quando o usuário pedir explicitamente para salvar no contexto pessoal; copie o pedido literal em user_request. Para memorizações genéricas use learning_save.",
        json!({
            "type": "object",
            "required": ["instructions", "user_request"],
            "properties": {
                "instructions": { "type": "string", "description": "A regra que deve entrar nos prompts." },
                "user_request": { "type": "string", "description": "Citação literal do pedido explícito do usuário para salvar no contexto pessoal." },
                "scope": { "type": "string", "enum": ["workspace", "global"] },
                "name": { "type": "string", "description": "Rótulo interno opcional." },
                "workspace_id": { "type": "string", "description": "Workspace alvo; vazio usa o ativo." }
            }
        }),
    )
}
