use super::tool;
use serde_json::{Value, json};

pub(super) fn voice_speak() -> Value {
    tool(
        "voice_speak",
        "Gera áudio TTS a partir de texto usando o motor de voz configurado e devolve o caminho do arquivo.",
        json!({
            "type": "object",
            "required": ["text"],
            "properties": { "text": { "type": "string" } }
        }),
    )
}
