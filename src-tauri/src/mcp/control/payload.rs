use super::util::{bool_arg, integer_arg, optional, optional_str, required, string_array};
use anyhow::{Result, anyhow};
use serde_json::{Value, json};

/// Strict `usage_scope` validation shared by memory tools. Invalid values are
/// tagged so the JSON-RPC gateway can answer with `-32602`.
pub(super) fn usage_scope_arg(args: &Value) -> Result<Option<String>> {
    match optional_str(args, "usage_scope") {
        None => Ok(None),
        Some(value) if matches!(value, "normal" | "chat" | "both") => Ok(Some(value.into())),
        Some(value) => Err(anyhow!(
            "INVALID_PARAMS: usage_scope inválido '{value}'; use 'normal', 'chat' ou 'both'"
        )),
    }
}

pub(super) fn task_create_body(args: &Value) -> Result<Value> {
    Ok(json!({
        "title": required(args, "title")?,
        "description": required(args, "description")?,
        "execution_mode": optional(args, "execution_mode").unwrap_or_else(|| "AUTO_DSH".into()),
        "priority": integer_arg(args, "priority"),
        "assigned_agent": optional(args, "assigned_agent"),
        "preferred_model": optional(args, "preferred_model"),
        "effort_level": optional(args, "effort_level"),
        "notify_email": optional(args, "notify_email"),
        "export_to_gdocs": bool_arg(args, "export_to_gdocs").unwrap_or(false),
        "tags": string_array(args, "tags"),
        "origin": "MANUAL",
    }))
}

pub(super) fn task_update_body(args: &Value) -> Result<Value> {
    required(args, "id")?;
    let mut body = serde_json::Map::new();
    for key in [
        "title",
        "description",
        "assigned_agent",
        "preferred_model",
        "execution_mode",
        "effort_level",
        "notify_email",
        "priority",
    ] {
        if let Some(value) = args.get(key).filter(|value| !value.is_null()) {
            body.insert(key.into(), value.clone());
        }
    }
    if let Some(tags) = args.get("tags") {
        body.insert("tags".into(), tags.clone());
    }
    for key in ["export_to_gdocs", "clear_agent", "clear_model"] {
        if let Some(value) = args.get(key).and_then(Value::as_bool) {
            body.insert(key.into(), json!(value));
        }
    }
    Ok(Value::Object(body))
}

pub(super) fn routine_body(workspace_id: &str, args: &Value) -> Result<Value> {
    let recipe_id = optional_str(args, "recipe_id");
    let target_kind = optional_str(args, "target_kind")
        .map(str::to_string)
        .unwrap_or_else(|| {
            if recipe_id.is_some() {
                "RECIPE".into()
            } else {
                "PROMPT".into()
            }
        });
    let recurrence = args
        .get("recurrence")
        .filter(|value| !value.is_null())
        .map(serde_json::to_string)
        .transpose()?;
    let cron_expression = optional_str(args, "cron_expression")
        .map(str::to_string)
        .unwrap_or_default();
    let trigger_type = optional(args, "trigger_type")
        .map(|value| value.to_uppercase())
        .unwrap_or_else(|| "CRON".into());
    if trigger_type == "CRON" && recurrence.is_none() && cron_expression.trim().is_empty() {
        return Err(anyhow!(
            "rotinas CRON exigem 'recurrence' (preferido) ou 'cron_expression'"
        ));
    }
    let prompt = optional_str(args, "command_or_prompt")
        .map(str::to_string)
        .unwrap_or_default();
    if target_kind == "PROMPT" && prompt.trim().is_empty() {
        return Err(anyhow!(
            "command_or_prompt é obrigatório quando target_kind=PROMPT"
        ));
    }
    Ok(json!({
        "workspace_id": workspace_id,
        "name": required(args, "name")?,
        "description": optional(args, "description"),
        "cron_expression": cron_expression,
        "timezone": optional(args, "timezone")
            .unwrap_or_else(|| iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into())),
        "agent_name": optional(args, "agent_name"),
        "command_or_prompt": prompt,
        "trigger_type": trigger_type,
        "trigger_config_json": trigger_config_json(args)?,
        "target_kind": target_kind,
        "recipe_id": recipe_id,
        "router_policy": optional(args, "router_policy").unwrap_or_else(|| "COST_BENEFIT".into()),
        "output_folder_template": optional(args, "output_folder_template")
            .unwrap_or_else(|| "output/{YYYY}-{MM}/{schedule_slug}".into()),
        "notify_email_recipients": optional(args, "notify_email_recipients"),
        "notify_signal": bool_arg(args, "notify_signal").unwrap_or(false),
        "notify_signal_recipients": optional(args, "notify_signal_recipients"),
        "notify_telegram": bool_arg(args, "notify_telegram").unwrap_or(false),
        "notify_telegram_recipients": optional(args, "notify_telegram_recipients"),
        "notify_only_on_change": bool_arg(args, "notify_only_on_change").unwrap_or(false),
        "recurrence_json": recurrence,
        "one_time": false,
        "misfire_policy": optional(args, "misfire_policy").unwrap_or_else(|| "SKIP_TO_NEXT".into()),
        "safe_readonly_mode": bool_arg(args, "safe_readonly_mode").unwrap_or(true),
        "sync_to_google_docs": bool_arg(args, "sync_to_google_docs").unwrap_or(false),
        "sync_to_google_calendar": bool_arg(args, "sync_to_google_calendar").unwrap_or(false),
        // The backend also forces this; sending it makes the intent explicit.
        "is_active": false,
    }))
}

