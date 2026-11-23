use super::tool;
use serde_json::{Value, json};

pub(super) fn recipe_list() -> Value {
    tool(
        "recipe_list",
        "Lista receitas ou abre uma receita por id. Com id, include_yaml=false omite o YAML.",
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "include_yaml": { "type": "boolean" }
            }
        }),
    )
}

pub(super) fn recipe_save() -> Value {
    tool(
        "recipe_save",
        "Cria ou atualiza uma receita YAML. Receitas novas são gravadas desligadas; edições preservam o estado.",
        json!({
            "type": "object",
            "required": ["id", "yaml"],
            "properties": {
                "id": { "type": "string" },
                "yaml": { "type": "string" }
            }
        }),
    )
}

pub(super) fn recipe_set_enabled() -> Value {
    tool(
        "recipe_set_enabled",
        "Liga ou desliga uma receita. Só use quando o usuário pedir explicitamente; nunca ligue por conta própria.",
        json!({
            "type": "object",
            "required": ["id", "enabled"],
            "properties": {
                "id": { "type": "string" },
                "enabled": { "type": "boolean" }
            }
        }),
    )
}

pub(super) fn recipe_run() -> Value {
    tool(
        "recipe_run",
        "Executa uma receita agora. Use dry_run=true para validar sem executar.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" },
                "trigger_context": { "type": "object" },
                "dry_run": { "type": "boolean" }
            }
        }),
    )
}

pub(super) fn recipe_resume() -> Value {
    tool(
        "recipe_resume",
        "Repete um run de receita que falhou/rejeitou, preservando os nós já concluídos.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string" } }
        }),
    )
}

pub(super) fn recipe_cancel() -> Value {
    tool(
        "recipe_cancel",
        "Solicita o cancelamento de um run de receita ativo.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string" } }
        }),
    )
}

pub(super) fn recipe_versions() -> Value {
    tool(
        "recipe_versions",
        "Lista as versões salvas de uma receita, da mais recente para a mais antiga.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string" } }
        }),
    )
}

pub(super) fn recipe_diff() -> Value {
    tool(
        "recipe_diff",
        "Mostra o diff unificado de uma versão da receita contra o arquivo atual ou outra versão.",
        json!({
            "type": "object",
            "required": ["id", "version_id"],
            "properties": {
                "id": { "type": "string" },
                "version_id": { "type": "string" },
                "against": { "type": "string" }
            }
        }),
    )
}

pub(super) fn recipe_rollback() -> Value {
    tool(
        "recipe_rollback",
        "Reescreve a receita com o conteúdo de uma versão anterior, revalidando o YAML e registrando uma nova versão.",
        json!({
            "type": "object",
            "required": ["id", "version_id"],
            "properties": {
                "id": { "type": "string" },
                "version_id": { "type": "string" }
            }
        }),
    )
}
