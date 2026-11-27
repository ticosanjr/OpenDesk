use super::tool;
use serde_json::{Value, json};

pub(super) fn memory_save() -> Value {
    tool(
        "memory_save",
        "Salva uma nota durável na memória estruturada (namespace, tags, confiança e trust_level). Use usage_scope='normal' | 'chat' | 'both' para definir onde a nota pode ser injetada. Use para fatos reutilizáveis; preferências pessoais explícitas vão em personal_context_create.",
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
                "usage_scope": {
                    "type": "string",
                    "enum": ["normal", "chat", "both"],
                    "description": "normal = tarefas/rotinas; chat = chat e bots; both = ambos."
                },
                "agent": { "type": "string" }
            }
        }),
    )
}

pub(super) fn memory_create() -> Value {
    memory_save()
        .as_object_mut()
        .map(|object| {
            object.insert("name".into(), json!("memory_create"));
            Value::Object(object.clone())
        })
        .unwrap_or_else(memory_save)
}

pub(super) fn memory_list() -> Value {
    tool(
        "memory_list",
        "Lista notas da memória estruturada respeitando a ACL do agente. Filtre por usage_scope='normal' | 'chat' | 'both'.",
        json!({
            "type": "object",
            "properties": {
                "namespace": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 500 },
                "usage_scope": { "type": "string", "enum": ["normal", "chat", "both"] },
                "agent": { "type": "string" }
            }
        }),
    )
}

pub(super) fn memory_search() -> Value {
    tool(
        "memory_search",
        "Busca notas na memória estruturada respeitando a ACL do agente. Use usage_scope='normal' | 'chat' | 'both' e limite para restringir o resultado.",
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": { "type": "string" },
                "namespace": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100 },
                "usage_scope": { "type": "string", "enum": ["normal", "chat", "both"] },
                "agent": { "type": "string" }
            }
        }),
    )
}

pub(super) fn memory_update() -> Value {
    tool(
        "memory_update",
        "Atualiza uma nota existente. Aceita usage_scope='normal' | 'chat' | 'both'; qualquer outro valor é rejeitado com -32602.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" },
                "title": { "type": "string" },
                "body": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                "trust_level": { "type": "string", "enum": ["observed", "verified", "inferred"] },
                "expires_at": { "type": "string" },
                "clear_expiry": { "type": "boolean" },
                "usage_scope": { "type": "string", "enum": ["normal", "chat", "both"] }
            }
        }),
    )
}

pub(super) fn memory_set_scope() -> Value {
    tool(
        "memory_set_scope",
        "Altera apenas o escopo de injeção de uma nota: usage_scope='normal' | 'chat' | 'both'.",
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
