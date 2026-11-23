use super::{empty, tool};
use serde_json::Value;

pub(super) fn opendesk_status() -> Value {
    tool(
        "opendesk_status",
        "Resumo operacional: workspace, motor OpenCode, fila, budget e itens que precisam de atenção. Não resolve HITL, permissões ou propostas.",
        empty(),
    )
}
