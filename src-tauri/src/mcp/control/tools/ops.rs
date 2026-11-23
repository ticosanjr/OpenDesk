use super::{empty, tool};
use serde_json::{Value, json};

pub(super) fn attention_list() -> Value {
    tool(
        "attention_list",
        "Lista HITL, permissões, propostas e falhas que precisam de decisão humana. Apenas leitura: nunca aprove por conta própria.",
        empty(),
    )
}

pub(super) fn queue_control() -> Value {
    tool(
        "queue_control",
        "Consulta ou controla a fila: status, pause, resume, cancel ou retry (cancel/retry exigem job_id).",
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": { "type": "string", "enum": ["status", "pause", "resume", "cancel", "retry"] },
                "job_id": { "type": "string" },
                "reason": { "type": "string" }
            }
        }),
    )
}

pub(super) fn artifact_list() -> Value {
    tool(
        "artifact_list",
        "Lista artefatos do workspace ou abre um artefato por id, com filtros por run, tarefa, agente e ação.",
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "run_id": { "type": "string" },
                "task_id": { "type": "string" },
                "agent": { "type": "string" },
                "action": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 }
            }
        }),
    )
}

pub(super) fn artifact_read() -> Value {
    tool(
        "artifact_read",
        "Lê o conteúdo textual de um artefato (até max_bytes, padrão 64 KB). Use artifact_list para descobrir o id e o caminho.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" },
                "max_bytes": { "type": "integer", "minimum": 1024 }
            }
        }),
    )
}

pub(super) fn artifact_pin() -> Value {
    tool(
        "artifact_pin",
        "Fixa ou desafixa um artefato como entregável. Passe pinned=true/false.",
        json!({
            "type": "object",
            "required": ["id", "pinned"],
            "properties": {
                "id": { "type": "string" },
                "pinned": { "type": "boolean" }
            }
        }),
    )
}

pub(super) fn artifact_rollback() -> Value {
    tool(
        "artifact_rollback",
        "Reverte a alteração registrada para o artefato. Use apenas com pedido explícito e após conferir o run.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string" } }
        }),
    )
}

pub(super) fn mcp_discover_tools() -> Value {
    tool(
        "mcp_discover_tools",
        "Descobre ferramentas do opendesk-control sob demanda. Use quando a fase atual expõe poucas tools; retorna nome, descrição e schema das ferramentas que correspondem à busca (ou todas da fase).",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "phase": { "type": "string", "enum": ["PLAN", "BUILD", "AUDIT", "CHAT", "OPS", "ALL"] }
            }
        }),
    )
}
