use super::tool;
use serde_json::{Value, json};

pub(super) fn ask_user() -> Value {
    tool(
        "ask_user",
        "Faz uma pergunta de esclarecimento ao humano e pausa a tarefa até a resposta. Use quando faltar informação essencial e não for seguro assumir. Escolha o kind: APPROVAL para aprovar/negar uma ação (com payload opcional), SINGLE_CHOICE ou MULTI_CHOICE quando houver options, TEXT para resposta livre. Informe o task_id recebido no contexto OpenDesk.",
        json!({
            "type": "object",
            "required": ["task_id", "question"],
            "properties": {
                "task_id": { "type": "string" },
                "question": { "type": "string" },
                "kind": {
                    "type": "string",
                    "enum": ["APPROVAL", "SINGLE_CHOICE", "MULTI_CHOICE", "TEXT"],
                    "description": "APPROVAL exige aprovar/negar; escolhas exigem options; TEXT aceita resposta livre."
                },
                "context": { "type": "string" },
                "options": { "type": "array", "items": { "type": "string" } },
                "multiple": { "type": "boolean", "description": "Compatibilidade: MULTI_CHOICE já implica múltipla seleção." },
                "allow_custom": { "type": "boolean", "description": "Permite payload/texto livre junto da decisão." },
                "timeout_minutes": { "type": "integer", "minimum": 1, "maximum": 120 }
            }
        }),
    )
}

pub(super) fn elicit() -> Value {
    tool(
        "elicit",
        "Pede um dado ao cliente MCP via elicitation/create. Com sessão SSE, a pergunta aparece no cliente; sem sessão, use task_id para cair no HITL 2.0 (ask_user).",
        json!({
            "type": "object",
            "required": ["message"],
            "properties": {
                "message": { "type": "string" },
                "schema": {
                    "type": "object",
                    "description": "JSON Schema dos campos pedidos; padrão: objeto livre."
                },
                "task_id": { "type": "string", "description": "Fallback HITL quando não há sessão SSE." },
                "timeout_seconds": { "type": "integer", "minimum": 5, "maximum": 300 }
            }
        }),
    )
}

pub(super) fn sampling() -> Value {
    tool(
        "sampling",
        "Pede uma amostra de texto ao cliente MCP (sampling/createMessage). Sem cliente compatível, usa DSH + budget para gerar a resposta.",
        json!({
            "type": "object",
            "required": ["prompt"],
            "properties": {
                "prompt": { "type": "string" },
                "model": { "type": "string", "description": "Opcional: força um modelo provider/modelo." },
                "max_tokens": { "type": "integer", "minimum": 1 },
                "timeout_seconds": { "type": "integer", "minimum": 5, "maximum": 300 }
            }
        }),
    )
}
