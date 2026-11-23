use super::tool;
use serde_json::{Value, json};

pub(super) fn memory_save() -> Value {
    tool(
        "memory_save",
        "Salva uma nota durável na memória estruturada (namespace, tags, confiança e trust_level). Use para fatos reutilizáveis; preferências pessoais explícitas vão em personal_context_save.",
        json!({
            "type": "object",
            "required": ["title", "body"],
            "properties": {
                "title": { "type": "string" },
                "body": { "type": "string" },
                "namespace": { "type": "string" },
                "kind": { "type": "string", "enum": ["semantic", "episodic", "procedural", "preference"] },
                "tags": { "type": "array", "items": { "type": "string" } },
                "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                "trust_level": { "type": "string", "enum": ["observed", "verified", "inferred"] },
                "expires_at": { "type": "string" },
                "source": { "type": "string" },
                "agent": { "type": "string" }
            }
        }),
    )
}

pub(super) fn memory_search() -> Value {
    tool(
        "memory_search",
        "Busca notas na memória estruturada respeitando a ACL do agente. Use antes de assumir preferências ou fatos do workspace.",
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": { "type": "string" },
                "namespace": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100 },
                "agent": { "type": "string" }
            }
        }),
    )
}
