use serde_json::{Value, json};

pub(super) fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

pub(super) fn empty() -> Value {
    json!({ "type": "object", "properties": {} })
}

pub(super) fn tools() -> Vec<Value> {
    vec![
        tool(
            "opendesk_status",
            "Resumo operacional: workspace, motor OpenCode, fila, budget e itens que precisam de atenção. Não resolve HITL, permissões ou propostas.",
            empty(),
        ),
        tool(
            "task_list",
            "Lista tarefas do workspace, com filtros simples. Passe 'id' para abrir o detalhe de uma tarefa.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "status": { "type": "string", "enum": ["TODO", "IN_PROGRESS", "IN_REVIEW", "BLOCKED", "DONE", "CANCELLED"] },
                    "agent": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1 }
                }
            }),
        ),
        tool(
            "task_create",
            "Cria uma tarefa no Kanban em TODO. Use para trabalho durável, não para passos triviais. O título é opcional; sem ele, usamos o início da descrição.",
            json!({
                "type": "object",
                "required": ["description"],
                "properties": {
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "assigned_agent": { "type": "string" },
                    "preferred_model": { "type": "string" },
                    "execution_mode": { "type": "string" },
                    "effort_level": { "type": "string" },
                    "priority": { "type": "integer", "minimum": 1, "maximum": 4 },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "notify_email": { "type": "string" },
                    "export_to_gdocs": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "task_update",
            "Atualiza campos de uma tarefa. Mudanças de coluna usam task_move.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "assigned_agent": { "type": "string" },
                    "preferred_model": { "type": "string" },
                    "execution_mode": { "type": "string" },
                    "effort_level": { "type": "string" },
                    "priority": { "type": "integer", "minimum": 1, "maximum": 4 },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "notify_email": { "type": "string" },
                    "export_to_gdocs": { "type": "boolean" },
                    "clear_agent": { "type": "boolean" },
                    "clear_model": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "task_move",
            "Move a tarefa para outra coluna/status do Kanban, validando a transição no backend.",
            json!({
                "type": "object",
                "required": ["id", "status"],
                "properties": {
                    "id": { "type": "string" },
                    "status": { "type": "string", "enum": ["TODO", "IN_PROGRESS", "IN_REVIEW", "BLOCKED", "DONE", "CANCELLED"] },
                    "reason": { "type": "string" }
                }
            }),
        ),
        tool(
            "task_dispatch",
            "Enfileira uma tarefa para execução no OpenCode. Exige motor online e respeita budget e roteador.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "policy": { "type": "string" },
                    "manual_agent": { "type": "string" },
                    "manual_model": { "type": "string" },
                    "manual_effort": { "type": "string" },
                    "execution_mode": { "type": "string" }
                }
            }),
        ),
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
        ),
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
        ),
        tool(
            "task_message_list",
            "Lista as mensagens e comentários de uma tarefa.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        ),
        tool(
            "task_message_post",
            "Adiciona um comentário/andamento à tarefa. continue_execution=true reenfileira a execução com esse contexto.",
            json!({
                "type": "object",
                "required": ["id", "content"],
                "properties": {
                    "id": { "type": "string" },
                    "content": { "type": "string" },
                    "run_id": { "type": "string" },
                    "continue_execution": { "type": "boolean" },
                    "agent": { "type": "string" },
                    "model": { "type": "string" }
                }
            }),
        ),
        tool(
            "run_list",
            "Lista execuções ou abre uma execução. Com include_timeline=true e id, devolve run + timeline.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "task_id": { "type": "string" },
                    "status": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1 },
                    "include_timeline": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "routine_list",
            "Lista rotinas do workspace (ativas e pausadas) ou abre uma rotina por id.",
            json!({
                "type": "object",
                "properties": { "id": { "type": "string" } }
            }),
        ),
        tool(
            "routine_create",
            "Cria uma rotina SEMPRE desligada. Para CRON, informe recurrence (preferido) ou cron_expression. Para os demais gatilhos, informe trigger_config. Ativar é uma ação humana explícita.",
            json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "recurrence": {
                        "type": "object",
                        "properties": {
                            "frequency": { "type": "string", "enum": ["once", "hourly", "daily", "weekly", "weekdays", "monthly", "custom"] },
                            "interval": { "type": "integer", "minimum": 1 },
                            "time": { "type": "string" },
                            "weekdays": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 7 } },
                            "day_of_month": { "type": "integer", "minimum": 1, "maximum": 31 },
                            "start_date": { "type": "string" },
                            "end_date": { "type": "string" },
                            "cron_expression": { "type": "string" }
                        }
                    },
                    "cron_expression": { "type": "string" },
                    "timezone": { "type": "string" },
                    "trigger_type": {
                        "type": "string",
                        "enum": ["CRON", "FOLDER_WATCH", "RSYNC_FOLDER", "IMAP_IDLE", "SIGNAL_MSG", "WEBHOOK", "SQL_POLL"]
                    },
                    "trigger_config": {
                        "type": "object",
                        "description": "Configuração do gatilho não-CRON, por exemplo {\"path\":\"input\"} ou {\"path\":\"/origem\",\"direction\":\"pull\"}."
                    },
                    "trigger_config_json": { "type": "string", "description": "Alternativa textual a trigger_config." },
                    "target_kind": { "type": "string", "enum": ["PROMPT", "RECIPE"] },
                    "recipe_id": { "type": "string" },
                    "command_or_prompt": { "type": "string" },
                    "agent_name": { "type": "string" },
                    "router_policy": { "type": "string" },
                    "output_folder_template": { "type": "string" },
                    "notify_email_recipients": { "type": "string" },
                    "notify_signal": { "type": "boolean" },
                    "notify_signal_recipients": { "type": "string" },
                    "notify_telegram": { "type": "boolean" },
                    "notify_telegram_recipients": { "type": "string" },
                    "notify_only_on_change": { "type": "boolean" },
                    "misfire_policy": { "type": "string" },
                    "safe_readonly_mode": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "routine_update",
            "Edita uma rotina existente. Não altera o estado ligado/desligado; use routine_set_active.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "recurrence": { "type": "object" },
                    "cron_expression": { "type": "string" },
                    "timezone": { "type": "string" },
                    "trigger_type": {
                        "type": "string",
                        "enum": ["CRON", "FOLDER_WATCH", "RSYNC_FOLDER", "IMAP_IDLE", "SIGNAL_MSG", "WEBHOOK", "SQL_POLL"]
                    },
                    "trigger_config": { "type": "object" },
                    "trigger_config_json": { "type": "string" },
                    "recipe_id": { "type": "string" },
                    "command_or_prompt": { "type": "string" },
                    "agent_name": { "type": "string" },
                    "router_policy": { "type": "string" },
                    "output_folder_template": { "type": "string" },
                    "notify_email_recipients": { "type": "string" },
                    "notify_signal": { "type": "boolean" },
                    "notify_signal_recipients": { "type": "string" },
                    "notify_telegram": { "type": "boolean" },
                    "notify_telegram_recipients": { "type": "string" },
                    "notify_only_on_change": { "type": "boolean" },
                    "misfire_policy": { "type": "string" },
                    "safe_readonly_mode": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "routine_set_active",
            "Liga ou desliga uma rotina. Só use quando o usuário pedir explicitamente; nunca ative por conta própria.",
            json!({
                "type": "object",
                "required": ["id", "active"],
                "properties": {
                    "id": { "type": "string" },
                    "active": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "routine_run",
            "Executa uma rotina agora. Use dry_run=true para validar sem executar.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "dry_run": { "type": "boolean" }
                }
            }),
        ),
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
        ),
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
        ),
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
        ),
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
        ),
        tool(
            "attention_list",
            "Lista HITL, permissões, propostas e falhas que precisam de decisão humana. Apenas leitura: nunca aprove por conta própria.",
            empty(),
        ),
        tool(
            "queue_control",
            "Consulta ou controla a fila: status, pause, resume, cancel ou retry (cancel/retry exigem job_id).",
            json!({
                "type": "object",
                "required": ["action"],
                "properties": {
                    "action": { "type": "string", "enum": ["status", "pause", "resume", "cancel", "retry"] },
                    "job_id": { "type": "string" },
                    "reason": { "type": "string" }
                }
            }),
        ),
        tool(
            "artifact_list",
            "Lista artefatos do workspace ou abre um artefato por id, com filtros por run, tarefa, agente e ação.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "run_id": { "type": "string" },
                    "task_id": { "type": "string" },
                    "agent": { "type": "string" },
                    "action": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1 }
                }
            }),
        ),
        tool(
            "artifact_read",
            "Lê o conteúdo textual de um artefato (até max_bytes, padrão 64 KB). Use artifact_list para descobrir o id e o caminho.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "max_bytes": { "type": "integer", "minimum": 1024 }
                }
            }),
        ),
        tool(
            "artifact_pin",
            "Fixa ou desafixa um artefato como entregável. Passe pinned=true/false.",
            json!({
                "type": "object",
                "required": ["id", "pinned"],
                "properties": {
                    "id": { "type": "string" },
                    "pinned": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "artifact_rollback",
            "Reverte a alteração registrada para o artefato. Use apenas com pedido explícito e após conferir o run.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_expected_control_tools() {
        let names = tools()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>();
        for expected in [
            "opendesk_status",
            "task_list",
            "task_create",
            "task_update",
            "task_move",
            "task_dispatch",
            "ask_user",
            "chat_send_file",
            "task_message_list",
            "task_message_post",
            "run_list",
            "routine_list",
            "routine_create",
            "routine_update",
            "routine_set_active",
            "routine_run",
            "recipe_list",
            "recipe_save",
            "recipe_set_enabled",
            "recipe_run",
            "attention_list",
            "queue_control",
            "artifact_list",
            "artifact_read",
            "artifact_pin",
            "artifact_rollback",
        ] {
            assert!(names.iter().any(|name| name == expected), "{expected}");
        }
        assert_eq!(names.len(), 26);
    }
}