pub(super) fn routine_update_body(args: &Value) -> Result<Value> {
    required(args, "id")?;
    let mut body = serde_json::Map::new();
    for key in [
        "name",
        "description",
        "cron_expression",
        "timezone",
        "agent_name",
        "command_or_prompt",
        "trigger_type",
        "target_kind",
        "recipe_id",
        "router_policy",
        "output_folder_template",
        "notify_email_recipients",
        "notify_signal_recipients",
        "notify_telegram_recipients",
        "misfire_policy",
    ] {
        if let Some(value) = args.get(key).filter(|value| !value.is_null()) {
            body.insert(key.into(), value.clone());
        }
    }
    for key in [
        "notify_signal",
        "notify_telegram",
        "notify_only_on_change",
        "safe_readonly_mode",
        "sync_to_google_docs",
        "sync_to_google_calendar",
    ] {
        if let Some(value) = args.get(key).and_then(Value::as_bool) {
            body.insert(key.into(), json!(value));
        }
    }
    if let Some(recurrence) = args.get("recurrence").filter(|value| !value.is_null()) {
        body.insert(
            "recurrence_json".into(),
            json!(serde_json::to_string(recurrence)?),
        );
    }
    if args.get("trigger_config").is_some() || args.get("trigger_config_json").is_some() {
        body.insert(
            "trigger_config_json".into(),
            json!(trigger_config_json(args)?),
        );
    }
    Ok(Value::Object(body))
}

pub(super) fn trigger_config_json(args: &Value) -> Result<String> {
    if let Some(config) = args.get("trigger_config").filter(|value| !value.is_null()) {
        if !config.is_object() {
            return Err(anyhow!("'trigger_config' deve ser um objeto JSON"));
        }
        return Ok(serde_json::to_string(config)?);
    }
    if let Some(raw) = optional_str(args, "trigger_config_json") {
        serde_json::from_str::<Value>(raw)
            .map_err(|error| anyhow!("'trigger_config_json' inválido: {error}"))?;
        return Ok(raw.to_string());
    }
    Ok("{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_create_body_has_safe_defaults() {
        let body = task_create_body(&json!({
            "title": "Relatório",
            "description": "Produza o relatório mensal"
        }))
        .expect("body");
        assert_eq!(body["execution_mode"], "AUTO_DSH");
        assert_eq!(body["origin"], "MANUAL");
        assert_eq!(body["export_to_gdocs"], false);
        assert!(body["tags"].as_array().unwrap().is_empty());
    }

    #[test]
    fn routine_body_always_starts_inactive() {
        let body = routine_body(
            "ws-1",
            &json!({
                "name": "Resumo diário",
                "recurrence": { "frequency": "daily", "time": "08:00" },
                "command_or_prompt": "Resuma o dia"
            }),
        )
        .expect("body");
        assert_eq!(body["is_active"], false);
        assert!(body["recurrence_json"].as_str().unwrap().contains("daily"));
        assert_eq!(body["target_kind"], "PROMPT");
    }

    #[test]
    fn routine_body_requires_a_schedule_for_cron() {
        let error = routine_body("ws-1", &json!({ "name": "Sem agenda" })).unwrap_err();
        assert!(error.to_string().contains("recurrence"));
    }

    #[test]
    fn routine_body_supports_non_cron_triggers() {
        let body = routine_body(
            "ws-1",
            &json!({
                "name": "Pasta monitorada",
                "trigger_type": "FOLDER_WATCH",
                "trigger_config": { "path": "input" },
                "command_or_prompt": "Resuma os novos arquivos"
            }),
        )
        .expect("body");
        assert_eq!(body["trigger_type"], "FOLDER_WATCH");
        assert_eq!(body["is_active"], false);
        assert!(
            body["trigger_config_json"]
                .as_str()
                .unwrap()
                .contains("input")
        );
    }

    #[test]
    fn routine_update_accepts_trigger_config() {
        let body = routine_update_body(&json!({
            "id": "routine-1",
            "trigger_type": "WEBHOOK",
            "trigger_config": { "path": "/hooks/entrada" }
        }))
        .expect("body");
        assert_eq!(body["trigger_type"], "WEBHOOK");
        assert!(
            body["trigger_config_json"]
                .as_str()
                .unwrap()
                .contains("/hooks/entrada")
        );
    }

    #[test]
    fn usage_scope_is_fail_closed() {
        assert_eq!(usage_scope_arg(&json!({})).expect("default"), None);
        assert_eq!(
            usage_scope_arg(&json!({ "usage_scope": "chat" })).expect("chat"),
            Some("chat".into())
        );
        let error = usage_scope_arg(&json!({ "usage_scope": "none" })).unwrap_err();
        assert!(error.to_string().contains("INVALID_PARAMS"));
        assert!(error.to_string().contains("normal"));
    }
}
