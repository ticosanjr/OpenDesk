use super::tool;
use serde_json::{Value, json};

pub(super) fn task_list() -> Value {
    tool(
        "task_list",
        "Lista tarefas do workspace, com filtros simples. Passe 'id' para abrir o detalhe de uma tarefa.",
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "status": { "type": "string", "enum": ["TODO", "IN_PROGRESS", "IN_REVIEW", "BLOCKED", "DONE", "CANCELLED"] },
                "agent": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 }
            }
        }),
    )
}

pub(super) fn task_create() -> Value {
    tool(
        "task_create",
        "Cria uma tarefa no Kanban em TODO. Use para trabalho durável, não para passos triviais. O título é opcional; sem ele, usamos o início da descrição.",
        json!({
            "type": "object",
            "required": ["description"],
            "properties": {
                "title": { "type": "string" },
                "description": { "type": "string" },
                "assigned_agent": { "type": "string" },
                "preferred_model": { "type": "string" },
                "execution_mode": { "type": "string" },
                "effort_level": { "type": "string" },
                "priority": { "type": "integer", "minimum": 1, "maximum": 4 },
                "tags": { "type": "array", "items": { "type": "string" } },
                "notify_email": { "type": "string" },
                "export_to_gdocs": { "type": "boolean" }
            }
        }),
    )
}

pub(super) fn task_update() -> Value {
    tool(
        "task_update",
        "Atualiza campos de uma tarefa. Mudanças de coluna usam task_move.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" },
                "title": { "type": "string" },
                "description": { "type": "string" },
                "assigned_agent": { "type": "string" },
                "preferred_model": { "type": "string" },
                "execution_mode": { "type": "string" },
                "effort_level": { "type": "string" },
                "priority": { "type": "integer", "minimum": 1, "maximum": 4 },
                "tags": { "type": "array", "items": { "type": "string" } },
                "notify_email": { "type": "string" },
                "export_to_gdocs": { "type": "boolean" },
                "clear_agent": { "type": "boolean" },
                "clear_model": { "type": "boolean" }
            }
        }),
    )
}

pub(super) fn task_move() -> Value {
    tool(
        "task_move",
        "Move a tarefa para outra coluna/status do Kanban, validando a transição no backend.",
        json!({
            "type": "object",
            "required": ["id", "status"],
            "properties": {
                "id": { "type": "string" },
                "status": { "type": "string", "enum": ["TODO", "IN_PROGRESS", "IN_REVIEW", "BLOCKED", "DONE", "CANCELLED"] },
                "reason": { "type": "string" }
            }
        }),
    )
}

pub(super) fn task_dispatch() -> Value {
    tool(
        "task_dispatch",
        "Enfileira uma tarefa para execução no OpenCode. Exige motor online e respeita budget e roteador.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" },
                "policy": { "type": "string" },
                "manual_agent": { "type": "string" },
                "manual_model": { "type": "string" },
                "manual_effort": { "type": "string" },
                "execution_mode": { "type": "string" }
            }
        }),
    )
}

pub(super) fn task_message_list() -> Value {
    tool(
        "task_message_list",
        "Lista as mensagens e comentários de uma tarefa.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string" } }
        }),
    )
}

pub(super) fn task_message_post() -> Value {
    tool(
        "task_message_post",
        "Adiciona um comentário/andamento à tarefa. continue_execution=true reenfileira a execução com esse contexto.",
        json!({
            "type": "object",
            "required": ["id", "content"],
            "properties": {
                "id": { "type": "string" },
                "content": { "type": "string" },
                "run_id": { "type": "string" },
                "continue_execution": { "type": "boolean" },
                "agent": { "type": "string" },
                "model": { "type": "string" }
            }
        }),
    )
}

pub(super) fn run_list() -> Value {
    tool(
        "run_list",
        "Lista execuções ou abre uma execução. Com include_timeline=true e id, devolve run + timeline.",
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "task_id": { "type": "string" },
                "status": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 },
                "include_timeline": { "type": "boolean" }
            }
        }),
    )
}
