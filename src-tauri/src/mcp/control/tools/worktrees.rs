use super::{empty, tool};
use serde_json::{Value, json};

pub(super) fn worktree_list() -> Value {
    tool(
        "worktree_list",
        "Lista os worktrees/overlays isolados do workspace.",
        empty(),
    )
}

pub(super) fn worktree_prepare() -> Value {
    tool(
        "worktree_prepare",
        "Cria um worktree Git (ou overlay sem Git) para isolar um run. Retorna id, caminho e modo.",
        json!({
            "type": "object",
            "properties": {
                "run_id": { "type": "string" }
            }
        }),
    )
}

pub(super) fn worktree_diff() -> Value {
    tool(
        "worktree_diff",
        "Mostra o diff/lista de mudanças do worktree sem tocar na árvore principal.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string" } }
        }),
    )
}

pub(super) fn worktree_merge() -> Value {
    tool(
        "worktree_merge",
        "Mescla o worktree na árvore principal. Exige approved=true; conflitos abortam sem alterar o workspace.",
        json!({
            "type": "object",
            "required": ["id", "approved"],
            "properties": {
                "id": { "type": "string" },
                "approved": { "type": "boolean" }
            }
        }),
    )
}

pub(super) fn worktree_cleanup() -> Value {
    tool(
        "worktree_cleanup",
        "Remove o worktree e o branch temporário. Use depois do merge ou quando o run for descartado.",
        json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string" } }
        }),
    )
}
