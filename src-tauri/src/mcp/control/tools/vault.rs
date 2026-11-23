use super::{empty, tool};
use serde_json::{Value, json};

pub(super) fn vault_list_refs() -> Value {
    tool(
        "vault_list_refs",
        "Lista as credenciais do Vault apenas com metadados (id, tipo, rótulo, domínios permitidos, expiração). Nunca retorna o segredo; use o id para declarar vault_refs ou pedir preenchimento.",
        empty(),
    )
}

pub(super) fn vault_request_fill() -> Value {
    tool(
        "vault_request_fill",
        "Abre um HITL para o usuário preencher a credencial no site/navegador. Nunca retorna o segredo. Exige task_id para criar a interação; use quando a receita ou o Browse precisar de login/2FA.",
        json!({
            "type": "object",
            "required": ["entry_id", "task_id"],
            "properties": {
                "entry_id": { "type": "string", "description": "Id retornado por vault_list_refs." },
                "task_id": { "type": "string", "description": "Tarefa que receberá a interação HITL." },
                "reason": { "type": "string", "description": "Por que a credencial é necessária." },
                "timeout_minutes": { "type": "integer", "minimum": 1, "maximum": 120 }
            }
        }),
    )
}
