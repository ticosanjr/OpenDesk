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
        ),
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
            "learning_save",
            "Salva um aprendizado do agente no workspace. Use para preferências, lições e pedidos de memorizar. remember=true grava também no MEMORY.md. Nunca use esta ferramenta para contexto pessoal; para isso existe personal_context_save, somente com pedido explícito do usuário.",
            json!({
                "type": "object",
                "required": ["title", "detail"],
                "properties": {
                    "title": { "type": "string", "description": "Título curto do aprendizado." },
                    "detail": { "type": "string", "description": "O que deve ser lembrado, com contexto suficiente." },
                    "category": { "type": "string", "description": "Categoria livre; padrão manual." },
                    "severity": { "type": "string", "enum": ["info", "warning", "critical"] },
                    "remember": { "type": "boolean", "description": "true quando o usuário pedir para memorizar; grava no MEMORY.md." },
                    "agent_name": { "type": "string" },
                    "workspace_id": { "type": "string", "description": "Workspace alvo; vazio usa o ativo." }
                }
            }),
        ),
        tool(
            "personal_context_save",
            "Salva uma regra no contexto pessoal. SÓ use quando o usuário pedir explicitamente para salvar no contexto pessoal; copie o pedido literal em user_request. Para memorizações genéricas use learning_save.",
            json!({
                "type": "object",
                "required": ["instructions", "user_request"],
                "properties": {
                    "instructions": { "type": "string", "description": "A regra que deve entrar nos prompts." },
                    "user_request": { "type": "string", "description": "Citação literal do pedido explícito do usuário para salvar no contexto pessoal." },
                    "scope": { "type": "string", "enum": ["workspace", "global"] },
                    "name": { "type": "string", "description": "Rótulo interno opcional." },
                    "workspace_id": { "type": "string", "description": "Workspace alvo; vazio usa o ativo." }
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
        ),
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
        ),
        tool(
            "handoff_create",
            "Delega trabalho para outro agente com objetivo, critérios de aceite e sub-orçamento. O handoff nasce PENDING e precisa de aceite humano.",
            json!({
                "type": "object",
                "required": ["source_agent", "target_agent", "objective"],
                "properties": {
                    "source_agent": { "type": "string" },
                    "target_agent": { "type": "string" },
                    "objective": { "type": "string" },
                    "task_id": { "type": "string" },
                    "parent_handoff_id": { "type": "string" },
                    "input_artifacts": { "type": "array", "items": { "type": "string" } },
                    "output_schema": { "type": "object" },
                    "acceptance": { "type": "array", "items": { "type": "string" } },
                    "sub_budget_usd": { "type": "number", "minimum": 0 }
                }
            }),
        ),
        tool(
            "handoff_list",
            "Lista handoffs do workspace. Use para acompanhar delegações e o que aguarda aceite.",
            json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string", "enum": ["PENDING", "ACCEPTED", "REJECTED", "RUNNING", "DONE", "FAILED", "CANCELLED"] },
                    "limit": { "type": "integer", "minimum": 1 }
                }
            }),
        ),
        tool(
            "handoff_respond",
            "Aceita ou rejeita um handoff pendente. Aceitar cria a tarefa do agente destino. Pedidos acima do orçamento exigem force=true e decisão humana.",
            json!({
                "type": "object",
                "required": ["id", "decision"],
                "properties": {
                    "id": { "type": "string" },
                    "decision": { "type": "string", "enum": ["accept", "reject"] },
                    "note": { "type": "string" },
                    "reason": { "type": "string" },
                    "force": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "handoff_finish",
            "Fecha um handoff em execução como DONE ou FAILED, com resultado estruturado.",
            json!({
                "type": "object",
                "required": ["id", "status"],
                "properties": {
                    "id": { "type": "string" },
                    "status": { "type": "string", "enum": ["DONE", "FAILED"] },
                    "result": { "type": "object" }
                }
            }),
        ),
        tool(
            "mcp_discover_tools",
            "Descobre ferramentas do opendesk-control sob demanda. Use quando a fase atual expõe poucas tools; retorna nome, descrição e schema das ferramentas que correspondem à busca (ou todas da fase).",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "phase": { "type": "string", "enum": ["PLAN", "BUILD", "AUDIT", "CHAT", "OPS", "ALL"] }
                }
            }),
        ),
        tool(
            "recipe_resume",
            "Repete um run de receita que falhou/rejeitou, preservando os nós já concluídos.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        ),
        tool(
            "recipe_cancel",
            "Solicita o cancelamento de um run de receita ativo.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        ),
        tool(
            "voice_speak",
            "Gera áudio TTS a partir de texto usando o motor de voz configurado e devolve o caminho do arquivo.",
            json!({
                "type": "object",
                "required": ["text"],
                "properties": { "text": { "type": "string" } }
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
        tool(
            "recipe_versions",
            "Lista as versões salvas de uma receita, da mais recente para a mais antiga.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        ),
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
        ),
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
        ),
        tool(
            "worktree_list",
            "Lista os worktrees/overlays isolados do workspace.",
            empty(),
        ),
        tool(
            "worktree_prepare",
            "Cria um worktree Git (ou overlay sem Git) para isolar um run. Retorna id, caminho e modo.",
            json!({
                "type": "object",
                "properties": {
                    "run_id": { "type": "string" }
                }
            }),
        ),
        tool(
            "worktree_diff",
            "Mostra o diff/lista de mudanças do worktree sem tocar na árvore principal.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        ),
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
        ),
        tool(
            "worktree_cleanup",
            "Remove o worktree e o branch temporário. Use depois do merge ou quando o run for descartado.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        ),
    ]
}

/// Phase-aware visibility. `ALL` keeps the historical behavior (everything).
pub(super) fn filtered_tools(phase: &str, _role: Option<&str>) -> Vec<Value> {
    let phase = phase.trim().to_uppercase();
    tools()
        .into_iter()
        .filter(|tool| {
            let name = tool.get("name").and_then(Value::as_str).unwrap_or("");
            visible_in(name, &phase)
        })
        .collect()
}

/// Workspace-specific tools: every enabled recipe and every enabled agent
/// becomes callable by name. Discovery is best-effort; failures simply leave
/// the static tool set untouched.
pub(super) fn dynamic_tools(recipes: &[Value], agents: &[Value]) -> Vec<Value> {
    let mut tools = Vec::new();
    for recipe in recipes.iter().take(50) {
        let Some(id) = recipe.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !recipe
            .get("is_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            continue;
        }
        let name = tool_name("recipe", id);
        let label = recipe.get("name").and_then(Value::as_str).unwrap_or(id);
        tools.push(tool(
            &name,
            &format!("Executa a receita '{label}' ({id}) com contexto opcional. Retorna o run durável criado."),
            json!({
                "type": "object",
                "properties": {
                    "trigger": {
                        "type": "object",
                        "description": "Contexto de trigger; fica disponível como {{trigger.*}} na receita."
                    },
                    "dry_run": { "type": "boolean" }
                }
            }),
        ));
    }
    for agent in agents.iter().take(50) {
        let Some(id) = agent.get("id").and_then(Value::as_str) else {
            continue;
        };
        if agent
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let name = tool_name("agent", id);
        let label = agent.get("name").and_then(Value::as_str).unwrap_or(id);
        tools.push(tool(
            &name,
            &format!("Cria uma tarefa para o agente '{label}' ({id}) e tenta iniciá-la. Retorna a tarefa criada."),
            json!({
                "type": "object",
                "required": ["prompt"],
                "properties": {
                    "prompt": { "type": "string" },
                    "title": { "type": "string" },
                    "model": { "type": "string" },
                    "mode": { "type": "string" }
                }
            }),
        ));
    }
    tools
}

pub(super) fn tool_name(prefix: &str, id: &str) -> String {
    let sanitized: String = id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    format!("{prefix}_{sanitized}")
}

fn visible_in(name: &str, phase: &str) -> bool {
    if matches!(phase, "" | "ALL") {
        return true;
    }
    if matches!(
        name,
        "opendesk_status"
            | "task_list"
            | "ask_user"
            | "elicit"
            | "sampling"
            | "memory_search"
            | "mcp_discover_tools"
    ) {
        return true;
    }
    let category = if matches!(
        name,
        "queue_control"
            | "routine_list"
            | "routine_create"
            | "routine_update"
            | "routine_set_active"
            | "routine_run"
            | "recipe_list"
            | "recipe_save"
            | "recipe_set_enabled"
            | "recipe_run"
            | "recipe_resume"
            | "recipe_cancel"
            | "recipe_versions"
            | "recipe_diff"
            | "recipe_rollback"
            | "attention_list"
            | "worktree_cleanup"
    ) {
        "OPS"
    } else if matches!(name, "worktree_prepare") {
        "BUILD"
    } else if matches!(name, "worktree_merge") {
        "AUDIT"
    } else if matches!(
        name,
        "task_create"
            | "task_update"
            | "task_move"
            | "task_dispatch"
            | "learning_save"
            | "personal_context_save"
            | "memory_save"
            | "chat_send_file"
            | "task_message_post"
            | "voice_speak"
            | "handoff_create"
            | "handoff_respond"
            | "handoff_finish"
    ) {
        "BUILD"
    } else if matches!(name, "artifact_pin" | "artifact_rollback") {
        "AUDIT"
    } else {
        "READ"
    };
    match phase {
        "PLAN" => matches!(category, "READ" | "BUILD"),
        "BUILD" => matches!(category, "READ" | "BUILD"),
        "AUDIT" => matches!(category, "READ"),
        "CHAT" => matches!(category, "READ" | "BUILD"),
        "OPS" => true,
        _ => true,
    }
}

pub(super) fn discover(query: &str, phase: Option<&str>) -> Vec<Value> {
    let query = query.trim().to_lowercase();
    let phase = phase.unwrap_or("ALL");
    tools()
        .into_iter()
        .filter(|tool| {
            let name = tool.get("name").and_then(Value::as_str).unwrap_or("");
            let description = tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            let matches_query = query.is_empty()
                || name.to_lowercase().contains(&query)
                || description.to_lowercase().contains(&query);
            matches_query && visible_in(name, &phase.to_uppercase())
        })
        .collect()
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
            "learning_save",
            "personal_context_save",
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
            "mcp_discover_tools",
            "recipe_resume",
            "recipe_cancel",
            "voice_speak",
            "handoff_create",
            "handoff_list",
            "memory_save",
            "memory_search",
            "handoff_respond",
            "handoff_finish",
        ] {
            assert!(names.iter().any(|name| name == expected), "{expected}");
        }
        for expected in [
            "recipe_versions",
            "recipe_diff",
            "recipe_rollback",
            "worktree_list",
            "worktree_prepare",
            "worktree_diff",
            "worktree_merge",
            "worktree_cleanup",
        ] {
            assert!(names.iter().any(|name| name == expected), "{expected}");
        }
        assert_eq!(names.len(), 48);
    }

    #[test]
    fn builds_dynamic_recipe_and_agent_tools() {
        let recipes = vec![
            json!({ "id": "daily-brief", "name": "Daily", "is_enabled": true }),
            json!({ "id": "off", "name": "Off", "is_enabled": false }),
        ];
        let agents = vec![
            json!({ "id": "build.agent", "name": "Build", "disabled": false }),
            json!({ "id": "off", "name": "Off", "disabled": true }),
        ];
        let tools = dynamic_tools(&recipes, &agents);
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(names.contains(&"recipe_daily-brief"));
        assert!(!names.contains(&"recipe_off"));
        assert!(names.contains(&"agent_build_agent"));
        assert!(!names.contains(&"agent_off"));
        assert_eq!(tool_name("recipe", "a/b c"), "recipe_a_b_c");
    }

    #[test]
    fn filters_tools_by_phase_and_discovers() {
        let plan = filtered_tools("PLAN", None);
        let names = plan
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(names.contains(&"task_create"));
        assert!(names.contains(&"memory_save"));
        assert!(!names.contains(&"queue_control"));
        assert!(names.contains(&"mcp_discover_tools"));

        let audit = filtered_tools("AUDIT", None);
        let names = audit
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(!names.contains(&"task_create"));
        assert!(names.contains(&"artifact_read"));

        let found = discover("recipe", Some("OPS"));
        assert!(found.iter().any(|tool| tool["name"] == "recipe_run"));
        assert!(
            !discover("recipe", Some("PLAN"))
                .iter()
                .any(|tool| tool["name"] == "recipe_run")
        );
    }
}
