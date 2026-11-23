use super::tool;
use serde_json::{Value, json};

pub(super) fn routine_list() -> Value {
    tool(
        "routine_list",
        "Lista rotinas do workspace (ativas e pausadas) ou abre uma rotina por id.",
        json!({
            "type": "object",
            "properties": { "id": { "type": "string" } }
        }),
    )
}

pub(super) fn routine_create() -> Value {
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
    )
}

pub(super) fn routine_update() -> Value {
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
    )
}

pub(super) fn routine_set_active() -> Value {
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
    )
}

pub(super) fn routine_run() -> Value {
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
    )
}
