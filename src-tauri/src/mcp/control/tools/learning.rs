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

pub(super) fn personal_context_list() -> Value {
    tool(
        "personal_context_list",
        "Lista regras do contexto pessoal do workspace. Cada item informa usage_scope: 'normal' | 'chat' | 'both'.",
        json!({
            "type": "object",
            "properties": {
                "workspace_id": { "type": "string", "description": "Workspace alvo; vazio usa o ativo." }
            }
        }),
    )
}

pub(super) fn personal_context_create() -> Value {
    tool(
        "personal_context_create",
        "Cria uma regra no contexto pessoal. SÓ use com pedido explícito do usuário. usage_scope aceita 'normal' (tarefas), 'chat' (chat/bots) ou 'both'.",
        json!({
            "type": "object",
            "required": ["instructions", "user_request"],
            "properties": {
                "instructions": { "type": "string" },
                "user_request": { "type": "string" },
                "scope": { "type": "string", "enum": ["workspace", "global"] },
                "name": { "type": "string" },
                "is_enabled": { "type": "boolean" },
                "position": { "type": "integer" },
                "usage_scope": {
                    "type": "string",
                    "enum": ["normal", "chat", "both"],
                    "description": "normal = tarefas/rotinas; chat = chat e bots; both = ambos."
                },
                "workspace_id": { "type": "string" }
            }
        }),
    )
}

pub(super) fn personal_context_update() -> Value {
    tool(
        "personal_context_update",
        "Atualiza uma regra do contexto pessoal. usage_scope aceita 'normal' | 'chat' | 'both'; outros valores retornam -32602.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" },
                "instructions": { "type": "string" },
                "scope": { "type": "string", "enum": ["workspace", "global"] },
                "name": { "type": "string" },
                "is_enabled": { "type": "boolean" },
                "position": { "type": "integer" },
                "usage_scope": { "type": "string", "enum": ["normal", "chat", "both"] }
            }
        }),
    )
}

pub(super) fn personal_context_set_scope() -> Value {
    tool(
        "personal_context_set_scope",
        "Altera apenas o escopo de uma regra pessoal: usage_scope='normal' | 'chat' | 'both'.",
        json!({
            "type": "object",
            "required": ["id", "usage_scope"],
            "properties": {
                "id": { "type": "string" },
                "usage_scope": { "type": "string", "enum": ["normal", "chat", "both"] }
            }
        }),
    )
}
