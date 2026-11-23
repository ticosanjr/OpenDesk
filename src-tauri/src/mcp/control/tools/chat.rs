use super::tool;
use serde_json::{Value, json};

pub(super) fn chat_send_file() -> Value {
    tool(
        "chat_send_file",
        "Envia um arquivo do workspace ao usuário no chat atual (Telegram/Signal). Use quando o usuário pedir um arquivo ou quando precisar entregar um resultado. O thread_id está no contexto do canal.",
        json!({
            "type": "object",
            "required": ["thread_id", "path"],
            "properties": {
                "thread_id": { "type": "string", "description": "ID da conversa atual, informado no contexto do canal." },
                "path": { "type": "string", "description": "Caminho do arquivo, relativo ao workspace ou absoluto dentro dele." },
                "caption": { "type": "string", "description": "Legenda opcional enviada junto do arquivo." }
            }
        }),
    )
}
