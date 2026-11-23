use super::tool;
use serde_json::{Value, json};

pub(super) fn handoff_create() -> Value {
    tool(
        "handoff_create",
        "Delega trabalho para outro agente com objetivo, critérios de aceite e sub-orçamento. O handoff nasce PENDING e precisa de aceite humano.",
        json!({
            "type": "object",
            "required": ["source_agent", "target_agent", "objective"],
            "properties": {
                "source_agent": { "type": "string" },
                "target_agent": { "type": "string" },
                "objective": { "type": "string" },
                "task_id": { "type": "string" },
                "parent_handoff_id": { "type": "string" },
                "input_artifacts": { "type": "array", "items": { "type": "string" } },
                "output_schema": { "type": "object" },
                "acceptance": { "type": "array", "items": { "type": "string" } },
                "sub_budget_usd": { "type": "number", "minimum": 0 }
            }
        }),
    )
}

pub(super) fn handoff_list() -> Value {
    tool(
        "handoff_list",
        "Lista handoffs do workspace. Use para acompanhar delegações e o que aguarda aceite.",
        json!({
            "type": "object",
            "properties": {
                "status": { "type": "string", "enum": ["PENDING", "ACCEPTED", "REJECTED", "RUNNING", "DONE", "FAILED", "CANCELLED"] },
                "limit": { "type": "integer", "minimum": 1 }
            }
        }),
    )
}

pub(super) fn handoff_respond() -> Value {
    tool(
        "handoff_respond",
        "Aceita ou rejeita um handoff pendente. Aceitar cria a tarefa do agente destino. Pedidos acima do orçamento exigem force=true e decisão humana.",
        json!({
            "type": "object",
            "required": ["id", "decision"],
            "properties": {
                "id": { "type": "string" },
                "decision": { "type": "string", "enum": ["accept", "reject"] },
                "note": { "type": "string" },
                "reason": { "type": "string" },
                "force": { "type": "boolean" }
            }
        }),
    )
}

pub(super) fn handoff_finish() -> Value {
    tool(
        "handoff_finish",
        "Fecha um handoff em execução como DONE ou FAILED, com resultado estruturado.",
        json!({
            "type": "object",
            "required": ["id", "status"],
            "properties": {
                "id": { "type": "string" },
                "status": { "type": "string", "enum": ["DONE", "FAILED"] },
                "result": { "type": "object" }
            }
        }),
    )
}
